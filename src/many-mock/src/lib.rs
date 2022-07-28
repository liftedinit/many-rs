use std::collections::HashMap;

use many_protocol::RequestMessage;

pub type MockEntries = HashMap<String, toml::Value>;

/// Parses a string
pub fn parse_str(string: &str) -> Result<MockEntries, Box<dyn std::error::Error>> {
    let result: MockEntries = toml::from_str(string)?;
    Ok(result)
}

/// Reads and parses the mockfile provided by the mockfile_arg parameter, or from a default path
pub fn parse_mockfile(
    mockfile_arg: Option<&str>,
) -> Result<MockEntries, Box<dyn std::error::Error>> {
    let contents = if let Some(mockfile_arg) = mockfile_arg {
        // A file was passed from the command line
        std::fs::read_to_string(mockfile_arg)?
    } else {
        // If no file was passed from the command line, read the default path if it exists
        let fallback_path = std::path::Path::new("mockfile.toml");
        if fallback_path.exists() {
            std::fs::read_to_string(fallback_path)?
        } else {
            // If there's no file passed either from the command line or as a fallback path, just use an empty content
            "".to_string()
        }
    };
    parse_str(&contents)
}

/// Prepares a RequestMessage to fill a mocked response
fn load_request(request: &RequestMessage) -> HashMap<&'static str, String> {
    HashMap::from([
        (
            r#""?\$\{id\}"?"#,
            serde_json::to_string(&request.id).unwrap_or_default(),
        ),
        (
            r#""?\$\{version\}"?"#,
            serde_json::to_string(&request.version).unwrap_or_default(),
        ),
        (
            r#""?\$\{attributes\}"?"#,
            serde_json::ser::to_string(&request.attributes).unwrap_or_default(),
        ),
        (
            r#""?\$\{nonce\}"?"#,
            serde_json::to_string(&request.nonce).unwrap_or_default(),
        ),
        (
            r#""?\$\{data\}"?"#,
            serde_json::to_string(&request.data).unwrap_or_default(),
        ),
        (r#""?\$\{method\}"?"#, request.method.clone()),
        (
            r#""?\$\{timestamp\}"?"#,
            serde_json::to_string(&request.timestamp).unwrap_or_default(),
        ),
    ])
}

/// Replaces placeholders with RequestMessage information
pub fn fill_placeholders(request: &RequestMessage, response: String) -> String {
    let map = load_request(request);
    map.iter().fold(response, |acc, (key, value)| {
        let re = regex::Regex::new(key).unwrap();
        re.replace_all(&acc, value).to_string()
    })
}

#[cfg(test)]
mod tests {
    use crate::{parse_mockfile, MockEntries};

    const SIMPLE_TOML: &str = r#""/home" = "response""#;
    const COMPLEX_TOML: &str = r#"
    home = { complex = "toml", with = [ "many", "entries" ] }

    [and]
    more = "to see"
    "#;

    #[test]
    fn test_parser_simple() {
        let example: MockEntries = toml::from_str(SIMPLE_TOML).unwrap();
        assert_eq!(example["/home"], "response".into());
    }

    #[test]
    fn test_parser_complex() {
        let example: MockEntries = toml::from_str(COMPLEX_TOML).unwrap();
        let home = example["home"].as_table().unwrap();
        let and = example["and"].as_table().unwrap();
        assert_eq!(home["complex"], "toml".into());
        assert_eq!(home["with"], vec!["many", "entries"].into());
        assert_eq!(and["more"], "to see".into());
    }

    #[test]
    fn test_empty() {
        let example: MockEntries = toml::from_str("").unwrap();
        assert_eq!(example.len(), 0);
    }

    #[test]
    fn test_no_file() {
        let example = parse_mockfile(None).unwrap();
        assert_eq!(example.len(), 0);
    }
}
