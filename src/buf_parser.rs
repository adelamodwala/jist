use crate::utils::{array_ind, checkpoint_depth, sanitize_output, token_pos};
use json_tools::{BufferType, Lexer, TokenType};
use log::debug;
use std::cmp::Ordering;
use std::io::{BufRead, Read, Seek, SeekFrom};

struct StreamTracker {
    last_stream_pos: u64,
    last_chunk_len: usize,
    buffer: Vec<u8>,
    chunk: Vec<u8>,
}
impl StreamTracker {
    fn new(chunk_size: usize) -> StreamTracker {
        StreamTracker {
            last_stream_pos: 0,
            last_chunk_len: 0,
            buffer: Vec::with_capacity(chunk_size * 2), // Extra space for overflow,
            chunk: Vec::new(),
        }
    }
}

struct JStructTracker {
    // tuple of (depth, arr_depth, obj_depth)
    depth_curr: (i32, i32, i32),

    // keep track of array indices if currently inside array
    arr_idx: Vec<i64>,
    last_open: Vec<TokenType>,
    checkpoint_start: Vec<u64>,
    last_token_key_delimiter: bool,

    // build checkpoints that must pass
    checkpoints: Vec<(i32, i32, i32)>,
    search_keys: Vec<String>,
    arr_tgt: Vec<i64>,
    arr_tgt_size: usize,
}
impl JStructTracker {
    fn new(search_path: &Vec<String>) -> JStructTracker {
        let mut struct_tracker = JStructTracker {
            depth_curr: (-1, -1, -1),
            arr_idx: Vec::new(),
            last_open: Vec::new(),
            checkpoint_start: Vec::new(),
            last_token_key_delimiter: false,
            checkpoints: Vec::new(),
            search_keys: Vec::new(),
            arr_tgt: Vec::new(),
            arr_tgt_size: 0,
        };

        for search_idx in 0..search_path.len() {
            struct_tracker.checkpoints.push(checkpoint_depth(search_path, search_idx))
        }
        struct_tracker.checkpoints.reverse();

        struct_tracker.arr_tgt = search_path.iter()
            .filter(|x| x.starts_with("["))
            .map(|x| array_ind(x.clone()))
            .rev()
            .collect();
        struct_tracker.arr_tgt_size = struct_tracker.arr_tgt.len();
        struct_tracker.search_keys = search_path.iter()
            .filter(|x| !x.starts_with("["))
            .map(|x| x.clone())
            .rev()
            .collect::<Vec<String>>();
        debug!("checkpoints: {:?}, arr_tgt: {:?}, search_keys: {:?}", struct_tracker.checkpoints, struct_tracker.arr_tgt, struct_tracker.search_keys);
        struct_tracker
    }
}

fn find_str<R: Read + Seek>(mut seeker: R, start: u64, end: u64) -> Option<String> {
    let mut buff = vec![0u8; end as usize - start as usize];
    seeker.seek(SeekFrom::Start(start)).expect("error");
    seeker.read_exact(&mut buff).expect("error");
    String::from_utf8(buff.to_vec()).ok()
}

pub(crate) fn search<R: Read + Seek + BufRead>(mut reader: R, mut seeker: R, search_path: &Vec<String>, buff_size: Option<usize>) -> Result<String, &'static str> {
    let chunk_size = if buff_size.is_some() {buff_size.unwrap()} else { 1000000 };
    let mut stream_tracker = StreamTracker::new(chunk_size);
    let mut struct_tracker = JStructTracker::new(search_path);

    loop {
        let bytes_read = reader.by_ref().take(chunk_size as u64).read_to_end(&mut stream_tracker.buffer).unwrap();
        if bytes_read == 0 && stream_tracker.buffer.is_empty() {
            return Err("result not found");
        }

        // Find the last line break (add one if last buffer read)
        let mut chunk_str = String::from_utf8(stream_tracker.buffer.to_vec()).unwrap();
        if bytes_read < chunk_size {
            chunk_str.push('\n');
        }
        if let Some(last_chunk_tup) = chunk_str.rsplit_once("\n") {
            let last_chunk = last_chunk_tup.0;
            debug!("last_chunk: {}", last_chunk);
            // Process the chunk that ends with a newline
            stream_tracker.chunk.extend_from_slice(last_chunk.as_bytes());
            stream_tracker.chunk.push(b'\n');
            stream_tracker.last_chunk_len = stream_tracker.chunk.len();

            // Process chunk here
            let mut token_iter = Lexer::new(stream_tracker.chunk.to_vec(), BufferType::Span).peekable();

            loop {
                let token_opt = token_iter.next();
                if token_opt.is_none() {
                    break;
                }

                let mut token = token_opt.unwrap();

                let arr_idx_len = struct_tracker.arr_idx.len();
                match token.kind {
                    TokenType::CurlyOpen => {
                        struct_tracker.depth_curr.0 += 1;
                        struct_tracker.depth_curr.2 += 1;
                        struct_tracker.last_open.push(TokenType::CurlyOpen);
                    }
                    TokenType::CurlyClose => {
                        struct_tracker.depth_curr.0 -= 1;
                        struct_tracker.depth_curr.2 -= 1;
                        struct_tracker.last_open.pop();
                    }
                    TokenType::BracketOpen => {
                        struct_tracker.depth_curr.0 += 1;
                        struct_tracker.depth_curr.1 += 1;
                        struct_tracker.arr_idx.push(0);
                        struct_tracker.last_open.push(TokenType::BracketOpen);
                    }
                    TokenType::BracketClose => {
                        struct_tracker.depth_curr.0 -= 1;
                        struct_tracker.depth_curr.1 -= 1;
                        struct_tracker.arr_idx.pop();
                        struct_tracker.last_open.pop();
                    }
                    TokenType::Comma => {
                        if struct_tracker.depth_curr.1 > -1                    // must be inside an array
                            && *struct_tracker.last_open.last().unwrap() == TokenType::BracketOpen
                        {
                            struct_tracker.arr_idx[arr_idx_len - 1] += 1;
                        }
                    }
                    _ => {}
                }
                debug!("depth_curr: {:?}, arr_idx: {:?}, arr_tgt: {:?}, search_keys: {:?}, kind: {:?}, last_open: {:?}, checkpoints: {:?}, checkpoint_start: {:?}", struct_tracker.depth_curr, struct_tracker.arr_idx, struct_tracker.arr_tgt, struct_tracker.search_keys, &token.kind, struct_tracker.last_open, struct_tracker.checkpoints, struct_tracker.checkpoint_start);

                if struct_tracker.depth_curr.cmp(&struct_tracker.checkpoints.last().unwrap()) == Ordering::Equal {
                    // check if inside an array and iterating to find the expected place
                    if *struct_tracker.last_open.last().unwrap() == TokenType::BracketOpen && struct_tracker.arr_idx.last().unwrap().cmp(&struct_tracker.arr_tgt.last().unwrap()) == Ordering::Equal {
                        let (first, end) = token_pos(&token.buf)?;

                        // terminal point
                        if struct_tracker.checkpoints.len() == 1 && struct_tracker.checkpoint_start.len() == struct_tracker.arr_tgt_size {
                            debug!("[checkpoint ended]");

                            // add 1 to starting index to exclude commas or brackets
                            let result = find_str(&mut seeker, (struct_tracker.checkpoint_start.last().unwrap()) + 1, end + stream_tracker.last_stream_pos).unwrap();
                            return Ok(sanitize_output(result.as_str()));
                        } else {
                            debug!("[checkpoint started]");

                            struct_tracker.checkpoint_start.push(first + stream_tracker.last_stream_pos);
                            if struct_tracker.arr_tgt.len() > 1 {
                                struct_tracker.arr_tgt.pop();
                            }
                        }

                        if struct_tracker.checkpoints.len() > 1 {
                            struct_tracker.checkpoints.pop();
                        }
                    } else if *struct_tracker.last_open.last().unwrap() == TokenType::CurlyOpen {
                        if token.kind == TokenType::String && struct_tracker.last_token_key_delimiter {
                            let (first, end) = token_pos(&token.buf)?;
                            let key = find_str(&mut seeker, first + stream_tracker.last_stream_pos, end + stream_tracker.last_stream_pos).unwrap();
                            if key.trim_start_matches("\"").trim_end_matches("\"").cmp(&struct_tracker.search_keys.last().unwrap()) == Ordering::Equal {
                                debug!(">>>found next key: {:?}", key);
                                struct_tracker.search_keys.pop();

                                if struct_tracker.checkpoints.len() > 1 {
                                    struct_tracker.checkpoints.pop();
                                }
                            }
                            struct_tracker.last_token_key_delimiter = false;
                        } else if (token.kind == TokenType::CurlyOpen || token.kind == TokenType::Comma) && struct_tracker.search_keys.len() > 0 {
                            if token.kind == TokenType::CurlyOpen {
                                debug!("\\checkpoint started\\");
                            }
                            struct_tracker.last_token_key_delimiter = true;
                        } else if struct_tracker.search_keys.len() == 0 && struct_tracker.checkpoints.len() == 1 {
                            let (first, end) = token_pos(&token.buf)?;
                            match token.kind {
                                TokenType::String | TokenType::Number | TokenType::BooleanFalse | TokenType::BooleanTrue | TokenType::Null => {
                                    let result = find_str(&mut seeker, first + stream_tracker.last_stream_pos, end + stream_tracker.last_stream_pos).unwrap();
                                    return Ok(sanitize_output(result.as_str()));
                                }
                                TokenType::Colon => {
                                    struct_tracker.checkpoint_start.push(first + stream_tracker.last_stream_pos);
                                }
                                TokenType::CurlyClose | TokenType::BracketClose => {
                                    let result = find_str(&mut seeker, struct_tracker.checkpoint_start.last().unwrap() + 1, end + stream_tracker.last_stream_pos).unwrap();
                                    return Ok(sanitize_output(result.as_str()));
                                }
                                _ => {}
                            }
                            struct_tracker.last_token_key_delimiter = false;
                        }
                    }
                }

                if !token_iter.peek().is_some() {
                    break;
                }
            }

            // Clear chunk for next iteration
            stream_tracker.chunk.clear();
            // Remove processed data from buffer
            stream_tracker.buffer.drain(..last_chunk.len() + 1);
        } else {
            return Err("result not found");
        }

        stream_tracker.last_stream_pos += stream_tracker.last_chunk_len as u64;
        debug!("page finished - stream_position: {:?}", stream_tracker.last_stream_pos);
    }
}