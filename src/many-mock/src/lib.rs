use std::collections::BTreeMap;

use many_protocol::RequestMessage;

pub(crate) type MockEntries = BTreeMap<String, toml::Value>;

/// Reads and parses the mockfile provided by the mockfile_arg parameter, or from a default path
pub fn parse_mockfile(mockfile_arg: &str) -> Result<MockEntries, String> {
    let path = std::path::Path::new(mockfile_arg);
    if !path.exists() {
        return Err(format!("File {:?} does not exist", path));
    }
    let contents = std::fs::read_to_string(path).map_err(|_| "Error reading file".to_string())?;
    toml::from_str(&contents)
        .map_err(|e| format!("Invalid mockfile, parse errors: {:?}", e.to_string()))
}

/// Prepares a RequestMessage to fill a mocked response
fn load_request(request: &RequestMessage) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        (
            r#""\$\{id\}""#,
            serde_json::to_string(&request.id).unwrap_or_default(),
        ),
        (
            r#""\$\{version\}""#,
            serde_json::to_string(&request.version).unwrap_or_default(),
        ),
        (
            r#""\$\{attributes\}""#,
            serde_json::ser::to_string(&request.attributes).unwrap_or_default(),
        ),
        (
            r#""\$\{nonce\}""#,
            serde_json::to_string(&request.nonce).unwrap_or_default(),
        ),
        (
            r#""\$\{data\}""#,
            serde_json::to_string(&request.data).unwrap_or_default(),
        ),
        (r#""\$\{method\}""#, request.method.clone()),
        (
            r#""\$\{timestamp\}""#,
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
