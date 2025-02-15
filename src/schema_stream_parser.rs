use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use json_tools::{BufferType, Lexer, Token, TokenType};
use json_tools::TokenType::Invalid;
use log::debug;
use serde_json::Value;
use crate::buf_parser::_search;
use crate::model::j_struct_tracker::JStructTracker;
use crate::model::stream_tracker::StreamTracker;
use crate::utils;
use crate::utils::{find_str, token_pos};

pub fn parse(
    haystack: Option<&str>,
    file: Option<&str>, // Keep this as Option<&str> for future flexibility with testing & dev
) -> Result<String, &'static str> {
    if file.is_some() {
        let f = File::open(file.unwrap()).unwrap();
        let mut reader = BufReader::new(&f);
        let mut seeker = BufReader::new(&f);
        _parse(&mut reader, &mut seeker)
    } else if haystack.is_some() {
        let haystack_str = haystack.unwrap();
        if haystack_str.is_empty() {
            return Err("Invalid input - empty data");
        }
        let mut reader = Cursor::new(haystack_str.as_bytes());
        let mut seeker = Cursor::new(haystack_str.as_bytes());
        _parse(&mut reader, &mut seeker)
    } else {
        Err("Invalid input - empty data")
    }
}

pub fn _parse<R: Read + Seek + BufRead>(
    mut reader: R,
    mut seeker: R,
) -> Result<String, &'static str> {
    let chunk_size = 1_000_000;
    let mut stream_t = StreamTracker::new(chunk_size);
    let mut struct_t = JStructTracker::init();
    let mut schema_tape = String::new();

    loop {
        reader.seek(SeekFrom::Start(stream_t.last_stream_pos)).expect("Unable to seek");
        let bytes_read = reader
            .by_ref()
            .take(chunk_size as u64)
            .read_to_end(&mut stream_t.buffer)
            .unwrap();
        if bytes_read == 0 && stream_t.buffer.is_empty() {
            break;
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

                // token processing
                let mut token = token_opt.unwrap();
                match token.kind {
                    Invalid => {
                        panic!("At the disco!");
                    }
                    TokenType::CurlyOpen => {
                        struct_t.depth_curr.0 += 1;
                        struct_t.depth_curr.2 += 1;
                        let (first, end) = token_pos(&token.buf)?;
                        struct_t
                            .last_open_pin
                            .push((TokenType::CurlyOpen, first, schema_tape.len()));
                        schema_tape = schema_tape + "{";
                    }
                    TokenType::CurlyClose => {
                        struct_t.depth_curr.0 -= 1;
                        struct_t.depth_curr.2 -= 1;
                        schema_tape = schema_tape + "}";

                        let last_curly_open = struct_t
                            .last_open_pin
                            .iter()
                            .filter(|sym| sym.0 == TokenType::CurlyOpen)
                            .last();
                        if last_curly_open.is_none() {
                            return Err("invalid json: missing opening curly brace");
                        }

                        struct_t.last_open_pin.pop();
                    }
                    TokenType::BracketOpen => {
                        struct_t.depth_curr.0 += 1;
                        struct_t.depth_curr.1 += 1;
                        struct_t.arr_idx.push(0);
                        let (first, end) = token_pos(&token.buf)?;
                        struct_t
                            .last_open_pin
                            .push((TokenType::BracketOpen, first, schema_tape.len()));
                        schema_tape = schema_tape + "[";
                    }
                    TokenType::BracketClose => {
                        struct_t.depth_curr.0 -= 1;
                        struct_t.depth_curr.1 -= 1;
                        schema_tape = schema_tape + "]";

                        let last_bracket_open = struct_t
                            .last_open_pin
                            .iter()
                            .filter(|sym| sym.0 == TokenType::BracketOpen)
                            .last();
                        if last_bracket_open.is_none() {
                            return Err("invalid json: missing opening bracket");
                        }

                        struct_t.arr_idx.pop();
                        struct_t.last_open_pin.pop();
                    }
                    TokenType::Comma => {
                        let (token_type, _, _) = struct_t.last_open_pin.last().unwrap();
                        if struct_t.depth_curr.1 > -1                    // must be inside an array
                            && token_type.eq(&TokenType::BracketOpen)
                        {
                            let arr_idx_len = struct_t.arr_idx.len();
                            struct_t.arr_idx[arr_idx_len - 1] += 1;
                        }
                        schema_tape = schema_tape + ",";
                    }
                    TokenType::Colon => schema_tape = schema_tape + ":",
                    TokenType::BooleanTrue | TokenType::BooleanFalse => {
                        schema_tape = schema_tape + "\"boolean\""
                    }
                    TokenType::Number => schema_tape = schema_tape + "\"number\"",
                    TokenType::Null => schema_tape = schema_tape + "\"string\"",
                    _ => {} // String type can be an object key which requires special handling
                }

                if !struct_t.last_open_pin.is_empty() {
                    let (token_type, _, _) = struct_t.last_open_pin.last().unwrap();
                    if token_type.eq(&TokenType::BracketOpen) {
                        if token.kind == TokenType::String {
                            schema_tape = schema_tape + "\"string\"";
                        }
                    } else if token_type.eq(&TokenType::CurlyOpen) {
                        if token.kind == TokenType::String && struct_t.last_token_key_delimiter {
                            let (first, end) = token_pos(&token.buf)?;
                            let key = find_str(
                                &mut seeker,
                                first + stream_t.last_stream_pos,
                                end + stream_t.last_stream_pos,
                            ).unwrap();
                            schema_tape = schema_tape + &key;
                            struct_t.last_token_key_delimiter = false;
                        } else if token.kind == TokenType::String {
                            schema_tape = schema_tape + "\"string\"";
                        }

                        if token.kind == TokenType::CurlyOpen || token.kind == TokenType::Comma {
                            struct_t.last_token_key_delimiter = true;
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

    let mut json: Value = serde_json::from_str(schema_tape.as_str()).expect("JSON parsing error");
    json = crate::schema_parser::deduplicate_arrays(json);
    json = crate::schema_parser::sort_serde_json(&json);

    Ok(json.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{BufReader, Cursor};
    use super::*;

    // #[test]
    fn test_parse() {
        let f = File::open("test.json").unwrap();
        let mut reader = BufReader::new(&f);
        let mut seeker = BufReader::new(&f);
        let result = _parse(&mut reader, &mut seeker);
        print!("{:?}", result.unwrap());
    }
}