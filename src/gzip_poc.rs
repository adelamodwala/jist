use flate2::read::{GzDecoder, MultiGzDecoder};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use bgzip::BGZFReader;
use bgzip::index::BGZFIndex;
use bgzip::read::IndexedBGZFReader;
use log::debug;

fn is_gz(file: &str) -> bool {
    let mut buf = [0u8; 2];
    let f = File::open(file).unwrap();
    let mut reader = BufReader::new(f);
    reader.read_exact(&mut buf).unwrap();
    (buf[0] == 0x1f) && (buf[1] == 0x8b)
}

fn test() {
    let mut last_stream_pos = 0;
    let mut last_chunk_len = 0;
    let chunk_size = 100;
    let mut buffer = Vec::with_capacity(chunk_size * 2); // Extra space for overflow
    let mut chunk = Vec::new();
    // let file = File::open("json_test/test_small.json").expect("Ooops.");
    // let mut reader = BufReader::new(file);
    let file = File::open("json_test/test_small.json.bgz").expect("Ooops.");
    let mut reader = IndexedBGZFReader::new(BGZFReader::new(file).unwrap(), BGZFIndex::default()).unwrap();


    loop {
        let bytes_read = reader.by_ref().take(chunk_size as u64).read_to_end(&mut buffer).unwrap();
        if bytes_read == 0 && buffer.is_empty() {
            debug!("Empty buffer");
        }

        // Find the last line break (add one if last buffer read)
        let mut chunk_str = String::from_utf8(buffer.to_vec()).unwrap();
        if bytes_read < chunk_size {
            chunk_str.push('\n');
        }
        if let Some(last_chunk_tup) = chunk_str.rsplit_once("\n") {
            let last_chunk = last_chunk_tup.0;
            debug!("last_chunk: {}", last_chunk);
            // Process the chunk that ends with a newline
            chunk.extend_from_slice(last_chunk.as_bytes());
            chunk.push(b'\n');
            last_chunk_len = chunk.len();

            debug!("chunk: {}", chunk_str);

            if buffer.len() < last_chunk_len {
                break;
            }

            // Clear chunk for next iteration
            chunk.clear();
            // Remove processed data from buffer
            buffer.drain(..last_chunk.len() + 1);
        }

        last_stream_pos += last_chunk_len as u64;
        debug!("page finished - stream_position: {:?}", last_stream_pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::info;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }
    #[test]
    fn tester() {
        init();
        test();
    }
}