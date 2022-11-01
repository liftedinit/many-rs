use std::collections::BTreeMap;
use std::fmt;

use serde::{de::Visitor, Deserialize, Deserializer, Serialize};

pub mod server;

pub type MockEntries = BTreeMap<String, Vec<u8>>;

#[derive(Serialize, Deserialize, Debug)]
struct MockEntriesWrapper {
    #[serde(deserialize_with = "deserialize_entries", flatten)]
    entries: MockEntries,
}

fn deserialize_entries<'de, D>(d: D) -> Result<MockEntries, D::Error>
where
    D: Deserializer<'de>,
{
    struct MockEntriesVisitor;
    impl<'de> Visitor<'de> for MockEntriesVisitor {
        type Value = BTreeMap<String, Vec<u8>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("A map from string to hex code")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, String>()? {
                let value_data = cbor_diag::parse_diag(value).map_err(|e| {
                    serde::de::Error::custom(format!("Deserialization error: {e:?}"))
                })?;
                let value = value_data.to_bytes();
                result.insert(key, value);
            }
            Ok(result)
        }
    }

    d.deserialize_map(MockEntriesVisitor)
}

/// Reads and parses the mockfile provided by the mockfile_arg parameter, or from a default path
pub fn parse_mockfile(mockfile_arg: &str) -> Result<MockEntries, String> {
    let path = std::path::Path::new(mockfile_arg);
    if !path.exists() {
        return Err(format!("File {path:?} does not exist"));
    }
    let contents = std::fs::read_to_string(path).map_err(|_| "Error reading file".to_string())?;
    let parsed: MockEntriesWrapper = toml::from_str(&contents)
        .map_err(|e| format!("Invalid mockfile, parse errors: {:?}", e.to_string()))?;
    Ok(parsed.entries)
}
