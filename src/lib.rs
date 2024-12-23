use crate::utils::parse_search_key;
use std::fs::File;
use std::io::{BufReader, Cursor};

mod buf_parser;
mod utils;
mod split_parser;

pub fn search(
    haystack: Option<&str>,
    file: Option<&str>,
    search_key: &str,
    buff_size: Option<usize>,
) -> Result<String, &'static str> {
    if (haystack.is_none() && file.is_none()) || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }

    let search_path = parse_search_key(search_key);

    top_level_buf_search(haystack, file, &search_path, buff_size)
}

fn top_level_buf_search(
    haystack: Option<&str>,
    file: Option<&str>,
    search_path: &[String],
    buff_size: Option<usize>,
) -> Result<String, &'static str> {
    if file.is_some() {
        let f = File::open(file.unwrap()).unwrap();
        let mut reader = BufReader::new(&f);
        let mut seeker = BufReader::new(&f);
        buf_parser::search(&mut reader, &mut seeker, search_path, buff_size)
    } else {
        let haystack_str = haystack.unwrap();
        if haystack_str.is_empty() {
            return Err("Invalid input - empty data");
        }
        let mut reader = Cursor::new(haystack_str.as_bytes());
        let mut seeker = Cursor::new(haystack_str.as_bytes());
        buf_parser::search(&mut reader, &mut seeker, search_path, buff_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data() {
        assert!(search(Some(""), None, "a", None).is_err());
        assert!(search(None, None, "a", None).is_err());
    }

    #[test]
    fn empty_search_key() {
        assert!(search(Some(r#"{"a": "b"}"#), None, "", None).is_err());
    }

    #[test]
    fn object_search() {
        assert_eq!(
            search(Some(r#"{"b":"c"}"#), None, "b", None),
            Ok("c".to_string())
        );
        assert_eq!(
            search(
                Some(r#"{"b": {"a":"d"},"a":{"b":{"c":"e"}}}"#),
                None,
                "a.b",
                None
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
                None
            ),
            Ok("3".to_string())
        );
    }

    #[test]
    fn array_search() {
        assert_eq!(
            search(Some(r#"[{"x": "y"}, {"p":"q"}]"#), None, "[1].p", None),
            Ok(r#"q"#.to_string())
        );
        assert_eq!(
            search(Some(r#"[{"x": "y"}, {"p":"\"q\""}]"#), None, "[1].p", None),
            Ok(r#"\"q\"#.to_string())
        );
    }

    #[test]
    fn array_only() {
        assert_eq!(
            search(Some(r#"[8,9,1]"#), None, "[0]", None),
            Ok("8".to_string())
        );
        assert_eq!(
            search(Some(r#"[8,9,1]"#), None, "[1]", None),
            Ok("9".to_string())
        );
        assert_eq!(
            search(Some(r#"[8,9,1]"#), None, "[2]", None),
            Ok("1".to_string())
        );

        assert_eq!(
            search(Some(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#), None, "[0]", None),
            Ok(r#"{"x":"y"}"#.to_string())
        );
        assert_eq!(
            search(Some(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#), None, "[1]", None),
            Ok(r#"{"a":{"b":"c"}}"#.to_string())
        );

        assert_eq!(
            search(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0][1][1]", None),
            Ok("7".to_string())
        );
        assert_eq!(
            search(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0][1]", None),
            Ok("[6,7]".to_string())
        );
        assert_eq!(
            search(Some(r#"[[3, [6,7],5],9,1]"#), None, "[2]", None),
            Ok("1".to_string())
        );
        assert_eq!(
            search(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0]", None),
            Ok("[3,[6,7],5]".to_string())
        );
    }

    #[test]
    fn object_only() {
        assert_eq!(
            search(Some(r#"{"x":"y"}"#), None, "x", None),
            Ok("y".to_string())
        );

        assert_eq!(
            search(Some(r#"{"x":{"y":"z"}}"#), None, "x.y", None),
            Ok("z".to_string())
        );
        assert_eq!(
            search(Some(r#"{"x":{"y":"z"}}"#), None, "x", None),
            Ok(r#"{"y":"z"}"#.to_string())
        );

        assert_eq!(
            search(Some(r#"{"x":["y"]}"#), None, "x", None),
            Ok(r#"["y"]"#.to_string())
        );
        assert_eq!(
            search(Some(r#"{"x":{"y":["z"]}}"#), None, "x", None),
            Ok(r#"{"y":["z"]}"#.to_string())
        );
        assert_eq!(
            search(Some(r#"{"x":{"y":["z"]}}"#), None, "x.y", None),
            Ok(r#"["z"]"#.to_string())
        );
        assert_eq!(
            search(Some(r#"{"x":{"y":["z"]}}"#), None, "x.y[0]", None),
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
            search(Some(json1), None, "[3].a.b", None).unwrap(),
            "[2,7,4]".to_string()
        );
        assert_eq!(
            search(Some(json1), None, "[3].a.b[0]", None).unwrap(),
            "2".to_string()
        );
        assert_eq!(
            search(Some(json1), None, "[3].a", None).unwrap(),
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
                None
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
            search(Some(sample), None, "[1].attributes[1].shirt", None).unwrap(),
            "red".to_string()
        );
    }
}
