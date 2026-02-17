use std::collections::BTreeMap;

use cu::pre::*;
use tyyaml::Prim;

use crate::{ArcStr, Goff, GoffMap, GoffSet};

/// Data for all namespaces
pub struct NamespaceMaps {
    /// Goff to the qualifier that goff is in
    pub qualifiers: GoffMap<Namespace>,
    /// Goff to the namespace that goff is in (does not include types, etc)
    pub namespaces: GoffMap<Namespace>,
    /// Source string to namespace
    pub by_src: BTreeMap<String, Namespace>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, DebugCustom, Serialize, Deserialize)]
#[debug("{}", self)]
pub struct NamespacedName(pub Namespace, pub ArcStr);
impl NamespacedName {
    pub fn prim(prim: Prim) -> Self {
        Self::unnamespaced(prim.to_str())
    }
    pub fn unnamespaced(name: &str) -> Self {
        Self(Default::default(), name.into())
    }

    pub fn namespaced(namespace: &Namespace, name: &str) -> Self {
        Self(namespace.clone(), name.into())
    }

    pub fn basename(&self) -> &str {
        &self.1
    }

    pub fn namespace(&self) -> &Namespace {
        &self.0
    }

    /// Convert the namespaced name to string that can be used as a type
    /// in CPP. If the namespace involves a subprogram, Err is returned
    pub fn to_cpp_typedef_source(&self) -> cu::Result<String> {
        let mut s = self.namespace().to_cpp_typedef_source()?;
        if !s.is_empty() {
            s.push_str("::");
        }
        s.push_str(&self.1);
        Ok(s)
    }
}

#[derive(
    Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, DebugCustom, Serialize, Deserialize,
)]
#[debug("{}", self)]
pub struct Namespace(pub Vec<NameSeg>);
impl Namespace {
    pub fn parse_untemplated(s: &str) -> cu::Result<Self> {
        cu::ensure!(
            !s.contains(['<', '>', '*', '&']),
            "Namespace::parse_untemplated: cannot parse templated namespace: {s}"
        )?;
        Ok(Self(
            s.split("::")
                .map(|x| NameSeg::Name(x.trim().into()))
                .collect(),
        ))
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn contains_anonymous(&self) -> bool {
        self.0.iter().any(|x| x == &NameSeg::Anonymous)
    }
    pub fn source_segs_equal(&self, other: &Self) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }
        for (a, b) in std::iter::zip(&self.0, &other.0) {
            if !a.source_segs_equal(b) {
                return false;
            }
        }
        true
    }
    pub fn to_cpp_typedef_source(&self) -> cu::Result<String> {
        let mut s = String::new();
        for n in &self.0 {
            if let Some(x) = n.to_cpp_source()? {
                if !s.is_empty() {
                    s.push_str("::");
                }
                s.push_str(x);
            }
        }
        Ok(s)
    }
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display, DebugCustom, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum NameSeg {
    #[display("{}", _0)]
    #[debug("{}", _0)]
    Name(ArcStr),

    #[display("[ty={}]", _0)]
    #[debug("[ty={}]", _0)]
    Type(Goff, ArcStr),

    #[display("[subprogram={}]", _0)]
    #[debug("[subprogram={}]", _0)]
    Subprogram(Goff, ArcStr, bool /* is_linkage_name */),

    #[display("[anonymous]")]
    #[debug("[anonymous]")]
    Anonymous,
}

impl NameSeg {
    pub fn to_cpp_source(&self) -> cu::Result<Option<&str>> {
        match self {
            NameSeg::Name(s) => Ok(Some(s.as_ref())),
            NameSeg::Type(_, s) => Ok(Some(s.as_ref())),
            NameSeg::Subprogram(_, _, _) => {
                cu::bail!("to_cpp_source does not support subprogram as namespace");
            }
            NameSeg::Anonymous => Ok(None),
        }
    }
    pub fn source_segs_equal(&self, other: &Self) -> bool {
        match (self, other) {
            (NameSeg::Name(a), NameSeg::Name(b)) => a == b,
            (NameSeg::Type(_, a), NameSeg::Type(_, b)) => a == b,
            (NameSeg::Subprogram(a, _, _), NameSeg::Subprogram(b, _, _)) => a == b,
            (NameSeg::Anonymous, NameSeg::Anonymous) => true,
            _ => false,
        }
    }
    /// Mark referenced types for GC
    pub fn mark(&self, marked: &mut GoffSet) {
        if let NameSeg::Type(goff, _) = self {
            marked.insert(*goff);
        }
    }
}

#[rustfmt::skip]
mod __detail {
    use super::*;
    impl std::fmt::Display for NamespacedName { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() { self.1.fmt(f) } else { write!(f, "{}::{}", self.0, self.1) }
    } }
    impl std::fmt::Display for Namespace { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.0.iter();
        let Some(first) = iter.next() else { return Ok(()); };
        write!(f, "{first}")?; for n in iter { write!(f, "::{n}")?; }
        Ok(())
    } }
}
