use cu::pre::*;
use tyyaml::Tree;

use crate::{Goff, GoffMap};

const UNSIZED: u32 = u32::MAX;
pub struct SizeMap {
    map: GoffMap<u32>,
    pointer_size: u32,
    ptmd_size: u32,
    ptmf_size: u32,
}

impl SizeMap {
    pub fn new(
        map: GoffMap<Option<u32>>,
        pointer_size: u32,
        ptmd_size: u32,
        ptmf_size: u32,
    ) -> Self {
        let map = map
            .into_iter()
            .map(|(k, v)| (k, v.unwrap_or(UNSIZED)))
            .collect();
        Self {
            map,
            pointer_size,
            ptmd_size,
            ptmf_size,
        }
    }
    pub fn get(&self, k: Goff) -> cu::Result<u32> {
        cu::check!(self.get_optional(k), "unexpected unsized type goff {k}")
    }
    pub fn get_optional(&self, k: Goff) -> Option<u32> {
        match *self.map.get(&k)? {
            UNSIZED => None,
            x => Some(x),
        }
    }
    pub fn get_tree(&self, tree: &Tree<Goff>) -> cu::Result<u32> {
        cu::check!(
            self.get_tree_optional(tree),
            "unexpected unsized type tree {tree:#?}"
        )
    }
    pub fn get_tree_optional(&self, tree: &Tree<Goff>) -> Option<u32> {
        tree.byte_size(self.pointer_size, self.ptmd_size, self.ptmf_size, |x| {
            self.get_optional(*x)
        })
    }
}
