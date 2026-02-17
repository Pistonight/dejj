use cu::pre::*;
use tyyaml::{Tree, TreeRepr};

use crate::NamespacedName;

/// Name with namespace and templates. i.e. the fully qualified name (`foo::bar::Biz<T1, T2>`)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NamespacedTemplatedName {
    /// The untemplated base name (with namespace)
    pub base: NamespacedName,
    /// The template types
    pub templates: Vec<TemplateArg<NamespacedTemplatedName>>,
}
impl NamespacedTemplatedName {
    pub fn new(base: NamespacedName) -> Self {
        Self::with_templates(base, vec![])
    }
    pub fn with_templates(base: NamespacedName, templates: Vec<TemplateArg<Self>>) -> Self {
        Self { base, templates }
    }
}
impl TreeRepr for NamespacedTemplatedName {
    fn serialize_spec(&self) -> cu::Result<String> {
        Ok(json::stringify(self)?)
    }
    fn deserialize_void() -> Self {
        Self::new(NamespacedName::unnamespaced("void"))
    }
    fn deserialize_spec(spec: &str) -> cu::Result<Self> {
        Ok(json::parse(spec)?)
    }
}

/// Template arguments (i.e. `template <...>`)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TemplateArg<T: TreeRepr> {
    /// Constant value. Could also be boolean (0=false, 1=true)
    #[display("{}", _0)]
    Const(i64),
    /// Type value. Could be unflattened depending on the stage
    #[display("{}", _0)]
    Type(Tree<T>),

    /// A constant value assigned by compiler (like a function address)
    #[display("[static]")]
    StaticConst,
}
