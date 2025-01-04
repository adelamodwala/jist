use autocxx::prelude::*;
use crate::utils::sanitize_output;

include_cpp! {
    #include "simdjson/wrapper.h"
    safety!(unsafe)
    generate!("value_at_path")
}
pub fn search(haystack: Option<&str>, file: Option<&str>, search_path: &str) -> Result<String, &'static str> {
    let result: String = ffi::value_at_path(haystack.unwrap_or(""), file.unwrap_or(""), search_path);

    Ok(sanitize_output(&result))
}

#[cfg(test)]
mod tests {

    // #[test]
    // fn test_simd_parser() {
    //     let start_time = Instant::now();
    //     poc();
    //     let elapsed = Instant::now().duration_since(start_time).as_millis();
    //     println!("simd_poc time: {:.2?}", elapsed);
    // }
}
