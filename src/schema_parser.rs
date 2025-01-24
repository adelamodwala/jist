use crate::model::j_struct_tracker::JStructTracker;
use crate::model::stream_tracker::StreamTracker;
use crate::utils::{find_str, token_pos};
use json_tools::{BufferType, Lexer, Token, TokenType};
use log::{debug, info};
use md5;
use md5::Digest;
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};
use std::ops::{Add, ControlFlow};
use std::path::absolute;
use std::thread;

fn sort_raw_json(json: &str) -> Result<String, &'static str> {
    // First parse to Value
    let value: Value = serde_json::from_str(json).unwrap();

    // Convert to BTreeMap to sort
    let sorted = sort_serde_json(&value);

    // Convert back to Value
    Ok(sorted.to_string())
}

fn sort_serde_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            // Convert Map to BTreeMap to sort keys
            let mut sorted_map: BTreeMap<String, Value> = BTreeMap::new();
            for (k, v) in map {
                // Recursively sort nested objects
                sorted_map.insert(k.clone(), sort_serde_json(v));
            }
            // Convert back to Value
            Value::Object(Map::from_iter(sorted_map))
        }
        Value::Array(arr) => {
            // Recursively sort objects in arrays
            let mut sorted_arr: Vec<Value> = arr.iter().map(sort_serde_json).collect();
            // Sort the array by JSON value lexicographically
            sorted_arr.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
            Value::Array(sorted_arr)
        }
        // Other value types remain unchanged
        _ => value.clone(),
    }
}

fn add_sub_schema(
    sub_schemas: &mut HashMap<String, String>,
    sub_schema: String,
    token_type: &TokenType,
    // schema_tape: String,
    tape_stack: &mut Vec<(String, TokenType)>,
    signature_db: &mut HashMap<String, HashSet<String>>,
) {
    // sort internal structure
    let mut sub_schema_sorted = sort_raw_json(&sub_schema).unwrap();
    if token_type.eq(&TokenType::BracketClose) {
        sub_schema_sorted = collapse_array(&sub_schema_sorted);
    }

    // compress both raw and ordered sub schemas
    sub_schema_sorted = compress_schema(sub_schemas.to_owned(), sub_schema_sorted);
    let sub_schema_compressed = compress_schema(sub_schemas.to_owned(), sub_schema);

    let hash = murmur3::murmur3_32(&mut sub_schema_sorted.as_bytes(), 0)
        .unwrap()
        .to_string();

    // add sub_schema to registry
    sub_schemas.insert(sub_schema_sorted.clone(), hash.clone());

    // update signature database
    if !signature_db.contains_key(&hash) {
        signature_db.insert(hash.clone(), HashSet::new());
    }
    sub_schemas
        .iter()
        .for_each(|(_, value)| {
            if sub_schema_sorted.contains(value.as_str()) {
                signature_db
                    .get_mut(&hash)
                    .unwrap()
                    .insert(value.to_string());
            }
        });

    // add to stack
    tape_stack.push((hash.to_string(), token_type.clone()));

    while tape_stack.len() > 1 {
        if signature_db.get(&hash).unwrap().contains(&tape_stack[tape_stack.len() - 2].0) {
            tape_stack.remove(tape_stack.len() - 2);
        } else {
            break;
        }
    }
}

fn compress_schema(sub_schemas: HashMap<String, String>, schema_tape: String) -> String {
    let mut result = schema_tape.clone();
    for (sub_schema, murmur3_hash) in &sub_schemas {
        result = result.replace(sub_schema, murmur3_hash.as_str());
    }
    result
}

fn hydrate_schema(sub_schemas: &HashMap<String, String>, schema_tape: String) -> String {
    let mut result = schema_tape.clone();
    loop {
        let matches: Vec<String> = sub_schemas
            .iter()
            .filter(|(_, value)| result.contains(value.as_str()))
            .map(|(key, _)| key.clone())
            .collect();
        if matches.is_empty() {
            break;
        } else {
            for key in &matches {
                result = result.replace(sub_schemas.get(key).unwrap().as_str(), key);
            }
        }
    }

    result
}

fn collapse_array(array_schema: &String) -> String {
    let mut array: Vec<Value> =
        serde_json::from_str(sort_raw_json(array_schema).unwrap().as_str()).unwrap();
    array.dedup();
    let result = serde_json::to_string(&array).unwrap();
    debug!("array_schema: {:?}", result);
    result
}

pub fn parse<R: Read + Seek + BufRead>(
    mut reader: R,
    mut seeker: R,
) -> Result<String, &'static str> {
    let chunk_size = 1_000_000_000;
    let mut stream_t = StreamTracker::new(chunk_size);
    let mut struct_t = JStructTracker::init();
    let mut schema_tape = String::new();
    let mut sub_schemas: HashMap<String, String> = HashMap::new();
    let mut tape_stack: Vec<(String, TokenType)> = Vec::new();
    let mut signature_db: HashMap<String, HashSet<String>> = HashMap::new();

    loop {
        let bytes_read = reader
            .by_ref()
            .take(chunk_size as u64)
            .read_to_end(&mut stream_t.buffer)
            .unwrap();
        if bytes_read == 0 && stream_t.buffer.is_empty() {
            break;
        }

        // Find the last line break (add one if last buffer read)
        let mut chunk_str = String::from_utf8(stream_t.buffer.clone()).unwrap();
        if bytes_read < chunk_size {
            chunk_str.push('\n');
        }
        if let Some(last_chunk_tup) = chunk_str.rsplit_once('\n') {
            let last_chunk = last_chunk_tup.0;
            debug!("last_chunk: {}", last_chunk);
            // Process the chunk that ends with a newline
            stream_t.chunk.extend_from_slice(last_chunk.as_bytes());
            stream_t.chunk.push(b'\n');
            stream_t.last_chunk_len = stream_t.chunk.len();

            // chunk processing
            let mut token_iter = Lexer::new(stream_t.chunk.clone(), BufferType::Span).peekable();
            loop {
                let token_opt = token_iter.next();
                if token_opt.is_none() {
                    break;
                }

                // token processing
                let mut token = token_opt.unwrap();
                match token.kind {
                    TokenType::CurlyOpen => {
                        struct_t.depth_curr.0 += 1;
                        struct_t.depth_curr.2 += 1;
                        let (first, end) = token_pos(&token.buf)?;
                        struct_t.last_open_pin.push((
                            TokenType::CurlyOpen,
                            first,
                            schema_tape.len(),
                        ));
                        schema_tape = schema_tape + "{";
                    }
                    TokenType::CurlyClose => {
                        struct_t.depth_curr.0 -= 1;
                        struct_t.depth_curr.2 -= 1;
                        schema_tape = schema_tape + "}";

                        let last_curly_open = struct_t
                            .last_open_pin
                            .iter()
                            .filter(|sym| sym.0 == TokenType::CurlyOpen)
                            .last();
                        if last_curly_open.is_none() {
                            return Err("invalid json: missing opening curly brace");
                        }
                        let curly_open = last_curly_open.unwrap();
                        let mut value_schema = String::new();
                        value_schema.clone_from(&schema_tape[curly_open.2..].to_string());
                        add_sub_schema(
                            &mut sub_schemas,
                            value_schema,
                            &token.kind,
                            &mut tape_stack,
                            &mut signature_db
                        );

                        struct_t.last_open_pin.pop();
                    }
                    TokenType::BracketOpen => {
                        struct_t.depth_curr.0 += 1;
                        struct_t.depth_curr.1 += 1;
                        struct_t.arr_idx.push(0);
                        let (first, end) = token_pos(&token.buf)?;
                        struct_t.last_open_pin.push((
                            TokenType::BracketOpen,
                            first,
                            schema_tape.len(),
                        ));
                        schema_tape = schema_tape + "[";
                    }
                    TokenType::BracketClose => {
                        struct_t.depth_curr.0 -= 1;
                        struct_t.depth_curr.1 -= 1;
                        schema_tape = schema_tape + "]";

                        let last_bracket_open = struct_t
                            .last_open_pin
                            .iter()
                            .filter(|sym| sym.0 == TokenType::BracketOpen)
                            .last();
                        if last_bracket_open.is_none() {
                            return Err("invalid json: missing opening curly brace");
                        }
                        let curly_open = last_bracket_open.unwrap();
                        let mut value_schema = String::new();
                        value_schema.clone_from(&schema_tape[curly_open.2..].to_string());
                        add_sub_schema(
                            &mut sub_schemas,
                            value_schema,
                            &token.kind,
                            &mut tape_stack,
                            &mut signature_db
                        );

                        struct_t.arr_idx.pop();
                        struct_t.last_open_pin.pop();
                    }
                    TokenType::Comma => {
                        let (token_type, _, _) = struct_t.last_open_pin.last().unwrap();
                        if struct_t.depth_curr.1 > -1                    // must be inside an array
                            && token_type.eq(&TokenType::BracketOpen)
                        {
                            let arr_idx_len = struct_t.arr_idx.len();
                            struct_t.arr_idx[arr_idx_len - 1] += 1;
                        }
                        schema_tape = schema_tape + ",";
                    }
                    TokenType::Colon => schema_tape = schema_tape + ":",
                    TokenType::BooleanTrue | TokenType::BooleanFalse => {
                        schema_tape = schema_tape + "\"boolean\""
                    }
                    TokenType::Number => schema_tape = schema_tape + "\"number\"",
                    TokenType::Null => schema_tape = schema_tape + "\"null\"",
                    _ => {} // String type can be an object key which requires special handling
                }

                if !struct_t.last_open_pin.is_empty() {
                    let (token_type, _, _) = struct_t.last_open_pin.last().unwrap();
                    if token_type.eq(&TokenType::BracketOpen) {
                        if token.kind == TokenType::String {
                            schema_tape = schema_tape + "\"string\"";
                        }
                    } else if token_type.eq(&TokenType::CurlyOpen) {
                        if token.kind == TokenType::String && struct_t.last_token_key_delimiter {
                            let (first, end) = token_pos(&token.buf)?;
                            let key = find_str(
                                &mut seeker,
                                first + stream_t.last_stream_pos,
                                end + stream_t.last_stream_pos,
                            )
                            .unwrap();
                            schema_tape = schema_tape + &key;
                            struct_t.last_token_key_delimiter = false;
                        } else if token.kind == TokenType::String {
                            schema_tape = schema_tape + "\"string\"";
                        }

                        if token.kind == TokenType::CurlyOpen || token.kind == TokenType::Comma {
                            struct_t.last_token_key_delimiter = true;
                        }
                    }
                }

                if token_iter.peek().is_none() {
                    break;
                }
            }

            // Clear chunk for next iteration
            stream_t.chunk.clear();
            // Remove processed data from buffer
            stream_t.buffer.clear();
        } else {
            panic!("Invalid chunk");
        }

        stream_t.last_stream_pos += stream_t.last_chunk_len as u64;
    }

    debug!("{:?}", sub_schemas);

    info!("done");
    match tape_stack.last() {
        None => {Err("unable to find schema")}
        Some((sub_schema, _token_type)) => Ok(hydrate_schema(&sub_schemas, sub_schema.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use super::*;
    use std::io::Cursor;

    fn call(input: &str) -> Result<String, &'static str> {
        let mut reader = Cursor::new(input.as_bytes());
        let mut seeker = Cursor::new(input.as_bytes());
        parse(&mut reader, &mut seeker)
    }

    #[test]
    fn file_test() {
        let f = File::open("output.json").unwrap();
        let data = fs::read_to_string("output.json").unwrap();
        let mut reader = Cursor::new(data.as_bytes());
        let mut seeker = Cursor::new(data.as_bytes());
        let result = parse(&mut reader, &mut seeker);
        println!("{:?}", result);
    }

    #[test]
    fn ordering() {
        assert_eq!(
            sort_raw_json(r#"{"b":"c","a":"f"}"#).unwrap(),
            r#"{"a":"f","b":"c"}"#.to_string()
        );
        assert_eq!(
            sort_raw_json(r#"{"c":{"h":"i"}, "a":"b", "e":[2,false,{"bob":{"f":"g"}}]}"#).unwrap(),
            r#"{"a":"b","c":{"h":"i"},"e":[2,false,{"bob":{"f":"g"}}]}"#.to_string()
        );
        assert_eq!(
            sort_raw_json(r#"[23,12,22,0]"#).unwrap(),
            r#"[0,12,22,23]"#.to_string()
        );
        assert_eq!(
            sort_raw_json(r#"["23","12","22","0"]"#).unwrap(),
            r#"["0","12","22","23"]"#.to_string()
        );
    }

    #[test]
    fn it_works() {
        assert_eq!(call(r#"{"a":"b"}"#), Ok(r#"{"a":"string"}"#.to_string()));

        assert_eq!(
            call(r#"{"a":"b", "c":"d"}"#),
            Ok(r#"{"a":"string","c":"string"}"#.to_string())
        );

        assert_eq!(
            call(r#"{"a":"b", "c":"d", "e":[2,false,"bob"]}"#),
            Ok(r#"{"a":"string","c":"string","e":["boolean","number","string"]}"#.to_string())
        );

        assert_eq!(
            call(r#"{"c":{"h":"i"}, "a":"b", "e":[2,false,{"rob":"cob","bob":{"f":"g"}}]}"#),
            Ok(r#"{"a":"string","c":{"h":"string"},"e":["boolean","number",{"bob":{"f":"string"},"rob":"string"}]}"#.to_string())
        );

        // test repeating patterns
        assert_eq!(call(r#"[1,2,4]"#), Ok(r#"["number"]"#.to_string()));
        assert_eq!(
            call(r#"[1,2,4,"bob",43]"#),
            Ok(r#"["number","string"]"#.to_string())
        );
        assert_eq!(
            call(r#"[{"a":"b"},{"a":"d"}]"#),
            Ok(r#"[{"a":"string"}]"#.to_string())
        );
        assert_eq!(
            call(r#"[{"a":"b"},{"f":"g","h":{"a":"c"}},{"a":"d"}]"#),
            Ok(r#"[{"a":"string"},{"f":"string","h":{"a":"string"}}]"#.to_string())
        );
    }
}
