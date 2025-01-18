use crate::model::j_struct_tracker::JStructTracker;
use crate::model::stream_tracker::StreamTracker;
use crate::utils::{find_str, token_pos};
use json_tools::{BufferType, Lexer, Token, TokenType};
use log::debug;
use md5;
use md5::Digest;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};
use std::ops::Add;
use std::path::absolute;
use std::thread;

fn simple_poc<R: Read + Seek + BufRead>(
    mut reader: R,
    mut seeker: R,
) -> Result<String, &'static str> {
    let chunk_size = 1_000;
    let mut stream_t = StreamTracker::new(chunk_size);
    let mut struct_t = JStructTracker::init();
    let mut schema = String::new();

    loop {
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

            // chunk processing
            let mut token_iter = Lexer::new(stream_t.chunk.clone(), BufferType::Span).peekable();
            loop {
                let token_opt = token_iter.next();
                if token_opt.is_none() {
                    break;
                }

                // token processing
                let mut token = token_opt.unwrap();
                match token.kind {
                    TokenType::CurlyOpen => {
                        struct_t.depth_curr.0 += 1;
                        struct_t.depth_curr.2 += 1;
                        let (first, end) = token_pos(&token.buf)?;
                        struct_t.last_open_pin.push((TokenType::CurlyOpen, first));
                        schema = schema + "{";
                    }
                    TokenType::CurlyClose => {
                        struct_t.depth_curr.0 -= 1;
                        struct_t.depth_curr.2 -= 1;

                        let last_curly_open = struct_t.last_open_pin.iter()
                            .filter(|sym| sym.0 == TokenType::CurlyOpen)
                            .last();
                        if last_curly_open.is_none() {
                            return Err("invalid json: missing opening curly brace");
                        }
                        let curly_open = last_curly_open.unwrap();
                        let (first, end) = token_pos(&token.buf)?;
                        let value = find_str(&mut seeker, curly_open.1, end);
                        println!("{:?}", value.unwrap());

                        struct_t.last_open_pin.pop();
                        schema = schema + "}";
                    }
                    TokenType::BracketOpen => {
                        struct_t.depth_curr.0 += 1;
                        struct_t.depth_curr.1 += 1;
                        struct_t.arr_idx.push(0);
                        let (first, end) = token_pos(&token.buf)?;
                        struct_t.last_open_pin.push((TokenType::BracketOpen, first));
                        schema = schema + "[";
                    }
                    TokenType::BracketClose => {
                        struct_t.depth_curr.0 -= 1;
                        struct_t.depth_curr.1 -= 1;

                        let last_bracket_open = struct_t.last_open_pin.iter()
                            .filter(|sym| sym.0 == TokenType::BracketOpen)
                            .last();
                        if last_bracket_open.is_none() {
                            return Err("invalid json: missing opening curly brace");
                        }
                        let curly_open = last_bracket_open.unwrap();
                        let (first, end) = token_pos(&token.buf)?;
                        let value = find_str(&mut seeker, curly_open.1, end);
                        println!("{:?}", value.unwrap());

                        struct_t.arr_idx.pop();
                        struct_t.last_open_pin.pop();
                        schema = schema + "]";
                    }
                    TokenType::Comma => {
                        let (token_type, _) = struct_t.last_open_pin.last().unwrap();
                        if struct_t.depth_curr.1 > -1                    // must be inside an array
                            && token_type.eq(&TokenType::BracketOpen)
                        {
                            let arr_idx_len = struct_t.arr_idx.len();
                            struct_t.arr_idx[arr_idx_len - 1] += 1;
                        }
                        schema = schema + ",";
                    }
                    TokenType::Colon => schema = schema + ":",
                    TokenType::BooleanTrue | TokenType::BooleanFalse => schema = schema + "bool",
                    TokenType::Number => schema = schema + "num",
                    TokenType::Null => schema = schema + "null",
                    _ => {}
                }

                if !struct_t.last_open_pin.is_empty() {
                    let (token_type, _) = struct_t.last_open_pin.last().unwrap();
                    if token_type.eq(&TokenType::BracketOpen) {
                        if token.kind == TokenType::String {
                            schema = schema + "string";
                        }
                    } else if token_type.eq(&TokenType::CurlyOpen)
                    {
                        if token.kind == TokenType::String && struct_t.last_token_key_delimiter {
                            let (first, end) = token_pos(&token.buf)?;
                            let key = find_str(
                                &mut seeker,
                                first + stream_t.last_stream_pos,
                                end + stream_t.last_stream_pos,
                            )
                                .unwrap();
                            schema = schema + &key;
                            struct_t.last_token_key_delimiter = false;
                        } else if token.kind == TokenType::String {
                            schema = schema + "string";
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
            if bytes_read < chunk_size {
                stream_t.buffer.drain(..=last_chunk.len() - 1);
            } else {
                stream_t.buffer.drain(..=last_chunk.len());
            }
        } else {
            panic!("Invalid chunk");
        }

        stream_t.last_stream_pos += stream_t.last_chunk_len as u64;
    }

    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn call(input: &str) -> Result<String, &'static str> {
        let mut reader = Cursor::new(input.as_bytes());
        let mut seeker = Cursor::new(input.as_bytes());
        simple_poc(&mut reader, &mut seeker)
    }

    #[test]
    fn it_works() {
        assert_eq!(call(r#"{"a":"b"}"#), Ok(r#"{"a":string}"#.to_string()));

        assert_eq!(
            call(r#"{"a":"b", "c":"d"}"#),
            Ok(r#"{"a":string,"c":string}"#.to_string())
        );

        assert_eq!(
            call(r#"{"a":"b", "c":"d", "e":[2,false,"bob"]}"#),
            Ok(r#"{"a":string,"c":string,"e":[num,bool,string]}"#.to_string())
        );

        assert_eq!(
            call(r#"{"a":"b", "c":"d", "e":[2,false,{"bob":{"f":"g"}}]}"#),
            Ok(r#"{"a":string,"c":string,"e":[num,bool,{"bob":{"f":string}}]}"#.to_string())
        );

        // test repeating patterns
        assert_eq!(call(r#"[{"a":"b"},{"a":"d"}]"#), Ok(r#"[{"a":string},{"a":string}]"#.to_string()));
        assert_eq!(call(r#"[1,2,4]"#), Ok(r#"[num,num,num]"#.to_string()));
    }
}
