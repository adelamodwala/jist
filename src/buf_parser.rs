use crate::utils::{array_ind, checkpoint_depth, sanitize_output, token_pos};
use json_tools::{BufferType, Lexer, TokenType};
use log::debug;
use std::cmp::Ordering;
use std::io::{BufRead, Read, Seek, SeekFrom};

pub struct StreamTracker {
    pub last_stream_pos: u64,
    pub last_chunk_len: usize,
    pub buffer: Vec<u8>,
    pub chunk: Vec<u8>,
}
impl StreamTracker {
    pub fn new(chunk_size: usize) -> StreamTracker {
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
    fn new(search_path: &[String]) -> JStructTracker {
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
            struct_tracker
                .checkpoints
                .push(checkpoint_depth(search_path, search_idx));
        }
        struct_tracker.checkpoints.reverse();

        struct_tracker.arr_tgt = search_path
            .iter()
            .filter(|x| x.starts_with('['))
            .map(|x| array_ind(x.as_str()))
            .rev()
            .collect();
        struct_tracker.arr_tgt_size = struct_tracker.arr_tgt.len();
        struct_tracker.search_keys = search_path
            .iter()
            .filter(|x| !x.starts_with('['))
            .cloned()
            .rev()
            .collect::<Vec<String>>();
        debug!(
            "checkpoints: {:?}, arr_tgt: {:?}, search_keys: {:?}",
            struct_tracker.checkpoints, struct_tracker.arr_tgt, struct_tracker.search_keys
        );
        struct_tracker
    }
}

fn find_str<R: Read + Seek>(mut seeker: R, start: u64, end: u64) -> Option<String> {
    let mut buff = vec![0u8; end as usize - start as usize];
    seeker.seek(SeekFrom::Start(start)).expect("error");
    seeker.read_exact(&mut buff).expect("error");
    String::from_utf8(buff.clone()).ok()
}

pub(crate) fn search<R: Read + Seek + BufRead>(
    mut reader: R,
    mut seeker: R,
    search_path: &[String],
    buff_size: Option<usize>,
) -> Result<String, &'static str> {
    let chunk_size = buff_size.unwrap_or(1_000_000);
    let mut stream_t = StreamTracker::new(chunk_size);
    let mut struct_t = JStructTracker::new(search_path);

    loop {
        let bytes_read = reader
            .by_ref()
            .take(chunk_size as u64)
            .read_to_end(&mut stream_t.buffer)
            .unwrap();
        if bytes_read == 0 && stream_t.buffer.is_empty() {
            return Err("result not found");
        }

        // Find the last line break (add one if last buffer read)
        let mut chunk_str = String::from_utf8(stream_t.buffer.clone()).unwrap();
        if bytes_read < chunk_size {
            chunk_str.push('\n');
        }
        if let Some(last_chunk_tup) = chunk_str.rsplit_once('\n') {
            let last_chunk = last_chunk_tup.0;
            debug!("last_chunk: {}", last_chunk);
            // Process the chunk that ends with a newline
            stream_t.chunk.extend_from_slice(last_chunk.as_bytes());
            stream_t.chunk.push(b'\n');
            stream_t.last_chunk_len = stream_t.chunk.len();

            // Process chunk here
            let mut token_iter = Lexer::new(stream_t.chunk.clone(), BufferType::Span).peekable();

            loop {
                let token_opt = token_iter.next();
                if token_opt.is_none() {
                    break;
                }

                let mut token = token_opt.unwrap();

                let arr_idx_len = struct_t.arr_idx.len();
                match token.kind {
                    TokenType::CurlyOpen => {
                        struct_t.depth_curr.0 += 1;
                        struct_t.depth_curr.2 += 1;
                        struct_t.last_open.push(TokenType::CurlyOpen);
                    }
                    TokenType::CurlyClose => {
                        struct_t.depth_curr.0 -= 1;
                        struct_t.depth_curr.2 -= 1;
                        struct_t.last_open.pop();
                    }
                    TokenType::BracketOpen => {
                        struct_t.depth_curr.0 += 1;
                        struct_t.depth_curr.1 += 1;
                        struct_t.arr_idx.push(0);
                        struct_t.last_open.push(TokenType::BracketOpen);
                    }
                    TokenType::BracketClose => {
                        struct_t.depth_curr.0 -= 1;
                        struct_t.depth_curr.1 -= 1;
                        struct_t.arr_idx.pop();
                        struct_t.last_open.pop();
                    }
                    TokenType::Comma => {
                        if struct_t.depth_curr.1 > -1                    // must be inside an array
                            && *struct_t.last_open.last().unwrap() == TokenType::BracketOpen
                        {
                            struct_t.arr_idx[arr_idx_len - 1] += 1;
                        }
                    }
                    _ => {}
                }
                debug!("depth_curr: {:?}, arr_idx: {:?}, arr_tgt: {:?}, search_keys: {:?}, kind: {:?}, last_open: {:?}, checkpoints: {:?}, checkpoint_start: {:?}", struct_t.depth_curr, struct_t.arr_idx, struct_t.arr_tgt, struct_t.search_keys, &token.kind, struct_t.last_open, struct_t.checkpoints, struct_t.checkpoint_start);

                if struct_t
                    .depth_curr
                    .cmp(struct_t.checkpoints.last().unwrap())
                    == Ordering::Equal
                {
                    // check if inside an array and iterating to find the expected place
                    if *struct_t.last_open.last().unwrap() == TokenType::BracketOpen
                        && struct_t
                            .arr_idx
                            .last()
                            .unwrap()
                            .cmp(struct_t.arr_tgt.last().unwrap())
                            == Ordering::Equal
                    {
                        let (first, end) = token_pos(&token.buf)?;

                        // terminal point
                        if struct_t.checkpoints.len() == 1
                            && struct_t.checkpoint_start.len() == struct_t.arr_tgt_size
                        {
                            debug!("[checkpoint ended]");

                            // add 1 to starting index to exclude commas or brackets
                            let result = find_str(
                                &mut seeker,
                                (struct_t.checkpoint_start.last().unwrap()) + 1,
                                end + stream_t.last_stream_pos,
                            )
                            .unwrap();
                            return Ok(sanitize_output(result.as_str()));
                        } else {
                            debug!("[checkpoint started]");

                            struct_t
                                .checkpoint_start
                                .push(first + stream_t.last_stream_pos);
                            if struct_t.arr_tgt.len() > 1 {
                                struct_t.arr_tgt.pop();
                            }
                        }

                        if struct_t.checkpoints.len() > 1 {
                            struct_t.checkpoints.pop();
                        }
                    } else if *struct_t.last_open.last().unwrap() == TokenType::CurlyOpen {
                        if token.kind == TokenType::String && struct_t.last_token_key_delimiter {
                            let (first, end) = token_pos(&token.buf)?;
                            let key = find_str(
                                &mut seeker,
                                first + stream_t.last_stream_pos,
                                end + stream_t.last_stream_pos,
                            )
                            .unwrap();
                            if key
                                .trim_start_matches('\"')
                                .trim_end_matches('\"')
                                .cmp(struct_t.search_keys.last().unwrap())
                                == Ordering::Equal
                            {
                                debug!(">>>found next key: {:?}", key);
                                struct_t.search_keys.pop();

                                if struct_t.checkpoints.len() > 1 {
                                    struct_t.checkpoints.pop();
                                }
                            }
                            struct_t.last_token_key_delimiter = false;
                        } else if (token.kind == TokenType::CurlyOpen
                            || token.kind == TokenType::Comma)
                            && !struct_t.search_keys.is_empty()
                        {
                            if token.kind == TokenType::CurlyOpen {
                                debug!("\\checkpoint started\\");
                            }
                            struct_t.last_token_key_delimiter = true;
                        } else if struct_t.search_keys.is_empty() && struct_t.checkpoints.len() == 1
                        {
                            let (first, end) = token_pos(&token.buf)?;
                            match token.kind {
                                TokenType::String
                                | TokenType::Number
                                | TokenType::BooleanFalse
                                | TokenType::BooleanTrue
                                | TokenType::Null => {
                                    let result = find_str(
                                        &mut seeker,
                                        first + stream_t.last_stream_pos,
                                        end + stream_t.last_stream_pos,
                                    )
                                    .unwrap();
                                    return Ok(sanitize_output(result.as_str()));
                                }
                                TokenType::Colon => {
                                    struct_t
                                        .checkpoint_start
                                        .push(first + stream_t.last_stream_pos);
                                }
                                TokenType::CurlyClose | TokenType::BracketClose => {
                                    let result = find_str(
                                        &mut seeker,
                                        struct_t.checkpoint_start.last().unwrap() + 1,
                                        end + stream_t.last_stream_pos,
                                    )
                                    .unwrap();
                                    return Ok(sanitize_output(result.as_str()));
                                }
                                _ => {}
                            }
                            struct_t.last_token_key_delimiter = false;
                        }
                    }
                }

                if token_iter.peek().is_none() {
                    break;
                }
            }

            // Clear chunk for next iteration
            stream_t.chunk.clear();
            // Remove processed data from buffer
            stream_t.buffer.drain(..=last_chunk.len());
        } else {
            return Err("result not found");
        }

        stream_t.last_stream_pos += stream_t.last_chunk_len as u64;
        debug!(
            "page finished - stream_position: {:?}",
            stream_t.last_stream_pos
        );
    }
}
