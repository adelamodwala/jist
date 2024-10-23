use crate::utils::{array_ind, checkpoint_depth, parse_search_key, sanitize_output, token_pos};
use json_tools::{BufferType, Lexer, TokenType};
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read, Seek, SeekFrom};

fn find_str<R: Read + Seek>(mut seeker: R, start: u64, end: u64) -> Option<String> {
    let mut buff = vec![0u8; end as usize - start as usize];
    seeker.seek(SeekFrom::Start(start)).expect("error");
    seeker.read_exact(&mut buff).expect("error");
    String::from_utf8(buff.to_vec()).ok()
}

pub(crate) fn search<R: Read + Seek + BufRead>(mut reader: R, mut seeker: R, search_path: &Vec<String>) -> Result<String, &'static str> {
    let mut last_stream_pos = 0;
    let mut line = String::new();

    // tuple of (depth, arr_depth, obj_depth)
    let mut depth_curr: (i32, i32, i32) = (-1, -1, -1);

    // keep track of array indices if currently inside array
    let mut arr_idx = Vec::new();
    let mut last_open: Vec<TokenType> = Vec::new();
    let mut checkpoint_start = Vec::new();
    let mut last_token_key_delimiter = false;

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
        line.clear();
        reader.read_line(&mut line).unwrap();
        if line.len() == 0 {
            return Err("result not found");
        }
        let mut token_iter = Lexer::new(line.bytes(), BufferType::Span).peekable();

        loop {
            let token_opt = token_iter.next();
            if token_opt.is_none() {
                break;
            }

            let mut token = token_opt.unwrap();

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
            if depth_curr.cmp(&checkpoints.last().unwrap()) == Ordering::Equal {
                // check if inside an array and iterating to find the expected place
                if *last_open.last().unwrap() == TokenType::BracketOpen && arr_idx.last().unwrap().cmp(&arr_tgt.last().unwrap()) == Ordering::Equal {
                    let (first, end) = token_pos(&token.buf)?;

                    // terminal point
                    if checkpoints.len() == 1 && checkpoint_start.len() == arr_tgt_size {
                        println!("[checkpoint ended]");

                        // add 1 to starting index to exclude commas or brackets
                        let result = find_str(&mut seeker, (checkpoint_start.last().unwrap()) + 1, end + last_stream_pos).unwrap();
                        return Ok(sanitize_output(result.as_str()));
                    } else {
                        println!("[checkpoint started]");

                        checkpoint_start.push(first + last_stream_pos);
                        if arr_tgt.len() > 1 {
                            arr_tgt.pop();
                        }
                    }

                    if checkpoints.len() > 1 {
                        checkpoints.pop();
                    }
                } else if *last_open.last().unwrap() == TokenType::CurlyOpen {
                    if token.kind == TokenType::String && last_token_key_delimiter {
                        let (first, end) = token_pos(&token.buf)?;
                        let key = find_str(&mut seeker, first + last_stream_pos, end + last_stream_pos).unwrap();
                        if key.trim_start_matches("\"").trim_end_matches("\"").cmp(&search_keys.last().unwrap()) == Ordering::Equal {
                            println!(">>>found next key: {:?}", key);
                            search_keys.pop();

                            if checkpoints.len() > 1 {
                                checkpoints.pop();
                            }
                        }
                        last_token_key_delimiter = false;
                    } else if (token.kind == TokenType::CurlyOpen || token.kind == TokenType::Comma) && search_keys.len() > 0 {
                        if token.kind == TokenType::CurlyOpen {
                            println!("\\checkpoint started\\");
                        }
                        last_token_key_delimiter = true;
                    } else if search_keys.len() == 0 && checkpoints.len() == 1 {
                        let (first, end) = token_pos(&token.buf)?;
                        match token.kind {
                            TokenType::String | TokenType::Number | TokenType::BooleanFalse | TokenType::BooleanTrue | TokenType::Null => {
                                let result = find_str(&mut seeker, first + last_stream_pos, end + last_stream_pos).unwrap();
                                return Ok(sanitize_output(result.as_str()));
                            }
                            TokenType::Colon => {
                                checkpoint_start.push(first + last_stream_pos);
                            }
                            TokenType::CurlyClose | TokenType::BracketClose => {
                                let result = find_str(&mut seeker, checkpoint_start.last().unwrap() + 1, end + last_stream_pos).unwrap();
                                return Ok(sanitize_output(result.as_str()));
                            }
                            _ => {}
                        }
                        last_token_key_delimiter = false;
                    }
                }
            }

            if !token_iter.peek().is_some() {
                break;
            }
        }

        last_stream_pos = reader.stream_position().unwrap();
        println!("page finished - stream_position: {:?}", last_stream_pos);
    }
}