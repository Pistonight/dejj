use std::collections::BTreeMap;
use std::path::Path;

use cu::pre::*;

/// Parse compile_commands.json into a map from file name to the compile command
pub fn parse_compdb(path: &Path) -> cu::Result<BTreeMap<String, CompileCommand>> {
    let cc = cu::fs::read_string(path)?;
    let cc_vec = json::parse::<Vec<CompileCommand>>(&cc)?;
    let mut cc_map = BTreeMap::new();
    for c in cc_vec {
        cc_map.insert(c.file.clone(), c);
    }
    Ok(cc_map)
}

/// An entry in compile_commands.json
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompileCommand {
    /// The file name (usually absolute)
    pub file: String,
    /// The compile args. Does not include the compiler
    #[serde(deserialize_with = "deserialize_compile_command_args")]
    pub command: Vec<String>,
}
fn deserialize_compile_command_args<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<String>, D::Error> {
    return d.deserialize_str(Visitor);
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a POSIX compliant shell command")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            match shell_words::split(v) {
                Err(e) => Err(serde::de::Error::custom(format!(
                    "invalid shell command: {e}"
                ))),
                Ok(mut x) => {
                    if x.is_empty() {
                        return Err(serde::de::Error::custom(format!(
                            "command must be non-empty"
                        )));
                    }

                    // remove the compiler
                    x.remove(0);
                    Ok(x)
                }
            }
        }
    }
}
