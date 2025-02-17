use crate::utils::{array_ind, checkpoint_depth, find_str, sanitize_output, token_pos};
use crate::{buf_parser, utils};
use json_tools::{BufferType, Lexer, TokenType};
use log::debug;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use crate::model::j_struct_tracker::JStructTracker;
use crate::model::stream_tracker::StreamTracker;

pub fn search(
    haystack: Option<&str>,
    file: Option<&str>, // Keep this as Option<&str> for future flexibility with testing & dev
    search_key: &str,
) -> Result<String, &'static str> {
    if search_key.is_empty() {
        return Err("search_key is empty");
    }
    let search_path = utils::parse_search_key(search_key);
    if file.is_some() {
        let f = File::open(file.unwrap()).unwrap();
        let mut reader = BufReader::new(&f);
        let mut seeker = BufReader::new(&f);
        _search(&mut reader, &mut seeker, &search_path)
    } else if haystack.is_some() {
        let haystack_str = haystack.unwrap();
        if haystack_str.is_empty() {
            return Err("Invalid input - empty data");
        }
        let mut reader = Cursor::new(haystack_str.as_bytes());
        let mut seeker = Cursor::new(haystack_str.as_bytes());
        _search(&mut reader, &mut seeker, &search_path)
    } else {
        Err("Invalid input - empty data")
    }
}

pub fn _search<R: Read + Seek + BufRead>(
    mut reader: R,
    mut seeker: R,
    search_path: &[String],
) -> Result<String, &'static str> {
    let chunk_size = 1_000_000;
    let mut stream_t = StreamTracker::new(chunk_size);
    let mut struct_t = JStructTracker::new(search_path);

    loop {
        reader.seek(SeekFrom::Start(stream_t.last_stream_pos)).expect("Unable to seek");
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
                            let arr_idx_len = struct_t.arr_idx.len();
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
            stream_t.buffer.clear();
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
