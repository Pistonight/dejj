use std::collections::{BTreeMap, BTreeSet};

use cu::pre::*;
use rkyv::Archived;
use tyyaml::{Prim, TreeRepr};

mod imp {
    use super::*;

    /// Global offset into DWARF
    ///
    /// A Goff is used as the unique identifier for a type in one extraction run.
    /// However it is not stable across multiple DWARF outputs
    #[rustfmt::skip]
    #[derive(
        DebugCustom, Display,
        Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, From, Into,
        rkyv::Archive, rkyv::Serialize, rkyv::Deserialize
    )]
    #[rkyv(derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord))]
    #[display("0x{:08x}", self.0)]
    #[debug("0x{:08x}", self.0)]
    pub struct Goff(pub usize);
}

pub use imp::Goff;

impl PartialEq<Goff> for Archived<Goff> {
    fn eq(&self, other: &Goff) -> bool {
        self.0.to_native() == other.0 as u32
    }
}
impl PartialEq<Archived<Goff>> for Goff {
    fn eq(&self, other: &Archived<Goff>) -> bool {
        other.0.to_native() == self.0 as u32
    }
}

impl Goff {
    /// Get a fabricated global offset for primitive types
    pub const fn prim(p: Prim) -> Self {
        let s = match p {
            Prim::Void => 0xFFFF1000,
            Prim::Bool => 0xFFFF1001,
            Prim::U8 => 0xFFFF1101,
            Prim::U16 => 0xFFFF1102,
            Prim::U32 => 0xFFFF1104,
            Prim::U64 => 0xFFFF1108,
            Prim::U128 => 0xFFFF1110,
            Prim::I8 => 0xFFFF1201,
            Prim::I16 => 0xFFFF1202,
            Prim::I32 => 0xFFFF1204,
            Prim::I64 => 0xFFFF1208,
            Prim::I128 => 0xFFFF1210,
            Prim::F32 => 0xFFFF1304,
            Prim::F64 => 0xFFFF1308,
            Prim::F128 => 0xFFFF1310,
        };
        Self(s)
    }

    pub const fn pointer() -> Self {
        Self(0xFFFF2000)
    }

    pub const fn ptmd() -> Self {
        Self(0xFFFF2001)
    }

    pub const fn ptmf() -> Self {
        Self(0xFFFF2002)
    }

    pub const fn is_prim(self) -> bool {
        return self.0 >= 0xFFFF0000;
    }
}

impl Serialize for Goff {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Goff {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        return de.deserialize_str(Visitor);
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Goff;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a hex integer literal")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                match cu::parse::<usize>(v) {
                    Ok(x) => Ok(Goff(x)),
                    Err(e) => Err(serde::de::Error::custom(format!(
                        "failed to parse Goff: {e}"
                    ))),
                }
            }
        }
    }
}

impl TreeRepr for Goff {
    fn serialize_spec(&self) -> cu::Result<String> {
        Ok(self.to_string())
    }

    fn deserialize_void() -> Self {
        Self::prim(Prim::Void)
    }

    fn deserialize_spec(spec: &str) -> cu::Result<Self> {
        Ok(Self(cu::parse::<usize>(spec)?))
    }
}

pub type GoffMapFn<'a> = Box<dyn Fn(Goff) -> cu::Result<Goff> + 'a>;
pub type GoffMap<T> = BTreeMap<Goff, T>;
pub type GoffSet = BTreeSet<Goff>;

#[rustfmt::skip]
#[derive(DebugCustom, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Display)]
#[display("({}, {})", self.0, self.1)]
#[debug("({}, {})", self.0, self.1)]
pub struct GoffPair(Goff, Goff);
impl GoffPair {
    pub fn to_pair(&self) -> (Goff, Goff) {
        (*self).into()
    }
}
impl From<(Goff, Goff)> for GoffPair {
    fn from(value: (Goff, Goff)) -> Self {
        let (a, b) = value;
        if a < b { Self(a, b) } else { Self(b, a) }
    }
}
impl From<GoffPair> for (Goff, Goff) {
    fn from(value: GoffPair) -> Self {
        (value.0, value.1)
    }
}
