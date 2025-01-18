use crate::model::j_struct_tracker::JStructTracker;
use crate::model::stream_tracker::StreamTracker;
use crate::utils::token_pos;
use json_tools::{BufferType, Lexer, Token, TokenType};
use log::debug;
use md5;
use md5::Digest;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::absolute;
use std::thread;

fn simple_poc<R: Read + Seek + BufRead>(mut reader: R, mut seeker: R) {
    let chunk_size = 1_000;
    let mut stream_t = StreamTracker::new(chunk_size);
    let mut struct_t = JStructTracker::init();
    let mut last_token;

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
                        print!("{{");
                    }
                    TokenType::CurlyClose => {
                        print!("}}");
                    }
                    TokenType::BracketOpen => {
                        print!("[");
                    }
                    TokenType::BracketClose => {
                        print!("]");
                    }
                    TokenType::Comma => {
                        print!(",");
                    }
                    TokenType::Colon => {
                        print!(":");
                    }
                    _ => {}
                }

                if token_iter.peek().is_none() {
                    break;
                }

                last_token = &token;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::Instant;

    #[test]
    fn it_works() {
        let start_time = Instant::now();
        let haystack_str = r#"{"a":"b"}"#;
        let mut reader = Cursor::new(haystack_str.as_bytes());
        let mut seeker = Cursor::new(haystack_str.as_bytes());
        simple_poc(&mut reader, &mut seeker);
        let elapsed = Instant::now().duration_since(start_time).as_millis();
        println!("simple_poc time: {:.2?}", elapsed);
    }
}
