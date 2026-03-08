use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use cu::pre::*;
use dashmap::DashMap;
use dejj_utils::{
    Config,
    persist_map::{BinaryFileStorage, JsonDirStorage, PersistMap, PersistMapStorage},
};
use exstructs::{
    Goff, GoffMap, GoffMapFn, GoffSet, LType, MType, NamespaceMaps, SymbolInfo, algorithm::MapGoff,
};
use rkyv::rancor;

use crate::stages::{LStage, MStage};

pub enum L2mCache {
    Json(L2mCacheCore<JsonDirStorage>),
    Binary(L2mCacheCore<BinaryFileStorage>),
}
impl L2mCache {
    pub fn open(config: &Config) -> cu::Result<Self> {
        let bar = cu::progress("loading l2mcache").keep(false).spawn();
        if config.extract.debug.l2mcache {
            cu::hint!("l2mcache debugging is enabled, the cache will be stored in JSON");
            let cache_location = config.paths.extract_output.join("l2mcache.json");
            let cache = L2mCacheCore::open(&cache_location)?;
            bar.done();
            Ok(Self::Json(cache))
        } else {
            let cache_location = config.paths.extract_output.join("l2mcache.bin");
            let cache = L2mCacheCore::open(&cache_location)?;
            bar.done();
            Ok(Self::Binary(cache))
        }
    }
    pub fn get(&self, lstage: &LStage) -> cu::Result<Option<MStage>> {
        let (cache_key, goffs) = self.preprocess_lstage(lstage)?;
        let ldata = LStageCacheData::try_new(lstage, &goffs)?;
        let mstage = match self {
            L2mCache::Json(c) => c.try_restore_from_cache(lstage, &cache_key, &ldata, &goffs)?,
            L2mCache::Binary(c) => c.try_restore_from_cache(lstage, &cache_key, &ldata, &goffs)?,
        };
        if mstage.is_some() {
            return Ok(mstage);
        }
        // insert the goffs and ldata into the cache for later
        let entry = L2mCacheEntry {
            goffs,
            ldata,
            mdata: Default::default(),
        };
        match self {
            L2mCache::Json(c) => {
                c.working.insert(cache_key, entry);
            }
            L2mCache::Binary(c) => {
                c.working.insert(cache_key, entry);
            }
        }
        Ok(None)
    }
    pub fn set(&self, mstage: &MStage) -> cu::Result<()> {
        let cache_key = Self::cache_key(&mstage.name)?;
        match self {
            L2mCache::Json(c) => c.set(cache_key, mstage),
            L2mCache::Binary(c) => c.set(cache_key, mstage),
        }
    }
    pub fn save(&self) -> cu::Result<()> {
        match self {
            L2mCache::Json(c) => c.save(),
            L2mCache::Binary(c) => c.save(),
        }
    }
    fn preprocess_lstage(&self, lstage: &LStage) -> cu::Result<(String, Vec<Goff>)> {
        let cache_key = Self::cache_key(&lstage.name)?;

        let mut goffs = GoffSet::new();
        goffs.extend(lstage.types.keys().copied());
        goffs.extend(lstage.ns.qualifiers.keys().copied());
        goffs.extend(lstage.ns.namespaces.keys().copied());
        for ns in lstage.ns.qualifiers.values() {
            ns.mark_all(&mut goffs);
        }
        for ns in lstage.ns.namespaces.values() {
            ns.mark_all(&mut goffs);
        }
        for ns in lstage.ns.by_src.values() {
            ns.mark_all(&mut goffs);
        }
        // convert to vec for binary search to convert Goff to an index
        let goffs = goffs.into_iter().collect::<Vec<_>>();
        Ok((cache_key, goffs))
    }

    fn cache_key(name: &str) -> cu::Result<String> {
        let base_name = cu::check!(Path::new(name).file_name_str(), "failed to get stage name")?;
        let hash = fxhash::hash64(name);
        Ok(format!("{base_name}_{hash:016x}"))
    }
}

/// Cache from LStage to MStage (stage0 -> stage1)
pub struct L2mCacheCore<S: PersistMapStorage<String, L2mCacheEntry>> {
    store: PersistMap<String, L2mCacheEntry, S>,
    working: DashMap<String, L2mCacheEntry>,
}
impl<S: PersistMapStorage<String, L2mCacheEntry>> L2mCacheCore<S> {
    pub fn open(path: &Path) -> cu::Result<Self> {
        let store = PersistMap::open(path)?;
        Ok(Self {
            store,
            working: Default::default(),
        })
    }

    pub fn set(&self, cache_key: String, mstage: &MStage) -> cu::Result<()> {
        let (_, mut entry) = cu::check!(
            self.working.remove(&cache_key),
            "cannot find working cache entry. get() must be called before trying to set()"
        )?;
        let mdata = MStageCacheData::try_new(mstage, &entry.goffs)?;
        entry.mdata = mdata;
        self.store.set(cache_key, entry)?;
        Ok(())
    }

    pub fn save(&self) -> cu::Result<()> {
        cu::check!(self.store.save(), "failed to save l2mcache")
    }
}
impl L2mCacheCore<JsonDirStorage> {
    fn try_restore_from_cache(
        &self,
        lstage: &LStage,
        cache_key: &String,
        ldata: &LStageCacheData,
        goffs: &[Goff],
    ) -> cu::Result<Option<MStage>> {
        // see if an cached entry exists
        let cached = cu::check!(self.store.get(&cache_key), "error accessing json l2mcache")?;
        let cached = cu::some!(cached);
        // if config changed, cache is not valid
        if cached.mdata.config_hash != lstage.config.hash {
            return Ok(None);
        }
        // if data changed, cache is stale
        if cached.ldata != *ldata {
            return Ok(None);
        }
        // cache hit, restore MStage
        let mdata = &cached.mdata;
        let mstage = cu::check!(
            mdata.to_mstage(lstage, goffs),
            "failed to restore mstage from cache"
        )?;
        Ok(Some(mstage))
    }
}

impl L2mCacheCore<BinaryFileStorage> {
    fn try_restore_from_cache(
        &self,
        lstage: &LStage,
        cache_key: &String,
        ldata: &LStageCacheData,
        goffs: &[Goff],
    ) -> cu::Result<Option<MStage>> {
        // see if an cached entry exists
        let cached = cu::check!(self.store.get(&cache_key), "error accessing json l2mcache")?;
        let cached = cu::some!(cached);
        // if config changed, cache is not valid
        if cached.mdata.config_hash != lstage.config.hash {
            return Ok(None);
        }
        // if data changed, cache is stale
        if cached.ldata != *ldata {
            return Ok(None);
        }
        // cache hit, restore MStage
        let mdata = &cached.mdata;
        let mdata = cu::check!(
            rkyv::deserialize::<MStageCacheData, rancor::Error>(mdata),
            "failed to deserialize mstage from binary cache"
        )?;
        let mstage = cu::check!(
            mdata.to_mstage(lstage, goffs),
            "failed to restore mstage from cache"
        )?;
        Ok(Some(mstage))
    }
}

#[derive(Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct L2mCacheEntry {
    #[rkyv(with = rkyv::with::Skip)]
    #[serde(skip)]
    goffs: Vec<Goff>,
    ldata: LStageCacheData,
    mdata: MStageCacheData,
}
//
// pub struct LStageToMStageCache {
//     cache_dir: PathBuf,
//     cache_name: String,
//     new_goffs: Vec<Goff>,
//     new_data: LStageCacheData,
// }
//
// impl LStageToMStageCache {
//     pub fn try_new(stage: &LStage) -> cu::Result<Self> {
//         let dir = stage.config.paths.extract_output.join("l2mcache");
//         let base_name = cu::check!(Path::new(&stage.name).file_name_str(), "failed to get stage name")?;
//         let hash = fxhash::hash64(&stage.name);
//         let cache_name = format!("{base_name}_{hash:016x}");
//
//
//         let data = LStageCacheData::try_new(stage, &goffs)?;
//         Ok(Self {
//             cache_dir: dir,
//             cache_name,
//             new_goffs: goffs, new_data: data
//         })
//     }
//
//     pub fn save_cache(&self, stage: &MStage) -> cu::Result<()> {
//         let data = MStageCacheData::try_new(stage, &self.new_goffs)?;
//         cu::fs::write_json_pretty(self.lstage_json_path(), &self.new_data)?;
//         cu::fs::write_json_pretty(self.mstage_json_path(), &data)?;
//         Ok(())
//     }
//
//     pub fn load_cache(&self, stage: &LStage) -> cu::Result<Option<MStage>> {
//         let lstage_path = self.lstage_json_path();
//         if !lstage_path.exists() {
//             return Ok(None);
//         }
//         let mstage_path = self.mstage_json_path();
//         if !mstage_path.exists() {
//             return Ok(None);
//         }
//         let lstage_cache = cu::fs::read_string(&lstage_path)?;
//         let cache_data = json::parse::<LStageCacheData>(&lstage_cache)?;
//         if self.new_data != cache_data {
//             return Ok(None);
//         }
//         let mstage_cache = cu::fs::read_string(&mstage_path)?;
//         let cache_data = json::parse::<MStageCacheData>(&mstage_cache)?;
//         if cache_data.config_hash != self.new_data.config_hash {
//             cu::debug!("l2mcache miss: {}: config mismatch", stage.name);
//             return Ok(None);
//         }
//
//         // cache hit, de-normalize the cache with new goffs
//         let stage = cu::check!(cache_data.to_mstage(stage, &self.new_goffs), "failed to hydrate mstage cache data")?;
//         Ok(Some(stage))
//     }
//
//     fn lstage_json_path(&self) -> PathBuf {
//         self.cache_dir.join(format!("{}_lstage.json", self.cache_name))
//     }
//     fn mstage_json_path(&self) -> PathBuf {
//         self.cache_dir.join(format!("{}_mstage.json", self.cache_name))
//     }
// }

#[derive(Default, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
struct MStageCacheData {
    pub config_hash: u64,
    pub normalized_types: GoffMap<MType>,
    pub normalized_symbols: BTreeMap<String, SymbolInfo>,
}

impl MStageCacheData {
    pub fn try_new(stage: &MStage, goffs: &[Goff]) -> cu::Result<Self> {
        // normalize Goffs to indices everywhere
        let normalized_types = convert(&stage.types, &goffs, goff2index)?;
        let normalized_symbols = convert_nongoff(&stage.symbols, &goffs, goff2index)?;
        Ok(Self {
            config_hash: stage.config.hash,
            normalized_types,
            normalized_symbols,
        })
    }
    pub fn to_mstage(&self, stage: &LStage, goffs: &[Goff]) -> cu::Result<MStage> {
        let offset = stage.offset;
        let name = stage.name.clone();
        let config = Arc::clone(&stage.config);
        let types = convert(&self.normalized_types, goffs, index2goff)?;
        let symbols = convert_nongoff(&self.normalized_symbols, goffs, index2goff)?;
        Ok(MStage {
            is_cache_hit: true,
            offset,
            name,
            types,
            config,
            symbols,
        })
    }
}

#[derive(PartialEq, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(compare(PartialEq))]
struct LStageCacheData {
    pub config_hash: u64,
    pub normalized_types: GoffMap<LType>,
    pub normalized_namespaces: NamespaceMaps,
    pub normalized_symbols: BTreeMap<String, SymbolInfo>,
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
            normalized_symbols,
        })
    }
}

fn convert<T: MapGoff + Clone>(
    data: &GoffMap<T>,
    goffs: &[Goff],
    convert_fn: fn(Goff, &[Goff]) -> cu::Result<Goff>,
) -> cu::Result<GoffMap<T>> {
    let mut converted = GoffMap::new();
    let map_fn: GoffMapFn = Box::new(|k| Ok(convert_fn(k, &goffs)?));
    for (k, t) in data {
        let conv_k = convert_fn(*k, &goffs)?;
        let mut conv_t = t.clone();
        conv_t.map_goff(&map_fn)?;
        converted.insert(conv_k, conv_t);
    }

    Ok(converted)
}

fn convert_nongoff<K: Clone + Ord, T: MapGoff + Clone>(
    data: &BTreeMap<K, T>,
    goffs: &[Goff],
    convert_fn: fn(Goff, &[Goff]) -> cu::Result<Goff>,
) -> cu::Result<BTreeMap<K, T>> {
    let mut converted = BTreeMap::new();
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
        Err(_) => cu::bail!("unexpected unmarked type {goff} when normalizing"),
    }
}

fn index2goff(index: Goff, goffs: &[Goff]) -> cu::Result<Goff> {
    cu::check!(
        goffs.get(index.0).copied(),
        "index out of bound when converting to goff: {index}"
    )
}
