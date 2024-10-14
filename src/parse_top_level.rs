use crate::utils::array_ind;
use json_tools::{BufferType, Lexer, TokenType};

// #[derive(Debug)]
// enum NodeTokenType {
//     TokenType(TokenType),
//     ObjectKey,
// }
// fn token_type_to_json_type(token_type: &TokenType) -> JsonType {
//     match token_type {
//         TokenType::CurlyOpen | TokenType::CurlyClose => Object,
//         TokenType::BracketOpen| TokenType::BracketClose => Array,
//         TokenType::String => String,
//         TokenType::BooleanTrue | TokenType::BooleanFalse => Boolean,
//         TokenType::Number => Number,
//         TokenType::Null => Null,
//         _ => Syntax
//     }
// }
// #[derive(Debug)]
// enum JsonType {
//     Object,
//     Array,
//     String,
//     Number,
//     Boolean,
//     Null,
//     Syntax,
// }
// #[derive(Debug)]
// struct Node {
//     token_type: NodeTokenType,
//     json_type: JsonType,
//     // parent_type: Option<JsonType>,
//     child: Option<Box<Node>>,
//     sibling: Option<Box<Node>>,
//     start: u64,     // where the node starts
//     end: u64,       // where the node actually ends in JSON
//     token_end: u64, // where the token ends
// }
//
// fn token_pos(buf: &Buffer) -> Result<(u64, u64), &'static str> {
//     let (first, end) = match buf {
//         Buffer::Span(pos) => (pos.first, pos.end),
//         _ => { return Err("error"); }
//     };
//     Ok((first, end))
// }
//
// fn next_node(mut lexer: Lexer<core::str::Bytes>) -> Option<Box<Node>> {
//     let token = lexer.next().unwrap();
//     let json_type = token_type_to_json_type(&token.kind);
//     let (first, end) = token_pos(&token.buf).unwrap();
//
//     // check if primitive
//     match json_type {
//         JsonType::String | JsonType::Number | JsonType::Boolean | JsonType::Null => {
//
//         }
//         _ => {}
//     }
//     None
// }
//
// fn parse(data: &str) -> Result<std::string::String, &'static str> {
//     let mut lexer = Lexer::new(data.bytes(), BufferType::Span);
//     let mut token = lexer.next().expect("no tokens for root object");
//     let (first, end) = token_pos(&token.buf)?;
//     let root_type = match token.kind {
//         TokenType::CurlyOpen => Some(Object),
//         TokenType::BracketOpen => Some(Array),
//         _ => {
//             return Err("root type is neither object nor array");
//         }
//     };
//     let mut root = Box::new(Node {
//         token_type: NodeTokenType::TokenType(token.kind),
//         json_type: root_type.unwrap(),
//         child: None,
//         sibling: None,
//         start: first,
//         end: 0,
//         token_end: end,
//     });
//     println!("{:?}", root);
//     if let token = lexer.next().unwrap() {
//         root.child = next_node(lexer);
//     }
//
//     Ok("Done".to_string())
// }

pub fn search(haystack: &str, search_path: &Vec<std::string::String>) -> Result<std::string::String, &'static str> {
    let mut token_iter = Lexer::new(haystack.bytes(), BufferType::Span).peekable();

    // tuple of (depth, arr_depth, obj_depth)
    let mut depth_curr = (-1, -1, -1);

    // keep track of array indices if currently inside array
    let mut arr_idx = Vec::new();
    let mut last_open: Vec<TokenType> = Vec::new();

    // target depth, [3].a.b[1]
    let depth_tgt = (
        search_path.len() - 1,
        search_path.iter().filter(|x| x.starts_with("[")).count() - 1,
        search_path.iter().filter(|x| !x.starts_with("[")).count() - 1
    );
    let arr_tgt: Vec<i64> = search_path.iter()
        .filter(|x| x.starts_with("["))
        .map(|x| array_ind(x.clone()))
        .collect();
    println!("depth_tgt: {:?}, arr_tgt: {:?}", depth_tgt, arr_tgt);

    loop {
        let token = token_iter.next().unwrap();
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
                    && last_open[last_open.len() - 1] == TokenType::BracketOpen
                {
                    arr_idx[arr_idx_len - 1] += 1;
                }
            }
            _ => {}
        }
        println!("depth_curr: {:?}, arr_idx: {:?}, kind: {:?}, last_open: {:?}", depth_curr, arr_idx, &token.kind, last_open);

        if !token_iter.peek().is_some() {
            break;
        }
    }

    Ok("Done".to_string())
}

pub fn parse_test(data: &str) {
    let token_iter = Lexer::new(data.bytes(), BufferType::Span);
    for token in token_iter {
        println!("{:?}", token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::parse_search_key;
    #[test]
    fn test_parse() {
        // parse(r#"{"foo":[1,"top",4],"bar":{"a":"b"}}"#);
        search(r#"
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
            ]"#, &parse_search_key("[3].a.b[1]".to_string()));
    }
}