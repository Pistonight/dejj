use std::{collections::BTreeMap, path::{Path, PathBuf}, sync::Arc};

use cu::pre::*;
use exstructs::{Goff, GoffMap, GoffMapFn, GoffSet, LType, MType, NamespaceMaps, SymbolInfo, algorithm::MapGoff};

use crate::stages::{LStage, MStage};



pub struct LStageToMStageCache {
    cache_dir: PathBuf,
    cache_name: String,
    new_goffs: Vec<Goff>,
    new_data: LStageCacheData,
}

impl LStageToMStageCache {
    pub fn try_new(stage: &LStage) -> cu::Result<Self> {
        let dir = stage.config.paths.extract_output.join("l2mcache");
        let base_name = cu::check!(Path::new(&stage.name).file_name_str(), "failed to get stage name")?;
        let hash = fxhash::hash64(&stage.name);
        let cache_name = format!("{base_name}_{hash:016x}");

        let mut goffs = GoffSet::new();
        goffs.extend(stage.types.keys().copied());
        goffs.extend(stage.ns.qualifiers.keys().copied());
        goffs.extend(stage.ns.namespaces.keys().copied());
        for ns in stage.ns.qualifiers.values() {
            ns.mark_all(&mut goffs);
        }
        for ns in stage.ns.namespaces.values() {
            ns.mark_all(&mut goffs);
        }
        for ns in stage.ns.by_src.values() {
            ns.mark_all(&mut goffs);
        }
        
        // convert to vec for binary search to convert Goff to an index
        let goffs = goffs.into_iter().collect::<Vec<_>>();

        let data = LStageCacheData::try_new(stage, &goffs)?;
        Ok(Self {
            cache_dir: dir,
            cache_name,
            new_goffs: goffs, new_data: data
        })
    }

    pub fn save_cache(&self, stage: &MStage) -> cu::Result<()> {
        let data = MStageCacheData::try_new(stage, &self.new_goffs)?;
        cu::fs::write_json_pretty(self.lstage_json_path(), &self.new_data)?;
        cu::fs::write_json_pretty(self.mstage_json_path(), &data)?;
        Ok(())
    }

    pub fn load_cache(&self, stage: &LStage) -> cu::Result<Option<MStage>> {
        let lstage_path = self.lstage_json_path();
        if !lstage_path.exists() {
            return Ok(None);
        }
        let mstage_path = self.mstage_json_path();
        if !mstage_path.exists() {
            return Ok(None);
        }
        let lstage_cache = cu::fs::read_string(&lstage_path)?;
        let cache_data = json::parse::<LStageCacheData>(&lstage_cache)?;
        if self.new_data != cache_data {
        // cu::debug!("new goffs: {} {:#?}", stage.name, self.new_goffs);
        //     // let debug_path = self.lstage_json_path().with_extension(".new.json");
        //     // cu::fs::write_json_pretty(debug_path, &self.new_data)?;
        //     cu::debug!("l2mcache miss: {}: mismatch", stage.name);
        //     if self.new_data.normalized_types != cache_data.normalized_types {
        //         // cu::debug!("l2mcache miss: {}: types mismatch: {:#?} vs {:#?}", stage.name, self.new_data.normalized_types, cache_data.normalized_types);
        //         for (k, v) in &self.new_data.normalized_types {
        //         if !cache_data.normalized_types.contains_key(k) {
        //                 cu::debug!("l2mcache miss: {}: type mismatch for {k}: {v:#?} does not exist in cache", stage.name);
        //                 continue;
        //         }
        //         let v2 = cache_data.normalized_types.get(k).unwrap();
        //             if v2 != v {
        //                 cu::debug!("l2mcache miss: {}: type mismatch for {k}: {v:#?} {v2:#?}", stage.name);
        //             }
        //         }
        //     }
        //     if self.new_data.normalized_namespaces != cache_data.normalized_namespaces {
        //         cu::debug!("l2mcache miss: {}: ns mismatch", stage.name);
        //     }
        //     if self.new_data.normalized_symbols != cache_data.normalized_symbols {
        //         cu::debug!("l2mcache miss: {}: symbols mismatch", stage.name);
        //     }
            return Ok(None);
        }
        let mstage_cache = cu::fs::read_string(&mstage_path)?;
        let cache_data = json::parse::<MStageCacheData>(&mstage_cache)?;
        if cache_data.config_hash != self.new_data.config_hash {
            cu::debug!("l2mcache miss: {}: config mismatch", stage.name);
            return Ok(None);
        }

        // cache hit, de-normalize the cache with new goffs
        let stage = cu::check!(cache_data.to_mstage(stage, &self.new_goffs), "failed to hydrate mstage cache data")?;
        Ok(Some(stage))
    }

    fn lstage_json_path(&self) -> PathBuf {
        self.cache_dir.join(format!("{}_lstage.json", self.cache_name))
    }
    fn mstage_json_path(&self) -> PathBuf {
        self.cache_dir.join(format!("{}_mstage.json", self.cache_name))
    }
}

#[derive(Serialize, Deserialize)]
struct MStageCacheData {
    pub config_hash: u64,
    pub normalized_types: GoffMap<MType>,
    pub normalized_symbols: BTreeMap<String, SymbolInfo>
}

impl MStageCacheData {
    pub fn try_new(stage: &MStage, goffs: &[Goff]) -> cu::Result<Self> {
        // normalize Goffs to indices everywhere
        let normalized_types = convert(&stage.types, &goffs, goff2index)?;
        let normalized_symbols = convert_nongoff(&stage.symbols, &goffs, goff2index)?;
        Ok(Self {
            config_hash: stage.config.hash,
            normalized_types,
            normalized_symbols
        })
    }
    pub fn to_mstage(&self, stage: &LStage, goffs: &[Goff]) -> cu::Result<MStage> {
        let offset = stage.offset;
        let name = stage.name.clone();
        let config = Arc::clone(&stage.config);
        let types = convert(&self.normalized_types, goffs, index2goff)?;
        let symbols = convert_nongoff(&self.normalized_symbols, goffs, index2goff)?;
        Ok(MStage { is_cache_hit: true, offset, name, types, config, symbols })
    }
}

#[derive(PartialEq, Serialize, Deserialize)]
struct LStageCacheData {
    pub config_hash: u64,
    pub normalized_types: GoffMap<LType>,
    pub normalized_namespaces: NamespaceMaps,
    pub normalized_symbols: BTreeMap<String, SymbolInfo>
}

impl LStageCacheData {
    pub fn try_new(stage: &LStage, goffs: &[Goff]) -> cu::Result<Self> {
        // normalize Goffs to indices everywhere
        let normalized_types = convert(&stage.types, &goffs, goff2index)?;
        let normalized_ns_qualifiers = convert(&stage.ns.qualifiers, &goffs, goff2index)?;
        let normalized_ns_namespaces = convert(&stage.ns.namespaces, &goffs, goff2index)?;
        let normalized_ns_by_src = convert_nongoff(&stage.ns.by_src, &goffs, goff2index)?;
        let normalized_symbols = convert_nongoff(&stage.symbols, &goffs, goff2index)?;
        Ok(Self {
            config_hash: stage.config.hash,
            normalized_types,
            normalized_namespaces: NamespaceMaps { 
                qualifiers: normalized_ns_qualifiers,
                namespaces: normalized_ns_namespaces,
                by_src: normalized_ns_by_src,
            },
            normalized_symbols
        })
    }
}

fn convert<
T: MapGoff + Clone,
>(data: &GoffMap<T>, goffs: &[Goff], 
    convert_fn:
    fn (Goff, &[Goff]) -> cu::Result<Goff>

) -> cu::Result<GoffMap<T>> {
    let mut converted= GoffMap::new();
    let map_fn: GoffMapFn = Box::new(|k| Ok(convert_fn(k, &goffs)?));
    for (k, t) in data {
        let conv_k = convert_fn(*k, &goffs)?;
        let mut conv_t = t.clone();
        conv_t.map_goff(&map_fn)?;
        converted.insert(conv_k, conv_t);
    }

    Ok(converted)
}

fn convert_nongoff<K: Clone + Ord, T: MapGoff + Clone>(data: &BTreeMap<K, T>, goffs: &[Goff],
    convert_fn:
    fn (Goff, &[Goff]) -> cu::Result<Goff>
) -> cu::Result<BTreeMap<K, T>> {
    let mut converted= BTreeMap::new();
    let map_fn: GoffMapFn = Box::new(|k| Ok(convert_fn(k, &goffs)?));
    for (k, t) in data {
        let mut conv_t = t.clone();
        conv_t.map_goff(&map_fn)?;
        converted.insert(k.clone(), conv_t);
    }

    Ok(converted)
}

fn goff2index(goff: Goff, goffs: &[Goff]) -> cu::Result<Goff> {
    match goffs.binary_search(&goff) {
        Ok(i) => Ok(Goff(i)),
        Err(_) => cu::bail!("unexpected unmarked type {goff} when normalizing")
    }
}

fn index2goff(index: Goff, goffs: &[Goff]) -> cu::Result<Goff> {
    cu::check!(goffs.get(index.0).copied(), "index out of bound when converting to goff: {index}")
}
