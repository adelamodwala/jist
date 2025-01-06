use crate::utils::parse_search_key;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use log::debug;

mod buf_parser;
mod utils;
mod split_parser;
mod simd_parser;

pub fn search(
    haystack: Option<&str>,
    file: Option<&str>,
    search_key: &str,
    buff_size: Option<usize>,
    streaming: bool,
) -> Result<String, &'static str> {
    if (haystack.is_none() && file.is_none()) || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }
    if file.is_none() && haystack.unwrap().is_empty() {
        return Err("Invalid input - empty data");
    }
    if haystack.is_none() && file.unwrap().is_empty() {
        return Err("Invalid input - empty file path");
    }

    // If input file size is greater than 4.2GB, fallback to buffered search
    let mut stream_only = streaming;
    if file.is_some() {
        let f = File::open(file.unwrap()).unwrap();
        if f.metadata().unwrap().len() >= u32::MAX as u64 {
            debug!("file too large - fallback to char lexer");
            stream_only = true;
        }
    }
    if stream_only {
        debug!("stream only");
        return top_level_buf_search(haystack, file, search_key, buff_size);
    }

    match simd_parser::search(haystack, file, search_key) {
        Ok(result) => Ok(result),
        Err(code) => {
            if code.eq("JIST_ERROR_FILE_TOO_LARGE") {
                debug!("fallback to char lexer");
                return top_level_buf_search(haystack, file, search_key, buff_size);
            }
            Err(code)
        }
    }
}

fn top_level_buf_search(
    haystack: Option<&str>,
    file: Option<&str>,     // Keep this as Option<&str> for future flexibility with testing & dev
    search_key: &str,
    buff_size: Option<usize>,
) -> Result<String, &'static str> {
    let search_path = parse_search_key(search_key);
    if file.is_some() {
        let f = File::open(file.unwrap()).unwrap();
        let mut reader = BufReader::new(&f);
        let mut seeker = BufReader::new(&f);
        buf_parser::search(&mut reader, &mut seeker, &search_path, buff_size)
    } else {
        let haystack_str = haystack.unwrap();
        if haystack_str.is_empty() {
            return Err("Invalid input - empty data");
        }
        let mut reader = Cursor::new(haystack_str.as_bytes());
        let mut seeker = Cursor::new(haystack_str.as_bytes());
        buf_parser::search(&mut reader, &mut seeker, &search_path, buff_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data() {
        assert!(search(Some(""), None, "a", None, false).is_err());
        assert!(search(None, None, "a", None, false).is_err());
    }

    #[test]
    fn empty_search_key() {
        assert!(search(Some(r#"{"a": "b"}"#), None, "", None, false).is_err());
    }

    #[test]
    fn object_search() {
        assert_eq!(
            search(Some(r#"{"b":"c"}"#), None, "b", None, false),
            Ok("c".to_string())
        );
        assert_eq!(
            search(
                Some(r#"{"b": {"a":"d"},"a":{"b":{"c":"e"}}}"#),
                None,
                "a.b",
                None,
                false
            ),
            Ok(r#"{"c":"e"}"#.to_string())
        );
        assert_eq!(
            search(
                Some(
                    r#"
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
            ]"#
                ),
                None,
                "[2].a.b[1]",
                None,
                false
            ),
            Ok("3".to_string())
        );
    }

    #[test]
    fn array_search() {
        assert_eq!(
            search(Some(r#"[{"x": "y"}, {"p":"q"}]"#), None, "[1].p", None, false),
            Ok(r#"q"#.to_string())
        );
        assert_eq!(
            search(Some(r#"[{"x": "y"}, {"p":"\"q\""}]"#), None, "[1].p", None, false),
            Ok(r#"\"q\"#.to_string())
        );
    }

    #[test]
    fn array_only() {
        assert_eq!(
            search(Some(r#"[8,9,1]"#), None, "[0]", None, false),
            Ok("8".to_string())
        );
        assert_eq!(
            search(Some(r#"[8,9,1]"#), None, "[1]", None, false),
            Ok("9".to_string())
        );
        assert_eq!(
            search(Some(r#"[8,9,1]"#), None, "[2]", None, false),
            Ok("1".to_string())
        );

        assert_eq!(
            search(Some(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#), None, "[0]", None, false),
            Ok(r#"{"x":"y"}"#.to_string())
        );
        assert_eq!(
            search(Some(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#), None, "[1]", None, false),
            Ok(r#"{"a":{"b":"c"}}"#.to_string())
        );

        assert_eq!(
            search(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0][1][1]", None, false),
            Ok("7".to_string())
        );
        assert_eq!(
            search(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0][1]", None, false),
            Ok("[6,7]".to_string())
        );
        assert_eq!(
            search(Some(r#"[[3, [6,7],5],9,1]"#), None, "[2]", None, false),
            Ok("1".to_string())
        );
        assert_eq!(
            search(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0]", None, false),
            Ok("[3,[6,7],5]".to_string())
        );
    }

    #[test]
    fn object_only() {
        assert_eq!(
            search(Some(r#"{"x":"y"}"#), None, "x", None, false),
            Ok("y".to_string())
        );

        assert_eq!(
            search(Some(r#"{"x":{"y":"z"}}"#), None, "x.y", None, false),
            Ok("z".to_string())
        );
        assert_eq!(
            search(Some(r#"{"x":{"y":"z"}}"#), None, "x", None, false),
            Ok(r#"{"y":"z"}"#.to_string())
        );

        assert_eq!(
            search(Some(r#"{"x":["y"]}"#), None, "x", None, false),
            Ok(r#"["y"]"#.to_string())
        );
        assert_eq!(
            search(Some(r#"{"x":{"y":["z"]}}"#), None, "x", None, false),
            Ok(r#"{"y":["z"]}"#.to_string())
        );
        assert_eq!(
            search(Some(r#"{"x":{"y":["z"]}}"#), None, "x.y", None, false),
            Ok(r#"["z"]"#.to_string())
        );
        assert_eq!(
            search(Some(r#"{"x":{"y":["z"]}}"#), None, "x.y[0]", None, false),
            Ok("z".to_string())
        );
    }

    #[test]
    fn mixed() {
        let json1 = r#"
            [
                [-140.5405, [2, 3], "n", {"o": "p"}],
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
                {
                    "a": {
                        "x": "y\"b\"",
                        "b": [
                            2,
                            7,
                            4
                        ]
                    }
                },
                [
                    "d",
                    "e"
                ]
            ]"#;
        assert_eq!(
            search(Some(json1), None, "[3].a.b", None, false).unwrap(),
            "[2,7,4]".to_string()
        );
        assert_eq!(
            search(Some(json1), None, "[3].a.b[0]", None, false).unwrap(),
            "2".to_string()
        );
        assert_eq!(
            search(Some(json1), None, "[3].a", None, false).unwrap(),
            r#"{"x":"y\"b\"","b":[2,7,4]}"#.to_string()
        );

        assert_eq!(
            search(
                Some(
                    r#"
            {
                "a": {
                    "x": "y\"b\"",
                    "b": [
                        2,
                        3,
                        4
                    ]
                }
            }"#
                ),
                None,
                "a.b",
                None,
                false
            )
            .unwrap(),
            "[2,3,4]".to_string()
        );
    }

    #[test]
    fn sibling_keys() {
        let sample = r#"[
    {
        "id": "50b21bee-4198-4183-bb0f-597f3cd2e1bf",
        "name": "Test",
        "attributes": [
            {
                "shirt": "red",
                "pants": "black"
            },
            {
                "shirt": "yellow",
                "pants": "black"
            }
        ]
    },
    {
        "id": "a179bc6e-6976-4e89-99ce-08974b388d41",
        "name": "Test",
        "attributes": [
            {
                "shirt": "red",
                "pants": "blue"
            },
            {
                "shirt": "red",
                "pants": "orange"
            }
        ]
    },
    {
        "id": "fbfe64c0-2bf6-43b7-82d0-5e1594019597",
        "name": "Test",
        "attributes": [
            {
                "shirt": "yellow",
                "pants": "black"
            },
            {
                "shirt": "green",
                "pants": "orange"
            }
        ]
    }
]"#;
        assert_eq!(
            search(Some(sample), None, "[1].attributes[1].shirt", None, false).unwrap(),
            "red".to_string()
        );
    }
}
