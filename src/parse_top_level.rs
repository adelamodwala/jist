use std::fmt::Debug;
use json_tools::BufferType;
use json_tools::Lexer;

fn parse(data: &str) -> Result<String, &'static str> {

    for token in Lexer::new(data.bytes(), BufferType::Span) {
        println!("{:?}", token);
    }

    Ok("Done".to_string())
}

pub fn search(haystack: &str, search_path: &Vec<String>) -> Result<String, &'static str> {
    let token_iter = Lexer::new(haystack.bytes(), BufferType::Span);
    for token in token_iter {
        print!("{:?}", token.kind);
    }

    Ok("Done".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse() {
        parse(r#"{"foo":[1,"top",4],"bar":{"a":"b"}}"#);
    }
}