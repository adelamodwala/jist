use crate::utils::array_ind;
use json_tools::{Buffer, BufferType, Lexer, TokenType};
use std::cmp::Ordering;

fn token_pos(buf: &Buffer) -> Result<(u64, u64), &'static str> {
    let (first, end) = match buf {
        Buffer::Span(pos) => (pos.first, pos.end),
        _ => { return Err("error"); }
    };
    Ok((first, end))
}

fn checkpoint_depth(search_path: &Vec<String>, idx: usize) -> (i32, i32, i32) {
    let search_array_nodes = search_path[..idx + 1].iter().filter(|x| x.starts_with("[")).count() as i32;
    let search_obj_nodes = idx as i32 + 1 - search_array_nodes;
    (
        idx as i32,
        search_array_nodes - 1,
        search_obj_nodes - 1
    )
}

pub fn search(haystack: &str, search_path: &Vec<String>) -> Result<String, &'static str> {
    println!("--- Searching for {} on path: {:?}", haystack, search_path);

    let mut token_iter = Lexer::new(haystack.bytes(), BufferType::Span).peekable();

    // tuple of (depth, arr_depth, obj_depth)
    let mut depth_curr: (i32, i32, i32) = (-1, -1, -1);

    // keep track of array indices if currently inside array
    let mut arr_idx = Vec::new();
    let mut last_open: Vec<TokenType> = Vec::new();
    let mut checkpoint_start = Vec::new();

    // build checkpoints that must pass
    let mut checkpoints = Vec::new();
    for search_idx in 0..search_path.len() {
        checkpoints.push(checkpoint_depth(search_path, search_idx))
    }
    checkpoints.reverse();

    let mut arr_tgt: Vec<i64> = search_path.iter()
        .filter(|x| x.starts_with("["))
        .map(|x| array_ind(x.clone()))
        .rev()
        .collect();
    let arr_tgt_size = arr_tgt.len();
    let mut search_keys = search_path.iter()
        .filter(|x| !x.starts_with("["))
        .map(|x| x.clone())
        .rev()
        .collect::<Vec<String>>();
    println!("checkpoints: {:?}, arr_tgt: {:?}, search_keys: {:?}", checkpoints, arr_tgt, search_keys);

    loop {
        let mut token = token_iter.next().unwrap();
        let arr_idx_len = arr_idx.len();
        match token.kind {
            TokenType::CurlyOpen => {
                depth_curr.0 += 1;
                depth_curr.2 += 1;
                last_open.push(TokenType::CurlyOpen);
            }
            TokenType::CurlyClose => {
                depth_curr.0 -= 1;
                depth_curr.2 -= 1;
                last_open.pop();
            }
            TokenType::BracketOpen => {
                depth_curr.0 += 1;
                depth_curr.1 += 1;
                arr_idx.push(0);
                last_open.push(TokenType::BracketOpen);
            }
            TokenType::BracketClose => {
                depth_curr.0 -= 1;
                depth_curr.1 -= 1;
                arr_idx.pop();
                last_open.pop();
            }
            TokenType::Comma => {
                if depth_curr.1 > -1                    // must be inside an array
                    && *last_open.last().unwrap() == TokenType::BracketOpen
                {
                    arr_idx[arr_idx_len - 1] += 1;
                }
            }
            _ => {}
        }
        println!("depth_curr: {:?}, arr_idx: {:?}, search_keys: {:?}, kind: {:?}, last_open: {:?}, checkpoints: {:?}, checkpoint_start: {:?}", depth_curr, arr_idx, search_keys, &token.kind, last_open, checkpoints, checkpoint_start);

        if depth_curr.cmp(&checkpoints.last().unwrap()) == Ordering::Equal {
            // check if inside an array and iterating to find the expected place
            if *last_open.last().unwrap() == TokenType::BracketOpen && arr_idx.last().unwrap().cmp(&arr_tgt.last().unwrap()) == Ordering::Equal {
                let (first, end) = token_pos(&token.buf)?;

                // terminal point
                if checkpoints.len() == 1 && checkpoint_start.len() == arr_tgt_size {
                    println!("[checkpoint ended]");

                    // add 1 to starting index to exclude commas or brackets
                    return Ok(haystack[checkpoint_start.last().unwrap() + 1..(end as usize)].trim().trim_start_matches("\"").trim_end_matches("\"").to_string());
                } else {
                    println!("[checkpoint started]");

                    checkpoint_start.push(first as usize);
                    if arr_tgt.len() > 1 {
                        arr_tgt.pop();
                    }
                }

                if checkpoints.len() > 1 {
                    checkpoints.pop();
                }
            } else if *last_open.last().unwrap() == TokenType::CurlyOpen {
                if (token.kind == TokenType::CurlyOpen || token.kind == TokenType::Comma) && search_keys.len() > 0 {
                    if token.kind == TokenType::CurlyOpen { println!("\\checkpoint started\\") }

                    // next token should be a key
                    token = token_iter.next().expect("incomplete json object");
                    let (first, end) = token_pos(&token.buf)?;
                    let key = haystack[first as usize..end as usize].trim_start_matches("\"").trim_end_matches("\"");
                    if key.cmp(&search_keys.last().unwrap()) == Ordering::Equal {
                        println!(">>>found next key: {:?}", key);
                        search_keys.pop();
                    }

                    if checkpoints.len() > 1 {
                        checkpoints.pop();
                    }
                } else if search_keys.len() == 0 && checkpoints.len() == 1 {
                    let (first, end) = token_pos(&token.buf)?;
                    match token.kind {
                        TokenType::String | TokenType::Number | TokenType::BooleanFalse | TokenType::BooleanTrue | TokenType::Null => {
                            return Ok(haystack[first as usize..(end as usize)].trim().trim_start_matches("\"").trim_end_matches("\"").to_string());
                        }
                        TokenType::Colon => {
                            checkpoint_start.push(first as usize);
                        }
                        TokenType::CurlyClose | TokenType::BracketClose => {
                            return Ok(haystack[checkpoint_start.last().unwrap() + 1..(end as usize)].trim().to_string());
                        }
                        _ => {}
                    }
                }
            }
        }

        if !token_iter.peek().is_some() {
            return Err("incomplete haystack");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::parse_search_key;
    use serde_json::Value;

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

    #[test]
    fn array_only() {
        assert_eq!(search(r#"[8,9,1]"#, &vec!("[0]".to_string())), Ok("8".to_string()));
        assert_eq!(search(r#"[8,9,1]"#, &vec!("[1]".to_string())), Ok("9".to_string()));
        assert_eq!(search(r#"[8,9,1]"#, &vec!("[2]".to_string())), Ok("1".to_string()));

        assert_eq!(search(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#, &vec!("[0]".to_string())), Ok(r#"{"x":"y"}"#.to_string()));
        assert_eq!(search(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#, &vec!("[1]".to_string())), Ok(r#"{"a":{"b": "c"}}"#.to_string()));

        assert_eq!(search(r#"[[3, [6,7],5],9,1]"#, &parse_search_key("[0][1][1]".to_string())), Ok("7".to_string()));
        assert_eq!(search(r#"[[3, [6,7],5],9,1]"#, &vec!("[0]".to_string(), "[1]".to_string())), Ok("[6,7]".to_string()));
        assert_eq!(search(r#"[[3, [6,7],5],9,1]"#, &vec!("[2]".to_string())), Ok("1".to_string()));
        assert_eq!(search(r#"[[3, [6,7],5],9,1]"#, &vec!("[0]".to_string())), Ok("[3, [6,7],5]".to_string()));
    }

    #[test]
    fn object_only() {
        assert_eq!(search(r#"{"x":"y"}"#, &vec!("x".to_string())), Ok("y".to_string()));

        assert_eq!(search(r#"{"x":{"y":"z"}}"#, &parse_search_key("x.y".to_string())), Ok("z".to_string()));
        assert_eq!(search(r#"{"x":{"y":"z"}}"#, &vec!("x".to_string())), Ok(r#"{"y":"z"}"#.to_string()));

        assert_eq!(search(r#"{"x":["y"]}"#, &vec!("x".to_string())), Ok(r#"["y"]"#.to_string()));
        assert_eq!(search(r#"{"x":{"y":["z"]}}"#, &vec!("x".to_string())), Ok(r#"{"y":["z"]}"#.to_string()));
        assert_eq!(search(r#"{"x":{"y":["z"]}}"#, &parse_search_key("x.y".to_string())), Ok(r#"["z"]"#.to_string()));
        assert_eq!(search(r#"{"x":{"y":["z"]}}"#, &parse_search_key("x.y[0]".to_string())), Ok("z".to_string()));
    }

    #[test]
    fn mixed() {
        let mut result = search(r#"
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
            ]"#, &parse_search_key("[3].a.b".to_string())).unwrap();
        let mut json: Value = serde_json::from_str(&result).expect("Invalid JSON");
        assert_eq!(json.to_string(), "[2,7,4]".to_string());

        result = search(r#"
            {
                "a": {
                    "x": "y\"b\"",
                    "b": [
                        2,
                        3,
                        4
                    ]
                }
            }"#, &parse_search_key("a.b".to_string())).unwrap();
        json = serde_json::from_str(&result).expect("Invalid JSON");
        assert_eq!(json.to_string(), "[2,3,4]".to_string());
    }
}