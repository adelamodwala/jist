use std::fs::File;
use std::io::{BufReader, Cursor};
use crate::utils::parse_search_key;

mod parse_top_level;
mod parse_all;
mod utils;
mod buf_parser;

pub fn search(haystack: Option<&str>, file: Option<&str>, search_key: &str) -> Result<String, &'static str> {
    if (haystack.is_none() && file.is_none()) || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }

    let search_path = parse_search_key(search_key.to_string());

    // parse_all::search(haystack, &search_path)
    // parse_top_level::search(haystack, &search_path)

    if file.is_some() {
        let f = File::open(file.unwrap()).unwrap();
        let mut reader = BufReader::new(&f);
        let mut seeker = BufReader::new(&f);
        buf_parser::search(&mut reader, &mut seeker, &search_path)
    } else {
        let haystack_str = haystack.unwrap();
        if haystack_str.is_empty() {
            return Err("Invalid input - empty data");
        }
        let mut reader = Cursor::new(haystack_str.as_bytes());
        let mut seeker = Cursor::new(haystack_str.as_bytes());
        buf_parser::search(&mut reader, &mut seeker, &search_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data() {
        assert!(search(Some(""), None, "a").is_err());
        assert!(search(None, None, "a").is_err());
    }

    #[test]
    fn empty_search_key() {
        assert!(search(Some(r#"{"a": "b"}"#), None, "").is_err());
    }

    #[test]
    fn object_search() {
        assert_eq!(search(Some(r#"{"b":"c"}"#), None, "b"), Ok("c".to_string()));
        assert_eq!(search(Some(r#"{"b": {"a":"d"},"a":{"b":{"c":"e"}}}"#), None, "a.b"), Ok(r#"{"c":"e"}"#.to_string()));
        assert_eq!(search(Some(r#"
            [
                [1, [2, 3], "n", {"o": "p"}],
                "c",
                {
                    "a": {
                        "x": "y\"b\"",
                        "b": [
                            2,
                            3,
                            4
                        ]
                    }
                },
                [
                    "d",
                    "e"
                ]
            ]"#), None, "[2].a.b[1]"), Ok("3".to_string()));
    }

    #[test]
    fn array_search() {
        assert_eq!(search(Some(r#"[{"x": "y"}, {"p":"q"}]"#), None, "[1].p"), Ok(r#"q"#.to_string()));
        assert_eq!(search(Some(r#"[{"x": "y"}, {"p":"\"q\""}]"#), None, "[1].p"), Ok(r#"\"q\"#.to_string()));
    }
}
