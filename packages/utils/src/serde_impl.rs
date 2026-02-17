use cu::pre::*;

/// Deserializable regex
#[derive(Debug, Clone, Deref, DerefMut)]
#[repr(transparent)]
pub struct SerdeRegex(regex::Regex);

impl<'de> Deserialize<'de> for SerdeRegex {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        return d.deserialize_str(Visitor);
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = SerdeRegex;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a regular expression")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                match regex::Regex::new(v) {
                    Err(e) => Err(serde::de::Error::custom(format!(
                        "invalid regular expression '{v}': {e}"
                    ))),
                    Ok(x) => Ok(SerdeRegex(x)),
                }
            }
        }
    }
}
