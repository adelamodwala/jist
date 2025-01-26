use crate::model::j_struct_tracker::JStructTracker;
use crate::model::stream_tracker::StreamTracker;
use crate::utils::{find_str, sanitize_output, token_pos};
use json_tools::{BufferType, Lexer, Token, TokenType};
use log::{debug, info};
use md5;
use md5::Digest;
use regex::Regex;
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};
use std::ops::{Add, ControlFlow};
use std::path::absolute;

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
    skip_sort: bool,
    signature_db: &mut HashMap<String, HashSet<String>>,
) -> (String, TokenType) {
    // sort internal structure
    let mut sub_schema_sorted = if skip_sort {
        sub_schema
    } else {
        sort_raw_json(&sub_schema).unwrap()
    };
    if token_type.eq(&TokenType::BracketClose) {
        sub_schema_sorted = collapse_array(&sub_schema_sorted);
    }

    // compress both raw and ordered sub schemas
    sub_schema_sorted = compress_schema(sub_schemas.to_owned(), sub_schema_sorted);

    let hash = murmur3::murmur3_32(&mut sub_schema_sorted.as_bytes(), 0)
        .unwrap()
        .to_string();

    // add sub_schema to registry
    sub_schemas.insert(sub_schema_sorted.clone(), hash.clone());

    // update signature database
    if !signature_db.contains_key(&hash) {
        signature_db.insert(hash.clone(), HashSet::new());
    }

    // this likely has big performance penalties re StrSearcher
    sub_schemas.iter().for_each(|(_, value)| {
        if sub_schema_sorted.contains(value.as_str()) {
            signature_db
                .get_mut(&hash)
                .unwrap()
                .insert(value.to_string());
        }
    });

    (hash.to_string(), token_type.clone())
}

fn compress_schema(sub_schemas: HashMap<String, String>, mut schema_tape: String) -> String {
    for (sub_schema, murmur3_hash) in sub_schemas.iter() {
        schema_tape = schema_tape.replace(sub_schema, murmur3_hash.as_str());
    }
    schema_tape
}

fn hydrate_schema(sub_schemas: &HashMap<String, String>, mut schema_tape: String) -> String {
    loop {
        let matches: Vec<String> = sub_schemas
            .iter()
            .filter(|(_, value)| schema_tape.contains(value.as_str()))
            .map(|(key, _)| key.clone())
            .collect();
        if matches.is_empty() {
            break;
        } else {
            for key in &matches {
                schema_tape = schema_tape.replace(sub_schemas.get(key).unwrap().as_str(), key);
            }
        }
    }

    schema_tape
}

fn collapse_array(array_schema: &String) -> String {
    let mut array: Vec<Value> = serde_json::from_str(array_schema).unwrap();
    array.dedup();
    let result = serde_json::to_string(&array).unwrap();
    debug!("array_schema: {:?}", result);
    result
}

pub fn parse(haystack: &str, unionize: bool) -> Result<String, &'static str> {
    let mut struct_t = JStructTracker::init();
    let mut schema_tape = String::new();
    let mut sub_schemas: HashMap<String, String> = HashMap::new();
    let mut last_seen_schema: (String, TokenType) = (String::new(), TokenType::Invalid);
    let mut signature_db: HashMap<String, HashSet<String>> = HashMap::new();

    let mut token_iter = Lexer::new(haystack.bytes(), BufferType::Span).peekable();
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
                struct_t
                    .last_open_pin
                    .push((TokenType::CurlyOpen, first, schema_tape.len()));
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
                last_seen_schema = add_sub_schema(
                    &mut sub_schemas,
                    value_schema,
                    &token.kind,
                    unionize, // skip sorting if going to take the largest schema
                    &mut signature_db,
                );

                struct_t.last_open_pin.pop();
            }
            TokenType::BracketOpen => {
                struct_t.depth_curr.0 += 1;
                struct_t.depth_curr.1 += 1;
                struct_t.arr_idx.push(0);
                let (first, end) = token_pos(&token.buf)?;
                struct_t
                    .last_open_pin
                    .push((TokenType::BracketOpen, first, schema_tape.len()));
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
                last_seen_schema = add_sub_schema(
                    &mut sub_schemas,
                    value_schema,
                    &token.kind,
                    unionize, // skip sorting if going to take the largest schema
                    &mut signature_db,
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
            TokenType::Null => schema_tape = schema_tape + "\"string\"",
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
                    let key = &haystack[first as usize..end as usize];
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

    debug!("{:?}", sub_schemas);

    info!("done");
    let full_schema = hydrate_schema(&sub_schemas, last_seen_schema.0);
    if last_seen_schema.1.eq(&TokenType::BracketClose) && unionize {
        let json: Value = serde_json::from_str(full_schema.as_str()).expect("JSON parsing error");
        let mut longest = 0;
        let mut longest_idx = 0;
        for (idx, el) in json.as_array().unwrap().iter().enumerate() {
            if el.to_string().len() > longest {
                longest_idx = idx;
                longest = el.to_string().len();
            }
        }
        let mut json_schema: Value = serde_json::from_str("[]").unwrap();
        json_schema
            .as_array_mut()
            .unwrap()
            .push(json.as_array().unwrap().get(longest_idx).unwrap().clone());

        Ok(json_schema.to_string())
    } else {
        Ok(full_schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // #[test]
    fn file_test() {
        let f = File::open("output.json").unwrap();
        let data = fs::read_to_string("output.json").unwrap();
        let result = parse(&data, true);
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
        assert_eq!(
            parse(r#"{"a":"b"}"#, false),
            Ok(r#"{"a":"string"}"#.to_string())
        );

        assert_eq!(
            parse(r#"{"a":"b", "c":"d"}"#, false),
            Ok(r#"{"a":"string","c":"string"}"#.to_string())
        );

        assert_eq!(
            parse(r#"{"a":"b", "c":"d", "e":[2,false,"bob"]}"#, false),
            Ok(r#"{"a":"string","c":"string","e":["boolean","number","string"]}"#.to_string())
        );

        assert_eq!(
            parse(r#"{"c":{"h":"i"}, "a":"b", "e":[2,false,{"rob":"cob","bob":{"f":"g"}}]}"#, false),
            Ok(r#"{"a":"string","c":{"h":"string"},"e":["boolean","number",{"bob":{"f":"string"},"rob":"string"}]}"#.to_string())
        );

        // test repeating patterns
        assert_eq!(parse(r#"[1,2,4]"#, true), Ok(r#"["number"]"#.to_string()));
        assert_eq!(
            parse(r#"[1,2,4,"bob",43]"#, false),
            Ok(r#"["number","string"]"#.to_string())
        );
        assert_eq!(
            parse(r#"[{"a":"b"},{"a":"d"}]"#, true),
            Ok(r#"[{"a":"string"}]"#.to_string())
        );
        assert_eq!(
            parse(r#"[{"a":"b"},{"f":"g","h":{"a":"c"}},{"a":"d"}]"#, false),
            Ok(r#"[{"a":"string"},{"f":"string","h":{"a":"string"}}]"#.to_string())
        );
    }
}
