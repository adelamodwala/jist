use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref ARRAY_REGEX: Regex = Regex::new(r"^\[(\d+)\]$").unwrap();
}

static JSON_SPACE: &str = " ";
static JSON_COLON: &str = ":";
static JSON_QUOTE: &str = "\"";
static JSON_COMMA: &str = ",";
static JSON_OPEN_BRACE: &str = "{";
static JSON_CLOSE_BRACE: &str = "}";
static JSON_OPEN_BRACKET: &str = "[";
static JSON_CLOSE_BRACKET: &str = "]";


pub fn search(haystack: String, search_key: String) -> Result<String, &'static str> {
    if haystack.is_empty() || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }

    let search_path = parse_search_key(search_key);
    // next need to iteratively parse haystack along search_path without parsing all of it
    let mut curr_char = String::new();
    let mut haystack_chars = haystack.chars();
    let haystack_size = haystack.len();
    for (path_pos, key) in search_path.iter().enumerate() {
        if key.starts_with(JSON_OPEN_BRACKET) {
            curr_char = haystack_chars.next().unwrap().to_string();
            if curr_char != JSON_OPEN_BRACKET {
                cancel_search_panic(&search_path, path_pos, haystack_chars.position().unwrap());
            }
            // TODO - arrays are broken by commas but each type needs to be understood in case there are
            // dictionaries or nested lists
            return Ok("NONE".to_string());
        }
        else {
            curr_char = haystack_chars.nth(0).unwrap().to_string();
            if curr_char != JSON_OPEN_BRACE {
                cancel_search_panic(&search_path, path_pos, idx);
            }
            // e.g. "a.b.c" searched on {"b": {"a":"d"},"a":{"b":{"c":"e"}}} - going to first "a" key won't work
            // Can slide to first "a" that is at the same depth by skipping over {} or []
            let mut depth = path_pos;
            let mut open_key = false;
            let mut key_found = false;
            let mut quoted_key = String::new();
            quoted_key.push_str(JSON_QUOTE);
            quoted_key.push_str(key);
            quoted_key.push_str(JSON_QUOTE);
            while haystack_chars.next < haystack_size {
                if curr_char == JSON_OPEN_BRACE || curr_char == JSON_OPEN_BRACKET {
                    depth += 1;
                    idx += 1;
                    continue;
                }
                else if curr_char == JSON_CLOSE_BRACE || curr_char == JSON_CLOSE_BRACKET {
                    depth -= 1;
                    if depth < path_pos {
                        cancel_search_panic(&search_path, path_pos, idx);
                    }

                    idx += 1;
                    continue;
                }
                else if curr_char == JSON_COLON || curr_char == JSON_COMMA {
                    idx += 1;
                    continue;
                }

                if depth > path_pos {
                    idx += 1;
                    continue;
                }

                if curr_char == JSON_QUOTE {
                    if open_key {
                        open_key = false;
                        idx += 1;
                        continue;
                    }
                    else {
                        open_key = true;
                        if quoted_key == &haystack[idx..key.len() + 1] {
                            // found the key - break!
                            key_found = true;
                            println!("key found: {}[{}]", key, idx);
                            return Ok(haystack[idx..idx + key.len() + 1].to_string());
                        }
                    }
                }

            }
            if (!key_found) {
                cancel_search_panic(&search_path, path_pos, depth);
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
    val
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
    fn object_key_found() {
        search(r#"{"b": {"a":"d"},"a":{"b":{"c":"e"}}}"#.to_string(), "a.b".to_string());
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
