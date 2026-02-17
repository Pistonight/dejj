use cu::pre::*;

use crate::{Goff, GoffMap, GoffSet};

/// A structure stores Goffs in mutually exclusive sets.
/// Each set is an equivalence class (all Goffs in the set are equivalent)
#[derive(Default)]
pub struct GoffBuckets {
    index_map: GoffMap<usize>,
    buckets: Vec<GoffSet>,
    free_list: Vec<usize>,
}

impl GoffBuckets {
    /// Check if `k` is in any bucket
    pub fn contains(&self, k: Goff) -> bool {
        self.primary(k).is_some()
    }

    /// Get the smallest goff in the bucket `k` is in.
    /// Returns the input if it's not in any bucket
    pub fn primary_fallback(&self, k: Goff) -> Goff {
        self.primary(k).unwrap_or(k)
    }

    /// Get the primary goff in the bucket `k` is in, if it is in any bucket.
    /// If the goff corresponds to a primitive, then the canonical value
    /// for the primitive is returned. Otherwise, the smallest goff is returned.
    pub fn primary(&self, k: Goff) -> Option<Goff> {
        let i = *self.index_map.get(&k)?;
        self.primary_from_index(i)
    }

    pub fn primaries(&self) -> impl Iterator<Item = Goff> {
        self.buckets
            .iter()
            .filter(|x| !x.is_empty())
            .filter_map(Self::primary_from_bucket)
    }

    /// Insert a new goff to the buckets.
    ///
    /// Returns `None` if the goff is inserted as a new bucket.
    /// Returns `Some(k)` if the goff is already in a bucket,
    /// and `k` is the smallest `Goff` in that bucket.
    #[must_use]
    pub fn insert(&mut self, k: Goff) -> Option<Goff> {
        match self.index_map.get(&k).copied() {
            None => {
                let i = self.make_new_bucket();
                self.buckets[i].insert(k);
                self.index_map.insert(k, i);
                None
            }
            Some(i) => self.primary_from_index(i),
        }
    }

    /// Returns Some(k) where k is the primary of the bucket at index i
    #[inline(always)]
    fn primary_from_index(&self, i: usize) -> Option<Goff> {
        if cfg!(debug_assertions) {
            let bucket = &self.buckets[i];
            Self::primary_from_bucket(bucket)
        } else {
            let bucket = self.buckets.get(i)?;
            Self::primary_from_bucket(bucket)
        }
    }

    #[inline(always)]
    fn primary_from_bucket(bucket: &GoffSet) -> Option<Goff> {
        if cfg!(debug_assertions) {
            let largest = *bucket.last().expect("unexpected empty bucket");
            if largest.is_prim() {
                return Some(largest);
            }
            Some(bucket.first().copied().expect("unexpected empty bucket"))
        } else {
            let largest = *bucket.last()?;
            if largest.is_prim() {
                return Some(largest);
            }
            bucket.first().copied()
        }
    }

    /// Merge the 2 buckets `k1` and `k2`, insert the bucket as new if `k1`
    /// or `k2` are not in any buckets
    pub fn merge(&mut self, k1: Goff, k2: Goff) -> cu::Result<()> {
        let k1_primary = self.insert(k1).unwrap_or(k1);
        let k2_primary = self.insert(k2).unwrap_or(k2);
        if k1_primary == k2_primary {
            // already in the same bucket
            return Ok(());
        }
        let (k_to, k_from) = cu::check!(
            pick_bucket_primary_key(k1_primary, k2_primary),
            "merge: failed to pick primary key when merging {k1} and {k2}"
        )?;
        let i_from = *cu::check!(
            self.index_map.get(&k_from),
            "merge: unexpected k_from not found: {k_from}"
        )?;
        let i_to = *cu::check!(
            self.index_map.get(&k_to),
            "merge: unexpected k_to not found: {k_to}"
        )?;
        let keys_from = self.remove_bucket(i_from);
        for k in &keys_from {
            self.index_map.insert(*k, i_to);
        }
        self.buckets[i_to].extend(keys_from);
        Ok(())
    }

    fn make_new_bucket(&mut self) -> usize {
        match self.free_list.pop() {
            None => {
                let i = self.buckets.len();
                self.buckets.push(Default::default());
                i
            }
            Some(i) => i,
        }
    }

    fn remove_bucket(&mut self, i: usize) -> GoffSet {
        self.free_list.push(i);
        std::mem::take(&mut self.buckets[i])
    }
}

/// Determistically determine the priority of 2 Goffs. Returns (k1, k2)
/// where k1 is the one with more priority (the primary key) and k2 is the other one
pub fn pick_bucket_primary_key(k1: Goff, k2: Goff) -> cu::Result<(Goff, Goff)> {
    if k1 == k2 {
        return Ok((k1, k2));
    }
    match (k1.is_prim(), k2.is_prim()) {
        (true, true) => {
            cu::bail!("cannot have 2 different primitive goffs in the same bucket: {k1} and {k2}")
        }
        (true, false) => Ok((k1, k2)),
        (false, true) => Ok((k2, k1)),
        (false, false) => Ok(if k1 < k2 { (k1, k2) } else { (k2, k1) }),
    }
}
