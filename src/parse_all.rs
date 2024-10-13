use serde_json::Value;
use crate::utils;


pub fn search(haystack: &str, search_path: &Vec<String>) -> Result<String, &'static str> {
    let json: Value = serde_json::from_str(haystack).expect("Invalid JSON");

    // collect
    let mut path_val = &json;
    for key in search_path.iter() {
        let idx = utils::array_ind(key.to_string());
        if idx > -1 {
            path_val = &path_val[idx.unsigned_abs() as usize];
        } else {
            path_val = &path_val[key];
        }
    }

    // If value is a string, need to treat it as_str to avoid adding surrounding '"'
    if path_val.is_string() {
        return Ok(path_val.as_str().unwrap().to_string());
    }
    Ok(path_val.to_string())
}
