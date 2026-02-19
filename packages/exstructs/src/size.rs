use cu::pre::*;

use crate::{Goff, GoffMap};

const UNSIZED: u32 = u32::MAX;
pub struct SizeMap(GoffMap<u32>);
impl From<GoffMap<Option<u32>>> for SizeMap {
    fn from(value: GoffMap<Option<u32>>) -> Self {
        Self(
            value
                .into_iter()
                .map(|(k, v)| (k, v.unwrap_or(UNSIZED)))
                .collect(),
        )
    }
}
impl SizeMap {
    pub fn get(&self, k: Goff) -> cu::Result<u32> {
        cu::check!(self.get_optional(k), "unexpected unsized type goff {k}")
    }
    pub fn get_optional(&self, k: Goff) -> Option<u32> {
        match *self.0.get(&k)? {
            UNSIZED => None,
            x => Some(x),
        }
    }
}
