use crate::utils::{array_ind, checkpoint_depth, sanitize_output, token_pos};
use json_tools::{BufferType, Lexer, TokenType};
use log::debug;
use std::cmp::Ordering;
use std::io::{BufRead, Read, Seek, SeekFrom};

fn find_str<R: Read + Seek>(mut seeker: R, start: u64, end: u64) -> Option<String> {
    let mut buff = vec![0u8; end as usize - start as usize];
    seeker.seek(SeekFrom::Start(start)).expect("error");
    seeker.read_exact(&mut buff).expect("error");
    String::from_utf8(buff.to_vec()).ok()
}

pub(crate) fn search<R: Read + Seek + BufRead>(mut reader: R, mut seeker: R, search_path: &Vec<String>, buff_size: Option<usize>) -> Result<String, &'static str> {
    let mut last_stream_pos = 0;
    let mut last_chunk_len = 0;
    let chunk_size = if buff_size.is_some() {buff_size.unwrap()} else { 1000000 };
    let mut buffer = Vec::with_capacity(chunk_size * 2); // Extra space for overflow
    let mut chunk = Vec::new();

    // tuple of (depth, arr_depth, obj_depth)
    let mut depth_curr: (i32, i32, i32) = (-1, -1, -1);

    // keep track of array indices if currently inside array
    let mut arr_idx = Vec::new();
    let mut last_open: Vec<TokenType> = Vec::new();
    let mut checkpoint_start = Vec::new();
    let mut last_token_key_delimiter = false;

    // build checkpoints that must pass
    let mut checkpoints = Vec::new();
    for search_idx in 0..search_path.len() {
        checkpoints.push(checkpoint_depth(search_path, search_idx))
    }
    checkpoints.reverse();

    let mut arr_tgt: Vec<i64> = search_path.iter()
        .filter(|x| x.starts_with("["))
        .map(|x| array_ind(x.clone()))
        .rev()
        .collect();
    let arr_tgt_size = arr_tgt.len();
    let mut search_keys = search_path.iter()
        .filter(|x| !x.starts_with("["))
        .map(|x| x.clone())
        .rev()
        .collect::<Vec<String>>();
    debug!("checkpoints: {:?}, arr_tgt: {:?}, search_keys: {:?}", checkpoints, arr_tgt, search_keys);

    loop {
        let bytes_read = reader.by_ref().take(chunk_size as u64).read_to_end(&mut buffer).unwrap();
        if bytes_read == 0 && buffer.is_empty() {
            return Err("result not found");
        }

        // Find the last line break
        let chunk_str = String::from_utf8(buffer.to_vec()).unwrap();
        if let Some(last_chunk_tup) = chunk_str.rsplit_once("\n") {
            let last_chunk = last_chunk_tup.0;
            debug!("last_chunk: {}", last_chunk);
            // Process the chunk that ends with a newline
            chunk.extend_from_slice(last_chunk.as_bytes());
            chunk.push(b'\n');
            last_chunk_len = chunk.len();

            // Process chunk here
            let mut token_iter = Lexer::new(chunk.to_vec(), BufferType::Span).peekable();

            loop {
                let token_opt = token_iter.next();
                if token_opt.is_none() {
                    break;
                }

                let mut token = token_opt.unwrap();

                let arr_idx_len = arr_idx.len();
                match token.kind {
                    TokenType::CurlyOpen => {
                        depth_curr.0 += 1;
                        depth_curr.2 += 1;
                        last_open.push(TokenType::CurlyOpen);
                    }
                    TokenType::CurlyClose => {
                        depth_curr.0 -= 1;
                        depth_curr.2 -= 1;
                        last_open.pop();
                    }
                    TokenType::BracketOpen => {
                        depth_curr.0 += 1;
                        depth_curr.1 += 1;
                        arr_idx.push(0);
                        last_open.push(TokenType::BracketOpen);
                    }
                    TokenType::BracketClose => {
                        depth_curr.0 -= 1;
                        depth_curr.1 -= 1;
                        arr_idx.pop();
                        last_open.pop();
                    }
                    TokenType::Comma => {
                        if depth_curr.1 > -1                    // must be inside an array
                            && *last_open.last().unwrap() == TokenType::BracketOpen
                        {
                            arr_idx[arr_idx_len - 1] += 1;
                        }
                    }
                    _ => {}
                }
                debug!("depth_curr: {:?}, arr_idx: {:?}, arr_tgt: {:?}, search_keys: {:?}, kind: {:?}, last_open: {:?}, checkpoints: {:?}, checkpoint_start: {:?}", depth_curr, arr_idx, arr_tgt, search_keys, &token.kind, last_open, checkpoints, checkpoint_start);

                if depth_curr.cmp(&checkpoints.last().unwrap()) == Ordering::Equal {
                    // check if inside an array and iterating to find the expected place
                    if *last_open.last().unwrap() == TokenType::BracketOpen && arr_idx.last().unwrap().cmp(&arr_tgt.last().unwrap()) == Ordering::Equal {
                        let (first, end) = token_pos(&token.buf)?;

                        // terminal point
                        if checkpoints.len() == 1 && checkpoint_start.len() == arr_tgt_size {
                            debug!("[checkpoint ended]");

                            // add 1 to starting index to exclude commas or brackets
                            let result = find_str(&mut seeker, (checkpoint_start.last().unwrap()) + 1, end + last_stream_pos).unwrap();
                            return Ok(sanitize_output(result.as_str()));
                        } else {
                            debug!("[checkpoint started]");

                            checkpoint_start.push(first + last_stream_pos);
                            if arr_tgt.len() > 1 {
                                arr_tgt.pop();
                            }
                        }

                        if checkpoints.len() > 1 {
                            checkpoints.pop();
                        }
                    } else if *last_open.last().unwrap() == TokenType::CurlyOpen {
                        if token.kind == TokenType::String && last_token_key_delimiter {
                            let (first, end) = token_pos(&token.buf)?;
                            let key = find_str(&mut seeker, first + last_stream_pos, end + last_stream_pos).unwrap();
                            if key.trim_start_matches("\"").trim_end_matches("\"").cmp(&search_keys.last().unwrap()) == Ordering::Equal {
                                debug!(">>>found next key: {:?}", key);
                                search_keys.pop();

                                if checkpoints.len() > 1 {
                                    checkpoints.pop();
                                }
                            }
                            last_token_key_delimiter = false;
                        } else if (token.kind == TokenType::CurlyOpen || token.kind == TokenType::Comma) && search_keys.len() > 0 {
                            if token.kind == TokenType::CurlyOpen {
                                debug!("\\checkpoint started\\");
                            }
                            last_token_key_delimiter = true;
                        } else if search_keys.len() == 0 && checkpoints.len() == 1 {
                            let (first, end) = token_pos(&token.buf)?;
                            match token.kind {
                                TokenType::String | TokenType::Number | TokenType::BooleanFalse | TokenType::BooleanTrue | TokenType::Null => {
                                    let result = find_str(&mut seeker, first + last_stream_pos, end + last_stream_pos).unwrap();
                                    return Ok(sanitize_output(result.as_str()));
                                }
                                TokenType::Colon => {
                                    checkpoint_start.push(first + last_stream_pos);
                                }
                                TokenType::CurlyClose | TokenType::BracketClose => {
                                    let result = find_str(&mut seeker, checkpoint_start.last().unwrap() + 1, end + last_stream_pos).unwrap();
                                    return Ok(sanitize_output(result.as_str()));
                                }
                                _ => {}
                            }
                            last_token_key_delimiter = false;
                        }
                    }
                }

                if !token_iter.peek().is_some() {
                    break;
                }
            }

            // Clear chunk for next iteration
            chunk.clear();
            // Remove processed data from buffer
            buffer.drain(..last_chunk.len() + 1);
        } else {
            return Err("result not found");
        }

        last_stream_pos += last_chunk_len as u64;
        debug!("page finished - stream_position: {:?}", last_stream_pos);
    }
}