use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref ARRAY_REGEX: Regex = Regex::new(r"^\[(\d+)\]$").unwrap();
}

pub fn search(haystack: String, search_key: String) -> Result<String, &'static str> {
    if haystack.is_empty() || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }

    let search_path = parse_search_key(search_key);
    // next need to iteratively parse haystack along search_path without parsing all of it
    let mut idx = 0;
    let mut haystack_chars = haystack.chars();
    for (path_pos, key) in search_path.iter().enumerate() {
        if key.starts_with("[") {
            if haystack_chars.nth(idx).unwrap().to_string() != "[" {
                cancel_search_panic(&search_path, path_pos, idx);
            }

        }
    }

    Ok("done".to_string())
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
        return -1
    }

    let val: i64 = match accessor.split(&['[', ']'][..]).nth(1) {
        Some(n) => n.parse::<i64>().expect("not a valid array accessor"),
        None => -1,
    };
    return val;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data() {
        assert!(search("".to_string(), "a".to_string()).is_err());
    }

    #[test]
    fn empty_search_key() {
        assert!(search(r#"{"a": "b"}"#.to_string(), "".to_string()).is_err());
    }

    #[test]
    #[should_panic]
    fn mismatched_search_key_no_array() {
        let _ = search(r#"{"a": "b"}"#.to_string(), "[0].a".to_string());
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
