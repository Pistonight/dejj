use std::collections::{BTreeMap, BTreeSet};

use cu::pre::*;

use crate::{Goff, GoffBuckets, GoffMap, GoffPair, MType};

/// Tracks dependency for merging types. After merging all deps, the merge can happen
#[derive(Debug)]
pub struct MergeTask {
    deps: Vec<GoffPair>,
    merge: GoffPair,
}
impl MergeTask {
    pub fn new(k1: Goff, k2: Goff) -> Self {
        Self {
            deps: vec![],
            merge: (k1, k2).into(),
        }
    }
    pub fn add_dep(&mut self, k1: Goff, k2: Goff) {
        if k1 == k2 || self.merge == (k1, k2).into() {
            // dep trivially satisfied
            return;
        }
        self.deps.push((k1, k2).into())
    }
    /// Update dependencies. Remove the deps that are satisfied. Return true
    /// if the deps are all satisfied and ready to merge
    pub fn update_deps(&mut self, buckets: &GoffBuckets) -> bool {
        self.deps.retain(|pair| {
            let (k1, k2) = pair.to_pair();
            buckets.primary_fallback(k1) != buckets.primary_fallback(k2)
        });
        // let entry = dep_sets.entry(self.merge_pair()).or_default();
        // for
        self.deps.is_empty()
    }

    pub fn remove_deps(&mut self, depmap: &BTreeMap<GoffPair, BTreeSet<GoffPair>>) {
        if let Some(to_remove) = depmap.get(&self.merge) {
            self.deps.retain(|pair| !to_remove.contains(pair));
        }
    }

    /// Add the dependencies to a dependency map
    pub fn track_deps(&self, depmap: &mut BTreeMap<GoffPair, BTreeSet<GoffPair>>) {
        depmap
            .entry(self.merge)
            .or_default()
            .extend(self.deps.iter().copied())
    }
    /// Execute the merge
    pub fn execute(&self, types: &mut GoffMap<MType>, buckets: &mut GoffBuckets) -> cu::Result<()> {
        let (k1, k2) = self.merge.to_pair();
        let t1 = types.get(&k1).unwrap();
        let t2 = types.get(&k2).unwrap();
        let merged = cu::check!(t1.merge_data(t2), "failed to merge types {k1} and {k2}")?;
        types.insert(k1, merged.clone());
        types.insert(k2, merged);
        cu::check!(
            buckets.merge(k1, k2),
            "failed to merge {k1} and {k2} in buckets"
        )
    }
}
