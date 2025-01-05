use autocxx::prelude::*;
use crate::utils::{array_ind, parse_search_key, sanitize_output};

include_cpp! {
    #include "simdjson/wrapper.h"
    safety!(unsafe)
    generate!("value_at_path")
}
pub fn search(haystack: Option<&str>, file: Option<&str>, search_key: &str) -> Result<String, &'static str> {
    let search_path = parse_search_key(search_key);
    if search_path.is_empty() {
        return Err("search key must not be empty");
    }

    let search_key_global = if array_ind(search_path[0].as_str()) < 0 {
        format!("$.{}", search_key)
    } else {
        format!("${}", search_key)
    };

    let result: String = ffi::value_at_path(haystack.unwrap_or(""), file.unwrap_or(""), search_key_global.as_str());
    if !result.is_empty() && result.eq("JIST_ERROR_FILE_TOO_LARGE") {
        return Err("JIST_ERROR_FILE_TOO_LARGE")
    }
    Ok(sanitize_output(&result))
}