use json_tools::Buffer;
use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};

lazy_static! {
    static ref ARRAY_REGEX: Regex = Regex::new(r"^\[(\d+)\]$").unwrap();
    static ref SPLIT_JSON_PATH_REGEX: Regex = Regex::new(r"\[(?:[^\[\]]*)\]|[^.\[\]]+").unwrap();
}

pub(crate) fn array_ind(accessor: &str) -> i64 {
    if !ARRAY_REGEX.is_match(accessor) {
        return -1;
    }

    let val: i64 = match accessor.split(&['[', ']'][..]).nth(1) {
        Some(n) => n.parse::<i64>().expect("not a valid array accessor"),
        None => -1,
    };
    val
}

pub fn parse_search_key(search_key: &str) -> Vec<String> {
    SPLIT_JSON_PATH_REGEX
        .find_iter(search_key)
        .map(|m| m.as_str().to_string())
        .collect()
}

pub(crate) fn token_pos(buf: &Buffer) -> Result<(u64, u64), &'static str> {
    let (first, end) = match buf {
        Buffer::Span(pos) => (pos.first, pos.end),
        _ => {
            return Err("error");
        }
    };
    Ok((first, end))
}

pub(crate) fn checkpoint_depth(search_path: &[String], idx: usize) -> (i32, i32, i32) {
    let search_array_nodes = search_path[..idx + 1]
        .iter()
        .filter(|x| x.starts_with("["))
        .count() as i32;
    let search_obj_nodes = idx as i32 + 1 - search_array_nodes;
    (idx as i32, search_array_nodes - 1, search_obj_nodes - 1)
}

pub(crate) fn sanitize_output(out: &str) -> String {
    let sanitized = out.trim().trim_start_matches("\"").trim_end_matches("\"");
    if sanitized.starts_with(['{', '[']) {
        let json: Value = serde_json::from_str(sanitized).expect("JSON parsing error");
        return json.to_string();
    }
    sanitized.to_string()
}

pub fn find_str<R: Read + Seek>(mut seeker: R, start: u64, end: u64) -> Option<String> {
    let mut buff = vec![0u8; end as usize - start as usize];
    seeker.seek(SeekFrom::Start(start)).expect("error");
    seeker.read_exact(&mut buff).expect("error");
    String::from_utf8(buff.clone()).ok()
}

pub fn is_ndjson(input: &str) -> bool {
    if input.starts_with("{") {
        match input.split_once("\n") {
            None => return true,
            Some((first, remaining)) => {
                if first.ends_with("}") {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn array_ind_test() {
        assert_eq!(array_ind("0"), -1);
        assert_eq!(array_ind("waef"), -1);
        assert_eq!(array_ind("[11]"), 11);
    }

    #[test]
    fn is_ndjson_test() {
        assert_eq!(is_ndjson("{}"), true);
        assert_eq!(is_ndjson(r#"{"a":"b"}
        {"a":"c"}"#), true);
        assert_eq!(is_ndjson(r#"[{"a":"b"},{"a":"c"}]"#), false);
        assert_eq!(is_ndjson(r#"[{"a":"b"},
        {"a":"c"}]"#), false);
    }

    #[test]
    fn parse_search_key_test() {
        assert_eq!(parse_search_key("myroot"), vec!["myroot"]);
        assert_eq!(parse_search_key("myroot.child1"), vec!["myroot", "child1"]);
        assert_eq!(
            parse_search_key("myroot.child1.grandchild1"),
            vec!["myroot", "child1", "grandchild1"]
        );
        assert_eq!(
            parse_search_key("myroot.child1[0]"),
            vec!["myroot", "child1", "[0]"]
        );
        assert_eq!(
            parse_search_key("myroot.child1[0].arr1"),
            vec!["myroot", "child1", "[0]", "arr1"]
        );
        assert_eq!(
            parse_search_key("[2].child1[0].arr1"),
            vec!["[2]", "child1", "[0]", "arr1"]
        );
        assert_eq!(
            parse_search_key("[1][1][1].b"),
            vec!["[1]", "[1]", "[1]", "b"]
        );
        assert_eq!(
            parse_search_key("x.y[1][1][1].b"),
            vec!["x", "y", "[1]", "[1]", "[1]", "b"]
        );
        assert_eq!(
            parse_search_key("x.y[1][1][1].b[1222][439834]"),
            vec!["x", "y", "[1]", "[1]", "[1]", "b", "[1222]", "[439834]"]
        );
    }

    #[test]
    fn checkpoint_depth_test() {
        assert_eq!(checkpoint_depth(&parse_search_key("a.b[1]"), 0), (0, -1, 0));
        assert_eq!(checkpoint_depth(&parse_search_key("a.b[1]"), 1), (1, -1, 1));
        assert_eq!(checkpoint_depth(&parse_search_key("a.b[1]"), 2), (2, 0, 1));

        assert_eq!(checkpoint_depth(&parse_search_key("[2]"), 0), (0, 0, -1));

        assert_eq!(checkpoint_depth(&vec!["a".to_string()], 0), (0, -1, 0));

        assert_eq!(
            checkpoint_depth(&parse_search_key("[1][1][1].b"), 0),
            (0, 0, -1)
        );
        assert_eq!(
            checkpoint_depth(&parse_search_key("[1][1][1].b"), 1),
            (1, 1, -1)
        );
        assert_eq!(
            checkpoint_depth(&parse_search_key("[1][1][1].b"), 2),
            (2, 2, -1)
        );
        assert_eq!(
            checkpoint_depth(&parse_search_key("[1][1][1].b"), 3),
            (3, 2, 0)
        );
    }
}
