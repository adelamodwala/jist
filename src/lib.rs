mod parse_top_level;
mod parse_all;
mod utils;

pub fn search(haystack: &str, search_key: &str) -> Result<String, &'static str> {
    if haystack.is_empty() || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }

    let search_path = parse_search_key(search_key.to_string());

    parse_all::search(haystack, &search_path)
    // parse_top_level::search(haystack, &search_path)
}

fn parse_search_key(search_key: String) -> Vec<String> {
    search_key.split(&['.'][..]).map(|s| {
        let s = match s.find("[") {
            Some(i) => s.split_at(i),
            None => (s, ""),
        };
        vec![s.0, s.1]
    }).flatten().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
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

    #[test]
    fn parse_search_key_test() {
        assert_eq!(parse_search_key("myroot".to_string()), vec!["myroot"]);
        assert_eq!(parse_search_key("myroot.child1".to_string()), vec!["myroot", "child1"]);
        assert_eq!(parse_search_key("myroot.child1.grandchild1".to_string()), vec!["myroot", "child1", "grandchild1"]);
        assert_eq!(parse_search_key("myroot.child1[0]".to_string()), vec!["myroot", "child1", "[0]"]);
        assert_eq!(parse_search_key("myroot.child1[0].arr1".to_string()), vec!["myroot", "child1", "[0]", "arr1"]);
        assert_eq!(parse_search_key("[2].child1[0].arr1".to_string()), vec!["[2]", "child1", "[0]", "arr1"]);
    }
}
