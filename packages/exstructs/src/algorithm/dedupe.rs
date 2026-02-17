use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use cu::pre::*;
use fxhash::FxHasher;

use crate::{GoffBuckets, GoffMap, GoffMapFn, NamespaceMaps, SymbolInfo};

pub fn dedupe<T: Eq + Hash + std::fmt::Debug, FMap: Fn(&mut T, &GoffBuckets) -> cu::Result<()>>(
    map: GoffMap<T>,
    buckets: GoffBuckets,
    symbols: &mut BTreeMap<String, SymbolInfo>,
    namespace: Option<&mut NamespaceMaps>,
    mapper: FMap,
) -> cu::Result<GoffMap<T>> {
    merging_dedupe(map, buckets, symbols, namespace, mapper, |a, b| {
        cu::bail!(
            "the data are not equal after mapping, please check the mapper implementation.\na={:#?}, b={:#?}. If this is expected, a merger must be provided to do dedupe-time merging.",
            a,
            b
        );
    })
}

/// Dedupe goffs that map to the same type data
///
/// `buckets` should contain types to merge in the same bucket if merging is needed.
/// If types to merge are not equal, a merger must be provided (use `dedupe()` otherwise).
///
/// Other than that, types that are strictly equal (i.e. `==`) will also be deduped
pub fn merging_dedupe<
    T: Eq + Hash + std::fmt::Debug,
    FMap: Fn(&mut T, &GoffBuckets) -> cu::Result<()>,
    FMerge: Fn(&T, &T) -> cu::Result<T>,
>(
    // map that contains all the types
    mut map: GoffMap<T>,
    // buckets that contain types to merge
    mut buckets: GoffBuckets,
    // symbol data to be modified as merge happens
    symbols: &mut BTreeMap<String, SymbolInfo>,
    // namespace data to be modified as merge happens
    namespace: Option<&mut NamespaceMaps>,
    // mapper to get the primary key for a given type
    mapper: FMap,
    // the merge function to merge 2 types if they are not equal (not called if equal)
    merger: FMerge,
) -> cu::Result<GoffMap<T>> {
    loop {
        // must run mapper first to make sure collision and merge check
        // picks up the change
        let mut new_map = GoffMap::default();
        for (goff, mut t) in map {
            use std::collections::btree_map::Entry;

            let k = buckets.primary_fallback(goff);
            cu::check!(
                mapper(&mut t, &buckets),
                "failed to run mapper for {goff} (primary: {k})"
            )?;
            match new_map.entry(k) {
                Entry::Occupied(mut e) => {
                    if e.get() == &t {
                        continue;
                    }
                    let merged = cu::check!(
                        merger(e.get(), &t),
                        "failed to merge {goff} into {k}, dedupe-time merging failed"
                    )?;
                    e.insert(merged);
                }
                Entry::Vacant(e) => {
                    e.insert(t);
                }
            }
        }

        map = new_map;

        let mut hash_map = BTreeMap::default();
        let mut has_collision = false;
        for (goff, t) in &map {
            let mut hasher = FxHasher::default();
            t.hash(&mut hasher);
            let h = hasher.finish();
            let set = hash_map.entry(h).or_insert_with(|| {
                has_collision = true;
                BTreeSet::new()
            });
            set.insert(*goff);
        }

        let mut has_merges = false;
        if has_collision {
            for keys in hash_map.into_values() {
                let keys = keys.into_iter().collect::<Vec<_>>();
                for (i, k) in keys.iter().copied().enumerate() {
                    for j in keys.iter().skip(i + 1).copied() {
                        let t1 = map.get(&k).unwrap();
                        let t2 = map.get(&j).unwrap();
                        if t1 == t2 {
                            cu::check!(buckets.merge(k, j), "failed to merge goff {k} and {j}")?;
                            has_merges = true;
                        }
                    }
                }
            }
        }

        if has_merges {
            continue;
        }

        let f: GoffMapFn = Box::new(|k| Ok(buckets.primary_fallback(k)));
        for symbol in symbols.values_mut() {
            cu::check!(symbol.map_goff(&f), "symbol mapping failed when deduping")?;
        }

        if let Some(ns) = namespace {
            for n in ns.qualifiers.values_mut() {
                cu::check!(n.map_goff(&f), "qualifier mapping failed when deduping")?;
            }
            for n in ns.namespaces.values_mut() {
                cu::check!(n.map_goff(&f), "namespace mapping failed when deduping")?;
            }
            for n in ns.by_src.values_mut() {
                cu::check!(
                    n.map_goff(&f),
                    "namespace by_src mapping failed when deduping"
                )?;
            }
        }

        return Ok(map);
    }
}
