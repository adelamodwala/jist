pub fn search(data: String, search_key: String) -> Result<String, &'static str> {
    if data.is_empty() || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }

    Ok(search_key)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn empty_data() {
        assert!(search("".to_string(), "a".to_string()).is_err());
    }

    #[test]
    fn empty_search_key() {
        assert!(search(r#"{"a": "b"}"#.to_string(), "".to_string()).is_err());
    }
}
