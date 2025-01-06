use json_tools::{BufferType, Lexer, Token};
use log::debug;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::absolute;
use std::thread;
use crate::utils::token_pos;
use md5;
use md5::Digest;

fn simple_poc() {
    let f = File::open("output.json").unwrap();
    let mut reader = BufReader::new(&f);
    let chunk_size = 1_000;
    let mut stream_t = crate::buf_parser::StreamTracker::new(chunk_size);

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

            // Process chunk here
            let mut token_iter = Lexer::new(stream_t.chunk.clone(), BufferType::Span).peekable();
            loop {
                let token_opt = token_iter.next();
                if token_opt.is_none() {
                    break;
                }

                let token = token_opt.unwrap();
                let (start, end) = token_pos(&token.buf).unwrap();
                compute((start, end));

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
}

fn split_poc() {
    let (sender, receiver) = std::sync::mpsc::channel();

    let producer_thread = thread::spawn(move || {
        let f = File::open("output.json").unwrap();
        let mut reader = BufReader::new(&f);
        let chunk_size = 1_000;
        let mut stream_t = crate::buf_parser::StreamTracker::new(chunk_size);

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

                // Process chunk here
                let mut token_iter = Lexer::new(stream_t.chunk.clone(), BufferType::Span).peekable();
                loop {
                    let token_opt = token_iter.next();
                    if token_opt.is_none() {
                        break;
                    }

                    let token = token_opt.unwrap();
                    let (start, end) = token_pos(&token.buf).unwrap();

                    sender.send((start, end)).unwrap();

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
    });

    for received in receiver {
        compute(received);
    }

    // Wait for the producer thread to finish
    producer_thread.join().expect("TODO: panic message");
}

fn compute((start, end): (u64, u64)) -> Digest {
    let total = start + end;
    md5::compute(format!("{total}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    // #[test]
    fn it_works() {
        let start_time = Instant::now();
        split_poc();
        let elapsed = Instant::now().duration_since(start_time).as_millis();
        debug!("split_poc time: {:.2?}", elapsed);

        let start_time = Instant::now();
        simple_poc();
        let elapsed = Instant::now().duration_since(start_time).as_millis();
        debug!("simple_poc time: {:.2?}", elapsed);
    }
}
