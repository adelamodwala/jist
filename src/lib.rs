use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;

lazy_static! {
    static ref ARRAY_REGEX: Regex = Regex::new(r"^\[(\d+)\]$").unwrap();
}

pub fn search(haystack: &str, search_key: &str) -> Result<String, &'static str> {
    if haystack.is_empty() || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }

    let search_path = parse_search_key(search_key.to_string());
    let json: Value = serde_json::from_str(haystack).expect("Invalid JSON");

    // collect
    let mut path_val = &json;
    for (path_pos, key) in search_path.iter().enumerate() {
        let idx = array_ind(key.to_string());
        if idx > -1 {
            path_val = &path_val[idx.unsigned_abs() as usize];
        } else {
            path_val = &path_val[key];
        }
    }

    // If value is a string, need to treat it as_str to avoid adding surrounding '"'
    if path_val.is_string() {
        return Ok(path_val.as_str().unwrap().to_string());
    }
    Ok(path_val.to_string())
}

fn cancel_search_panic(search_path: &Vec<String>, path_pos_reached: usize, idx: usize) {
    panic!("{}", format!(r"cancelled search:
    path_reached: {}
    haystack[idx]: {}", search_path[..=path_pos_reached].join("."), idx.to_string()));
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

fn array_ind(accessor: String) -> i64 {
    if !ARRAY_REGEX.is_match(accessor.as_str()) {
        return -1;
    }

    let val: i64 = match accessor.split(&['[', ']'][..]).nth(1) {
        Some(n) => n.parse::<i64>().expect("not a valid array accessor"),
        None => -1,
    };
    val
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

    #[test]
    fn array_ind_test() {
        assert_eq!(array_ind("0".to_string()), -1);
        assert_eq!(array_ind("waef".to_string()), -1);
        assert_eq!(array_ind("[11]".to_string()), 11);
    }
}
