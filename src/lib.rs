use crate::utils::parse_search_key;

mod parse_top_level;
mod parse_all;
mod utils;
mod buf_parser;

pub fn search(haystack: &str, search_key: &str) -> Result<String, &'static str> {
    if haystack.is_empty() || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }

    let search_path = parse_search_key(search_key.to_string());

    // parse_all::search(haystack, &search_path)
    parse_top_level::search(haystack, &search_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data() {
        assert!(search("", "a").is_err());
    }

    #[test]
    fn empty_search_key() {
        assert!(search(r#"{"a": "b"}"#, "").is_err());
    }

    #[test]
    fn object_search() {
        assert_eq!(search(r#"{"b":"c"}"#, "b"), Ok("c".to_string()));
        assert_eq!(search(r#"{"b": {"a":"d"},"a":{"b":{"c":"e"}}}"#, "a.b"), Ok(r#"{"c":"e"}"#.to_string()));
        assert_eq!(search(r#"
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
            ]"#, "[2].a.b[1]"), Ok("3".to_string()));
    }

    #[test]
    fn array_search() {
        assert_eq!(search(r#"[{"x": "y"}, {"p":"q"}]"#, "[1].p"), Ok(r#"q"#.to_string()));
        assert_eq!(search(r#"[{"x": "y"}, {"p":"\"q\""}]"#, "[1].p"), Ok(r#""q""#.to_string()));
    }
}
