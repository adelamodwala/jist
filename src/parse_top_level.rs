use std::cmp::{Ordering};
use crate::utils::array_ind;
use json_tools::{Buffer, BufferType, Lexer, TokenType};

fn token_pos(buf: &Buffer) -> Result<(u64, u64), &'static str> {
    let (first, end) = match buf {
        Buffer::Span(pos) => (pos.first, pos.end),
        _ => { return Err("error"); }
    };
    Ok((first, end))
}

fn is_primitive_token(token_type: TokenType) -> bool {
    match token_type {
        TokenType::String | TokenType::Number | TokenType::BooleanFalse | TokenType::BooleanTrue | TokenType::Null => true,
        _ => false
    }
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
    let mut token_iter = Lexer::new(haystack.bytes(), BufferType::Span).peekable();

    // tuple of (depth, arr_depth, obj_depth)
    let mut depth_curr: (i32, i32, i32) = (-1, -1, -1);

    // keep track of array indices if currently inside array
    let mut arr_idx = Vec::new();
    let mut last_open: Vec<TokenType> = Vec::new();
    let mut val_idx: (i32, i32) = (-1, -1);
    let mut checkpoint_start = -1;

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
    let mut search_keys = search_path.iter()
        .filter(|x| !x.starts_with("["))
        .map(|x| x.clone())
        .rev()
        .collect::<Vec<String>>();
    println!("checkpoints: {:?}, arr_tgt: {:?}", checkpoints, arr_tgt);

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
        println!("depth_curr: {:?}, arr_idx: {:?}, kind: {:?}, last_open: {:?}, checkpoints: {:?}", depth_curr, arr_idx, &token.kind, last_open, checkpoints);

        if depth_curr.cmp(&checkpoints.last().unwrap()) == Ordering::Equal {
            // check if inside an array and iterating to find the expected place
            if *last_open.last().unwrap() == TokenType::BracketOpen && arr_idx.last().unwrap().cmp(&arr_tgt.last().unwrap()) == Ordering::Equal {
                let (first, end) = token_pos(&token.buf)?;
                if checkpoint_start > -1 {
                    println!("***checkpoint ended***");
                    checkpoints.pop();

                    // terminal point
                    if checkpoints.len() == 0 {
                        // add 1 to starting index to exclude commas or brackets
                        return Ok(haystack[checkpoint_start as usize + 1..(end as usize)].to_string());
                    } else {
                        checkpoint_start = -1;
                    }
                } else {
                    println!("***checkpoint started***");

                    checkpoint_start = first as i32;
                }
            }
        }

        // match depth_curr.cmp(&depth_tgt) {
        //     Ordering::Equal => {
        //         if !arr_idx.is_empty() {
        //             match arr_idx.cmp(&arr_tgt) {
        //                 Ordering::Equal => {
        //                     let (first, end) = token_pos(&token.buf)?;
        //                     if val_idx.0 < 0 {
        //                         val_idx.0 = first as i32;
        //                     }
        //                     val_idx.1 = end as i32;
        //                 }
        //                 _ => {}
        //             }
        //         } else {
        //             // need to find the relevant key
        //             let search_key = search_keys.last().unwrap();
        //             while depth_curr.cmp(&depth_tgt) == Ordering::Equal {
        //                 token = token_iter.next().unwrap();
        //                 if token.kind == TokenType::Comma {
        //                     token = token_iter.next().unwrap();
        //                     let (first, end) = token_pos(&token.buf)?;
        //                     let (start, finish) = (first as usize, end as usize);
        //                     if start >= 0 && finish < haystack.len() {
        //                         let key = &haystack[start..finish];
        //                         if (key.cmp(search_key) == Ordering::Equal) && search_keys.len() == 1 {
        //                             token = token_iter.next().unwrap();
        //                             let (first, end) = token_pos(&token.buf)?;
        //                             val_idx.0 = first as i32;
        //                             if is_primitive_token(token.kind) {
        //                                 val_idx.1 = end as i32;
        //                                 return Ok(haystack[val_idx.0 as usize..(val_idx.1 as usize)].to_string());
        //                             }
        //                         }
        //                     }
        //                 }
        //             }
        //
        //         }
        //     }
        //     _ => {
        //         if val_idx.1 >= 0 {
        //             return Ok(haystack[val_idx.0 as usize..(val_idx.1 as usize)].to_string());
        //         }
        //     }
        // }

        if !token_iter.peek().is_some() && val_idx.1 < 0 {
            return Err("incomplete haystack");
        }
    }
}

pub fn parse_test(data: &str) {
    let token_iter = Lexer::new(data.bytes(), BufferType::Span);
    for token in token_iter {
        println!("{:?}", token);
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::parse_search_key;
    use super::*;

    #[test]
    fn checkpoint_depth_test() {
        assert_eq!(checkpoint_depth(&vec!["a".to_string(), "b".to_string(), "[1]".to_string()], 0), (0, -1, 0));
        assert_eq!(checkpoint_depth(&vec!["a".to_string(), "b".to_string(), "[1]".to_string()], 1), (1, -1, 1));
        assert_eq!(checkpoint_depth(&vec!["a".to_string(), "b".to_string(), "[1]".to_string()], 2), (2, 0, 1));

        assert_eq!(checkpoint_depth(&vec!["[2]".to_string()], 0), (0, 0, -1));

        assert_eq!(checkpoint_depth(&vec!["a".to_string()], 0), (0, -1, 0));

        assert_eq!(checkpoint_depth(&vec!["[1]".to_string(), "[1]".to_string(), "[1]".to_string(), "b".to_string()], 0), (0, 0, -1));
        assert_eq!(checkpoint_depth(&vec!["[1]".to_string(), "[1]".to_string(), "[1]".to_string(), "b".to_string()], 1), (1, 1, -1));
        assert_eq!(checkpoint_depth(&vec!["[1]".to_string(), "[1]".to_string(), "[1]".to_string(), "b".to_string()], 2), (2, 2, -1));
        assert_eq!(checkpoint_depth(&vec!["[1]".to_string(), "[1]".to_string(), "[1]".to_string(), "b".to_string()], 3), (3, 2, 0));
    }

    #[test]
    fn simple_array() {
        assert_eq!(search(r#"[8,9,1]"#, &vec!("[0]".to_string())), Ok("8".to_string()));
        assert_eq!(search(r#"[8,9,1]"#, &vec!("[1]".to_string())), Ok("9".to_string()));
        assert_eq!(search(r#"[8,9,1]"#, &vec!("[2]".to_string())), Ok("1".to_string()));

        assert_eq!(search(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#, &vec!("[0]".to_string())), Ok(r#"{"x":"y"}"#.to_string()));
        assert_eq!(search(r#"[{"x":"y"},{"a":{"b": "c"}},1]"#, &vec!("[1]".to_string())), Ok(r#"{"a":{"b": "c"}}"#.to_string()));
        assert_eq!(search(r#"[8,9,1]"#, &vec!("[2]".to_string())), Ok("1".to_string()));
        // assert_eq!(search(r#"
        //     [
        //         [-140.5405, [2, 3], "n", {"o": "p"}],
        //         "c",
        //         {
        //             "a": {
        //                 "x": "y\"b\"",
        //                 "b": [
        //                     2,
        //                     3,
        //                     4
        //                 ]
        //             }
        //         },
        //         {
        //             "a": {
        //                 "x": "y\"b\"",
        //                 "b": [
        //                     2,
        //                     7,
        //                     4
        //                 ]
        //             }
        //         },
        //         [
        //             "d",
        //             "e"
        //         ]
        //     ]"#, &parse_search_key("[3].a.b".to_string())), Ok("7".to_string()));
        // assert_eq!(search(r#"
        //     {
        //         "a": {
        //             "x": "y\"b\"",
        //             "b": [
        //                 2,
        //                 3,
        //                 4
        //             ]
        //         }
        //     }"#, &parse_search_key("a.b".to_string())), Ok("7".to_string()));
    }

    fn simple_object() {

    }
}