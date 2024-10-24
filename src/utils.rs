use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};
use bgzip::BGZFReader;
use bgzip::read::IndexedBGZFReader;
use flate2::read::GzDecoder;
use json_tools::Buffer;
use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;

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

pub(crate) fn token_pos(buf: &Buffer) -> Result<(u64, u64), &'static str> {
    let (first, end) = match buf {
        Buffer::Span(pos) => (pos.first, pos.end),
        _ => { return Err("error"); }
    };
    Ok((first, end))
}

pub(crate) fn checkpoint_depth(search_path: &Vec<String>, idx: usize) -> (i32, i32, i32) {
    let search_array_nodes = search_path[..idx + 1].iter().filter(|x| x.starts_with("[")).count() as i32;
    let search_obj_nodes = idx as i32 + 1 - search_array_nodes;
    (
        idx as i32,
        search_array_nodes - 1,
        search_obj_nodes - 1
    )
}

pub(crate) fn sanitize_output(out: &str) -> String {
    let sanitized = out.trim().trim_start_matches("\"").trim_end_matches("\"");
    if sanitized.starts_with(&['{', '[']) {
        let json: Value = serde_json::from_str(sanitized).expect("JSON parsing error");
        return json.to_string();
    }
    sanitized.to_string()
}

pub(crate) fn is_gz(file: &str) -> bool {
    let mut buf = [0u8; 2];
    let f = File::open(file).unwrap();
    let mut reader = BufReader::new(f);
    reader.read_exact(&mut buf).unwrap();
    (buf[0] == 0x1f) && (buf[1] == 0x8b)
}

pub type BoxedReader = Box<dyn ReadSeek>;
pub trait ReadSeek: Read + Seek + BufRead {}
impl<T: Read + Seek + BufRead> ReadSeek for T {}
pub(crate) fn get_reader(file: &str) -> BoxedReader {
    if is_gz(file) {
        Box::new(IndexedBGZFReader::new(BGZFReader::new(File::open(file).unwrap()).unwrap(), Default::default()).unwrap())
    } else {
        Box::new(BufReader::new(File::open(file).unwrap()))
    }
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

    #[test]
    fn checkpoint_depth_test() {
        assert_eq!(checkpoint_depth(&parse_search_key("a.b[1]".to_string()), 0), (0, -1, 0));
        assert_eq!(checkpoint_depth(&parse_search_key("a.b[1]".to_string()), 1), (1, -1, 1));
        assert_eq!(checkpoint_depth(&parse_search_key("a.b[1]".to_string()), 2), (2, 0, 1));

        assert_eq!(checkpoint_depth(&parse_search_key("[2]".to_string()), 0), (0, 0, -1));

        assert_eq!(checkpoint_depth(&vec!["a".to_string()], 0), (0, -1, 0));

        assert_eq!(checkpoint_depth(&parse_search_key("[1][1][1].b".to_string()), 0), (0, 0, -1));
        assert_eq!(checkpoint_depth(&parse_search_key("[1][1][1].b".to_string()), 1), (1, 1, -1));
        assert_eq!(checkpoint_depth(&parse_search_key("[1][1][1].b".to_string()), 2), (2, 2, -1));
        assert_eq!(checkpoint_depth(&parse_search_key("[1][1][1].b".to_string()), 3), (3, 2, 0));
    }
}