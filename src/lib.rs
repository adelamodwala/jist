use std::io::Read;

pub mod buf_parser;
pub mod simd_parser;
pub mod utils;

#[cfg(test)]
mod tests {
    use super::*;
    static PARSERS: &[fn(Option<&str>, Option<&str>, &str) -> Result<String, &'static str>] =
        &[simd_parser::search, buf_parser::search];

    #[test]
    fn empty_data() {
        for search_fn in PARSERS {
            assert!(search_fn(Some(""), None, "a").is_err());
            assert!(search_fn(None, None, "a").is_err());
        }
    }

    #[test]
    fn empty_search_key() {
        for search_fn in PARSERS {
            assert!(search_fn(Some(r#"{"a": "b"}"#), None, "").is_err());
        }
    }

    #[test]
    fn object_search() {
        for search_fn in PARSERS {
            assert_eq!(
                search_fn(Some(r#"{"b":"c"}"#), None, "b"),
                Ok("c".to_string())
            );
            assert_eq!(
                search_fn(Some(r#"{"b": {"a":"d"},"a":{"b":{"c":"e"}}}"#), None, "a.b"),
                Ok(r#"{"c":"e"}"#.to_string())
            );
            assert_eq!(
                search_fn(
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
                    "[2].a.b[1]"
                ),
                Ok("3".to_string())
            );
        }
    }

    #[test]
    fn array_search() {
        for search_fn in PARSERS {
            assert_eq!(
                search_fn(Some(r#"[{"x": "y"}, {"p":"q"}]"#), None, "[1].p"),
                Ok(r#"q"#.to_string())
            );
            assert_eq!(
                search_fn(Some(r#"[{"x": "y"}, {"p":"\"q\""}]"#), None, "[1].p"),
                Ok(r#"\"q\"#.to_string())
            );
        }
    }

    #[test]
    fn array_only() {
        for search_fn in PARSERS {
            assert_eq!(
                search_fn(Some(r#"[8,9,1]"#), None, "[0]"),
                Ok("8".to_string())
            );
            assert_eq!(
                search_fn(Some(r#"[8,9,1]"#), None, "[1]"),
                Ok("9".to_string())
            );
            assert_eq!(
                search_fn(Some(r#"[8,9,1]"#), None, "[2]"),
                Ok("1".to_string())
            );

            assert_eq!(
                search_fn(Some(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#), None, "[0]"),
                Ok(r#"{"x":"y"}"#.to_string())
            );
            assert_eq!(
                search_fn(Some(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#), None, "[1]"),
                Ok(r#"{"a":{"b":"c"}}"#.to_string())
            );

            assert_eq!(
                search_fn(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0][1][1]"),
                Ok("7".to_string())
            );
            assert_eq!(
                search_fn(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0][1]"),
                Ok("[6,7]".to_string())
            );
            assert_eq!(
                search_fn(Some(r#"[[3, [6,7],5],9,1]"#), None, "[2]"),
                Ok("1".to_string())
            );
            assert_eq!(
                search_fn(Some(r#"[[3, [6,7],5],9,1]"#), None, "[0]"),
                Ok("[3,[6,7],5]".to_string())
            );
        }
    }

    #[test]
    fn object_only() {
        for search_fn in PARSERS {
            assert_eq!(
                search_fn(Some(r#"{"x":"y"}"#), None, "x"),
                Ok("y".to_string())
            );

            assert_eq!(
                search_fn(Some(r#"{"x":{"y":"z"}}"#), None, "x.y"),
                Ok("z".to_string())
            );
            assert_eq!(
                search_fn(Some(r#"{"x":{"y":"z"}}"#), None, "x"),
                Ok(r#"{"y":"z"}"#.to_string())
            );

            assert_eq!(
                search_fn(Some(r#"{"x":["y"]}"#), None, "x"),
                Ok(r#"["y"]"#.to_string())
            );
            assert_eq!(
                search_fn(Some(r#"{"x":{"y":["z"]}}"#), None, "x"),
                Ok(r#"{"y":["z"]}"#.to_string())
            );
            assert_eq!(
                search_fn(Some(r#"{"x":{"y":["z"]}}"#), None, "x.y"),
                Ok(r#"["z"]"#.to_string())
            );
            assert_eq!(
                search_fn(Some(r#"{"x":{"y":["z"]}}"#), None, "x.y[0]"),
                Ok("z".to_string())
            );
        }
    }

    #[test]
    fn mixed() {
        for search_fn in PARSERS {
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
                search_fn(Some(json1), None, "[3].a.b").unwrap(),
                "[2,7,4]".to_string()
            );
            assert_eq!(
                search_fn(Some(json1), None, "[3].a.b[0]").unwrap(),
                "2".to_string()
            );
            assert_eq!(
                search_fn(Some(json1), None, "[3].a").unwrap(),
                r#"{"x":"y\"b\"","b":[2,7,4]}"#.to_string()
            );

            assert_eq!(
                search_fn(
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
                    "a.b"
                )
                .unwrap(),
                "[2,3,4]".to_string()
            );
        }
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
        for search_fn in PARSERS {
            assert_eq!(
                search_fn(Some(sample), None, "[1].attributes[1].shirt").unwrap(),
                "red".to_string()
            );
        }
    }
}
