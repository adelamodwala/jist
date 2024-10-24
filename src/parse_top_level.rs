use crate::utils::{array_ind, checkpoint_depth, sanitize_output, token_pos};
use json_tools::{BufferType, Lexer, TokenType};
use std::cmp::Ordering;

pub fn search(haystack: &str, search_path: &Vec<String>) -> Result<String, &'static str> {
    // println!("--- Searching for {} on path: {:?}", haystack, search_path);

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
    // println!("checkpoints: {:?}, arr_tgt: {:?}, search_keys: {:?}", checkpoints, arr_tgt, search_keys);

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
        // println!("depth_curr: {:?}, arr_idx: {:?}, search_keys: {:?}, kind: {:?}, last_open: {:?}, checkpoints: {:?}, checkpoint_start: {:?}", depth_curr, arr_idx, search_keys, &token.kind, last_open, checkpoints, checkpoint_start);

        if depth_curr.cmp(&checkpoints.last().unwrap()) == Ordering::Equal {
            // check if inside an array and iterating to find the expected place
            if *last_open.last().unwrap() == TokenType::BracketOpen && arr_idx.last().unwrap().cmp(&arr_tgt.last().unwrap()) == Ordering::Equal {
                let (first, end) = token_pos(&token.buf)?;

                // terminal point
                if checkpoints.len() == 1 && checkpoint_start.len() == arr_tgt_size {
                    // println!("[checkpoint ended]");

                    // add 1 to starting index to exclude commas or brackets
                    // return Ok(haystack[checkpoint_start.last().unwrap() + 1..(end as usize)].trim().trim_start_matches("\"").trim_end_matches("\"").to_string());
                    return Ok(sanitize_output(&haystack[checkpoint_start.last().unwrap() + 1..(end as usize)]));
                } else {
                    // println!("[checkpoint started]");

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
                    if token.kind == TokenType::CurlyOpen {
                        // println!("\\checkpoint started\\");
                    }

                    // next token should be a key
                    token = token_iter.next().expect("incomplete json object");
                    let (first, end) = token_pos(&token.buf)?;
                    let key = haystack[first as usize..end as usize].trim_start_matches("\"").trim_end_matches("\"");
                    if key.cmp(&search_keys.last().unwrap()) == Ordering::Equal {
                        // println!(">>>found next key: {:?}", key);
                        search_keys.pop();

                        if checkpoints.len() > 1 {
                            checkpoints.pop();
                        }
                    }
                } else if search_keys.len() == 0 && checkpoints.len() == 1 {
                    let (first, end) = token_pos(&token.buf)?;
                    match token.kind {
                        TokenType::String | TokenType::Number | TokenType::BooleanFalse | TokenType::BooleanTrue | TokenType::Null => {
                            return Ok(sanitize_output(&haystack[first as usize..(end as usize)]));
                        }
                        TokenType::Colon => {
                            checkpoint_start.push(first as usize);
                        }
                        TokenType::CurlyClose | TokenType::BracketClose => {
                            return Ok(sanitize_output(&haystack[checkpoint_start.last().unwrap() + 1..(end as usize)]));
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


}