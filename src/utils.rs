use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref ARRAY_REGEX: Regex = Regex::new(r"^\[(\d+)\]$").unwrap();
    static ref SPLIT_JSON_PATH_REGEX: Regex = Regex::new(r"\[(?:[^\[\]]*)\]|[^.\[\]]+").unwrap();
}

pub(crate) fn array_ind(accessor: String) -> i64 {
    if !ARRAY_REGEX.is_match(accessor.as_str()) {
        return -1;
    }

    let val: i64 = match accessor.split(&['[', ']'][..]).nth(1) {
        Some(n) => n.parse::<i64>().expect("not a valid array accessor"),
        None => -1,
    };
    val
}

pub(crate) fn parse_search_key(search_key: String) -> Vec<String> {
    SPLIT_JSON_PATH_REGEX.find_iter(search_key.as_str())
        .map(|m| m.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn array_ind_test() {
        assert_eq!(array_ind("0".to_string()), -1);
        assert_eq!(array_ind("waef".to_string()), -1);
        assert_eq!(array_ind("[11]".to_string()), 11);
    }

    #[test]
    fn parse_search_key_test() {
        assert_eq!(parse_search_key("myroot".to_string()), vec!["myroot"]);
        assert_eq!(parse_search_key("myroot.child1".to_string()), vec!["myroot", "child1"]);
        assert_eq!(parse_search_key("myroot.child1.grandchild1".to_string()), vec!["myroot", "child1", "grandchild1"]);
        assert_eq!(parse_search_key("myroot.child1[0]".to_string()), vec!["myroot", "child1", "[0]"]);
        assert_eq!(parse_search_key("myroot.child1[0].arr1".to_string()), vec!["myroot", "child1", "[0]", "arr1"]);
        assert_eq!(parse_search_key("[2].child1[0].arr1".to_string()), vec!["[2]", "child1", "[0]", "arr1"]);
        assert_eq!(parse_search_key("[1][1][1].b".to_string()), vec!["[1]", "[1]", "[1]", "b"]);
        assert_eq!(parse_search_key("x.y[1][1][1].b".to_string()), vec!["x", "y","[1]", "[1]", "[1]", "b"]);
        assert_eq!(parse_search_key("x.y[1][1][1].b[1222][439834]".to_string()), vec!["x", "y","[1]", "[1]", "[1]", "b", "[1222]", "[439834]"]);
    }
}