use crate::model::j_struct_tracker::JStructTracker;
use crate::model::stream_tracker::StreamTracker;
use crate::utils::{find_str, is_ndjson, sanitize_output, token_pos};
use futures::executor::{ThreadPool, ThreadPoolBuilder};
use futures::task::SpawnExt;
use json_tools::{BufferType, Lexer, Token, TokenType};
use json_value_merge::Merge;
use log::{debug, info};
use regex::Regex;
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};
use std::ops::{Add, ControlFlow};
use std::path::absolute;
use std::sync::mpsc;
use std::thread;
use std::thread::available_parallelism;

pub(crate) fn sort_serde_json(value: &Value) -> Value {
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

pub(crate) fn deduplicate_arrays(value: Value) -> Value {
    match value {
        Value::Array(arr) => {
            // Convert array elements to strings for comparison
            let mut seen = HashSet::new();
            let deduplicated: Vec<Value> = arr
                .into_iter()
                .filter(|item| {
                    // Convert to string for comparison, fallback to empty string if conversion fails
                    let str_val = item.to_string();
                    seen.insert(str_val)
                })
                .map(|item| deduplicate_arrays(item))
                .collect();
            Value::Array(deduplicated)
        }
        Value::Object(map) => {
            // Recursively process object values
            let new_map: Map<String, Value> = map
                .into_iter()
                .map(|(k, v)| (k, deduplicate_arrays(v)))
                .collect();
            Value::Object(new_map)
        }
        // Return other value types unchanged
        _ => value,
    }
}

pub fn summarize(haystack: &str, unionize: bool) -> Result<String, &'static str> {
    if is_ndjson(haystack) {
        let lines: Vec<String> = haystack.lines()
            .map(String::from)
            .collect();

        let num_threads = available_parallelism().unwrap().get();
        let pool = ThreadPoolBuilder::new()
            .pool_size(num_threads)
            .create()
            .unwrap();

        let (tx, rx) = mpsc::channel();
        for line in lines {
            let tx = tx.clone();
            let future = async move {
                tx.send(parse(&line, true).unwrap()).unwrap();
            };
            pool.spawn(future).unwrap();
        }

        drop(tx);

        let mut schemas: Vec<String> = rx.iter().collect();
        schemas.dedup();
        let first_schema = schemas.first().unwrap().as_str();
        let mut first: Value = serde_json::from_str(first_schema).unwrap();
        for next in schemas.iter() {
            let next_schema = serde_json::from_str(next).unwrap();
            first.merge(&next_schema);
        }
        let mut json_schema: Value = serde_json::from_str("[]").unwrap();
        json_schema.merge(&first);
        json_schema = deduplicate_arrays(json_schema);
        json_schema = sort_serde_json(&json_schema);
        Ok(json_schema.to_string())
    } else {
        parse(haystack, unionize)
    }
}

pub fn parse(haystack: &str, unionize: bool) -> Result<String, &'static str> {
    let mut struct_t = JStructTracker::init();
    let mut schema_tape = String::new();

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

    info!("done");
    let mut json: Value = serde_json::from_str(schema_tape.as_str()).expect("JSON parsing error");
    json = deduplicate_arrays(json);
    json = sort_serde_json(&json);

    // Perform union at top level
    if unionize {
        Ok(unionize_schema(&json, true))
    } else {
        Ok(json.to_string())
    }
}

fn unionize_schema(json: &Value, array_wrap: bool) -> String {
    if json.is_array() {
        let mut first = json.as_array().unwrap().first().unwrap().clone();
        for next in json.as_array().unwrap().iter() {
            first.merge(next);
        }

        if array_wrap {
            return "[".to_string() + first.to_string().as_str() + "]";
        }
        return first.to_string();
    }
    json.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sort_serde_json(json: &str) -> Result<String, &'static str> {
        // First parse to Value
        let value: Value = serde_json::from_str(json).unwrap();

        // Convert to BTreeMap to sort
        let sorted = sort_serde_json(&value);

        // Convert back to Value
        Ok(sorted.to_string())
    }

    #[test]
    fn ordering() {
        assert_eq!(
            test_sort_serde_json(r#"{"b":"c","a":"f"}"#).unwrap(),
            r#"{"a":"f","b":"c"}"#.to_string()
        );
        assert_eq!(
            test_sort_serde_json(r#"{"c":{"h":"i"}, "a":"b", "e":[2,false,{"bob":{"f":"g"}}]}"#).unwrap(),
            r#"{"a":"b","c":{"h":"i"},"e":[2,false,{"bob":{"f":"g"}}]}"#.to_string()
        );
        assert_eq!(
            test_sort_serde_json(r#"[23,12,22,0]"#).unwrap(),
            r#"[0,12,22,23]"#.to_string()
        );
        assert_eq!(
            test_sort_serde_json(r#"["23","12","22","0"]"#).unwrap(),
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

    #[test]
    fn ndjson_test() {
        assert_eq!(summarize(r#"{"a":"b"}
        {"a":"c"}"#, true), Ok(r#"[{"a":"string"}]"#.to_string()));

        assert_eq!(summarize(r#"{"a":"b","f":12}
        {"a":"c","d":"c"}"#, true), Ok(r#"[{"a":"string","d":"string","f":"number"}]"#.to_string()));

        assert_eq!(summarize(r#"{"a":"b","f":[{"x":"y"},{"x":"v"}]}
        {"a":"c"}"#, true), Ok(r#"[{"a":"string","f":[{"x":"string"}]}]"#.to_string()));
    }
}
