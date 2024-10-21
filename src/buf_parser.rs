use crate::utils::parse_search_key;
use json_tools::{Buffer, BufferType, Lexer, TokenType};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};

fn token_pos(buf: &Buffer) -> Result<(u64, u64), &'static str> {
    let (first, end) = match buf {
        Buffer::Span(pos) => (pos.first, pos.end),
        _ => { return Err("error"); }
    };
    Ok((first, end))
}

fn tester() {
    let f = File::open("json_test/test_small.json").unwrap();
    let search_path = parse_search_key("[100000].attributes[1].shirt".to_string());
    let mut reader = BufReader::new(f);

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line.len() == 0 {
            break;
        }
        let mut token_iter = Lexer::new(line.bytes(), BufferType::Span).peekable();

        while token_iter.peek().is_some() {
            let token = token_iter.next().unwrap();
            let (first, end) = token_pos(&token.buf).unwrap();
            let pos = reader.stream_position().unwrap();
            println!("{:?}, pos: {:?}", token, (first + pos, end + pos)); This is wrong - position will be end of line read
        }

        println!("page finished - stream_position: {:?}", reader.stream_position().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        tester();
    }
}