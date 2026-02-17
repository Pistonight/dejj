use std::collections::{BTreeMap, BTreeSet};

use cu::pre::*;
use tyyaml::{Prim, TreeRepr};

/// Global offset into DWARF
///
/// A Goff is used as the unique identifier for a type in one extraction run.
/// However it is not stable across multiple DWARF outputs
#[rustfmt::skip]
#[derive(DebugCustom, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, From, Into, Display)]
#[display("0x{:08x}", self.0)]
#[debug("0x{:08x}", self.0)]
pub struct Goff(pub usize);

impl Goff {
    /// Get a fabricated global offset for primitive types
    pub const fn prim(p: Prim) -> Self {
        let s = match p {
            Prim::Void => 0x1FFFF0000,
            Prim::Bool => 0x1FFFF0001,
            Prim::U8 => 0x1FFFF0101,
            Prim::U16 => 0x1FFFF0102,
            Prim::U32 => 0x1FFFF0104,
            Prim::U64 => 0x1FFFF0108,
            Prim::U128 => 0x1FFFF0110,
            Prim::I8 => 0x1FFFF0201,
            Prim::I16 => 0x1FFFF0202,
            Prim::I32 => 0x1FFFF0204,
            Prim::I64 => 0x1FFFF0208,
            Prim::I128 => 0x1FFFF0210,
            Prim::F32 => 0x1FFFF0304,
            Prim::F64 => 0x1FFFF0308,
            Prim::F128 => 0x1FFFF0310,
        };
        Self(s)
    }

    pub const fn pointer() -> Self {
        Self(0x2FFFF0000)
    }

    pub const fn ptmd() -> Self {
        Self(0x2FFFF0001)
    }

    pub const fn ptmf() -> Self {
        Self(0x2FFFF0002)
    }

    pub const fn is_prim(self) -> bool {
        return self.0 >= 0x1FFFF0000;
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
