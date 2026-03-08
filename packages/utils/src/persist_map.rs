use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::BTreeMap;

use cu::pre::*;
use dashmap::DashMap;
use rkyv::api::high::{HighDeserializer, HighSerializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use rkyv::{Archive, Archived, rancor};

pub struct PersistMap<K, V, S>
where
    K: Hash + Eq,
    S: PersistMapStorage<K, V>,
{
    /// File or directory to store the map
    path: PathBuf,
    /// If changes need to be persisted
    dirty: AtomicBool,
    /// in-memory working hashmap, stores modifications
    working: DashMap<K, S::Storage>,
    storage: S,
}

pub trait PersistMapStorage<K: Hash + Eq, V> {
    type Value;
    type ValueRef<'a>: Deref<Target = Self::Value>
    where
        Self: 'a,
        K: 'a;
    type Storage;
    fn open(path: &Path) -> cu::Result<(DashMap<K, Self::Storage>, Self)>
    where
        Self: Sized;
    fn save(&self, path: &Path, data: &DashMap<K, Self::Storage>) -> cu::Result<()>;
    fn get<'a, 'b, 'c>(
        &'a self,
        key: &K,
        working: &'b DashMap<K, Self::Storage>,
    ) -> cu::Result<Option<Self::ValueRef<'c>>>
    where
        'a: 'c,
        'b: 'c;
    fn to_storage(value: V) -> cu::Result<Self::Storage>;
}

impl<K, V, S> PersistMap<K, V, S>
where
    K: Hash + Eq,
    S: PersistMapStorage<K, V>,
{
    pub fn open(path: &Path) -> cu::Result<Self> {
        let (working, storage) = cu::check!(
            S::open(path),
            "failed to open persisted map from '{}'",
            path.display()
        )?;
        Ok(Self {
            path: path.to_path_buf(),
            dirty: AtomicBool::new(false),
            working,
            storage,
        })
    }

    pub fn get(&self, key: &K) -> cu::Result<Option<S::ValueRef<'_>>> {
        self.storage.get(key, &self.working)
    }

    pub fn set(&self, key: K, value: V) -> cu::Result<()> {
        let value = S::to_storage(value)?;
        self.working.insert(key, value);
        self.dirty.store(true, Ordering::Release);
        Ok(())
    }

    pub fn save(&self) -> cu::Result<()> {
        if !self.dirty.load(Ordering::Acquire) {
            return Ok(());
        }
        self.storage.save(&self.path, &self.working)
    }
}

pub struct JsonFileStorage;
impl<K, V> PersistMapStorage<K, V> for JsonFileStorage
where
    for<'de> K: Hash + Eq + Serialize + Deserialize<'de> + 'static,
    for<'de> V: Serialize + Deserialize<'de> + 'static,
{
    type Value = V;
    type ValueRef<'a> = dashmap::mapref::one::Ref<'a, K, V>;
    type Storage = V;

    fn open(path: &Path) -> cu::Result<(DashMap<K, V>, Self)>
    where
        Self: Sized,
    {
        match path.metadata() {
            Err(_) => {
                return Ok((Default::default(), Self));
            }
            Ok(meta) => {
                if meta.is_dir() {
                    cu::fs::rec_remove(path)?;
                    return Ok((Default::default(), Self));
                }
            }
        }
        let s = cu::fs::read_string(path)?;
        let map = json::parse::<DashMap<K, V>>(&s)?;
        Ok((map, Self))
    }
    fn save(&self, path: &Path, data: &DashMap<K, V>) -> cu::Result<()> {
        cu::fs::write_json_pretty(path, data)
    }
    fn get<'a, 'b, 'c>(
        &'a self,
        key: &K,
        working: &'b DashMap<K, Self::Storage>,
    ) -> cu::Result<Option<Self::ValueRef<'c>>>
    where
        'a: 'c,
        'b: 'c,
    {
        Ok(working.get(key))
    }
    fn to_storage(value: V) -> cu::Result<Self::Storage> {
        Ok(value)
    }
}

pub struct JsonDirStorage;
impl<V> PersistMapStorage<String, V> for JsonDirStorage
where
    for<'de> V: Serialize + Deserialize<'de> + 'static,
{
    type Value = V;
    type ValueRef<'a> = dashmap::mapref::one::Ref<'a, String, V>;
    type Storage = V;

    fn open(path: &Path) -> cu::Result<(DashMap<String, V>, Self)>
    where
        Self: Sized,
    {
        match path.metadata() {
            Err(_) => {
                return Ok((Default::default(), Self));
            }
            Ok(meta) => {
                if !meta.is_dir() {
                    cu::fs::remove(path)?;
                    return Ok((Default::default(), Self));
                }
            }
        }
        let map = DashMap::new();
        for entry in cu::fs::read_dir(path)? {
            let entry = entry?;
            let file_name = entry.file_name().into_utf8()?;
            let Some(key) = file_name.strip_suffix(".json") else {
                continue;
            };
            let s = cu::fs::read_string(entry.path())?;
            let value = json::parse::<V>(&s)?;
            map.insert(key.to_string(), value);
        }
        Ok((map, Self))
    }
    fn save(&self, path: &Path, data: &DashMap<String, V>) -> cu::Result<()> {
        let mut contents = Vec::with_capacity(data.len());
        // do not hold the map during IO
        {
            for entry in data.iter() {
                let file_name = format!("{}.json", entry.key());
                contents.push((path.join(file_name), json::stringify_pretty(entry.value())?));
            }
        }
        for (path, content) in contents {
            cu::fs::write(path, content)?;
        }
        Ok(())
    }
    fn get<'a, 'b, 'c>(
        &'a self,
        key: &String,
        working: &'b DashMap<String, Self::Storage>,
    ) -> cu::Result<Option<Self::ValueRef<'c>>>
    where
        'a: 'c,
        'b: 'c,
    {
        Ok(working.get(key))
    }
    fn to_storage(value: V) -> cu::Result<Self::Storage> {
        Ok(value)
    }
}

#[derive(Default)]
pub struct BinaryFileStorage(AlignedVec);
impl<K, V> PersistMapStorage<K, V> for BinaryFileStorage
where
    for<'a> K: Clone
        + Hash
        + Eq
        + Ord
        + Archive
        + rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, rancor::Error>>
        + 'static,
    for<'a> Archived<K>: Hash
        + Eq
        + Ord
        + CheckBytes<HighValidator<'a, rancor::Error>>
        + rkyv::Deserialize<K, HighDeserializer<rancor::Error>>,
    for<'a> V: Archive
        + rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, rancor::Error>>
        + 'static,
    for<'a> Archived<V>: CheckBytes<HighValidator<'a, rancor::Error>>
        + rkyv::Deserialize<V, HighDeserializer<rancor::Error>>,
{
    type Value = Archived<V>;
    type ValueRef<'a> = RkyvAccessor<'a, K, V>;
    type Storage = AlignedVec;

    fn open(path: &Path) -> cu::Result<(DashMap<K, Self::Storage>, Self)>
    where
        Self: Sized,
    {
        match path.metadata() {
            Err(_) => {
                return Ok(Default::default());
            }
            Ok(meta) => {
                if meta.is_dir() {
                    cu::fs::rec_remove(path)?;
                    return Ok(Default::default());
                }
            }
        }
            let mut bytes = AlignedVec::new();
            bytes.extend_from_reader(&mut cu::fs::reader(path)?)?;
        // validate the bytes
        let archived = rkyv::access::<Archived<BTreeMap<K, V>>, rancor::Error>(&bytes);
        cu::check!(archived, "invalid binary cache file: '{}'", path.display())?;

        Ok((Default::default(), Self(bytes)))
    }

    fn save(&self, path: &Path, new_data: &DashMap<K, Self::Storage>) -> cu::Result<()> {
        let mut data = if self.0.is_empty() {
            Default::default()
        } else {
            // safety: checked in constructor
            let archived = unsafe { rkyv::access_unchecked::<Archived<BTreeMap<K, V>>>(&self.0) };
            // deserialize so we can combine the data
            rkyv::deserialize::<BTreeMap<K, V>, rancor::Error>(archived)?
        };
        for e in new_data {
            let new_value_archived = rkyv::access::<Archived<V>, rancor::Error>(e.value())?;
            let new_value = rkyv::deserialize::<V, rancor::Error>(new_value_archived)?;
            data.insert(e.key().clone(), new_value);
        }
        // serialize the resulting map
        let bytes = rkyv::to_bytes(&data)?;
        cu::fs::write(path, bytes)
    }

    fn get<'a, 'b, 'c>(
        &'a self,
        key: &K,
        working: &'b DashMap<K, Self::Storage>,
    ) -> cu::Result<Option<Self::ValueRef<'c>>>
    where
        'a: 'c,
        'b: 'c,
    {
        // Check working copy first (dirty/newly inserted entries)
        if let Some(entry) = working.get(key) {
            rkyv::access::<Archived<V>, rancor::Error>(entry.value())?;
            return Ok(Some(RkyvAccessor::DashRef(entry)));
        }
        if self.0.is_empty() {
            return Ok(None);
        }
        // SAFETY: bytes were validated in open()
        let archived_map = unsafe { rkyv::access_unchecked::<Archived<BTreeMap<K, V>>>(&self.0) };
        let archived_key = rkyv::to_bytes(key)?;
        let archived_key = rkyv::access(&archived_key)?;
        let value = archived_map.get(archived_key);
        Ok(value.map(RkyvAccessor::Archived))
    }

    fn to_storage(value: V) -> cu::Result<Self::Storage> {
        cu::check!(rkyv::to_bytes(&value), "failed to archive new value")
    }
}

pub enum RkyvAccessor<'a, K, V: Archive> {
    DashRef(dashmap::mapref::one::Ref<'a, K, AlignedVec>),
    Archived(&'a Archived<V>),
}
impl<K: Hash + Eq, V: Archive> Deref for RkyvAccessor<'_, K, V> {
    type Target = Archived<V>;
    fn deref(&self) -> &Self::Target {
        match self {
            RkyvAccessor::DashRef(x) => {
                // safety: checked when constructing accessor
                unsafe { rkyv::access_unchecked(x.value()) }
            }
            RkyvAccessor::Archived(x) => x,
        }
    }
}

#[derive(Default)]
pub struct BinaryDirStorage;
impl<V> PersistMapStorage<String, V> for BinaryDirStorage
where
    for<'a> V: Archive
        + rkyv::Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, rancor::Error>>
        + 'static,
    for<'a> Archived<V>: CheckBytes<HighValidator<'a, rancor::Error>>
        + rkyv::Deserialize<V, HighDeserializer<rancor::Error>>,
{
    type Value = Archived<V>;
    type ValueRef<'a> = DashRefRkyvAccessor<'a, String, V>;
    type Storage = AlignedVec;

    fn open(path: &Path) -> cu::Result<(DashMap<String, Self::Storage>, Self)>
    where
        Self: Sized {
        match path.metadata() {
            Err(_) => {
                return Ok(Default::default());
            }
            Ok(meta) => {
                if !meta.is_dir() {
                    cu::fs::remove(path)?;
                    return Ok(Default::default());
                }
            }
        }
        let map = DashMap::new();
        for entry in cu::fs::read_dir(path)? {
            let entry = entry?;
            let file_name = entry.file_name().into_utf8()?;
            let Some(key) = file_name.strip_suffix(".bin") else {
                continue;
            };
            let mut bytes = AlignedVec::new();
            bytes.extend_from_reader(&mut cu::fs::reader(entry.path())?)?;
            map.insert(key.to_string(), bytes);
        }

        Ok((map, Self))
    }

    fn save(&self, path: &Path, data: &DashMap<String, Self::Storage>) -> cu::Result<()> {
        for entry in data {
            let file_path = path.join(format!("{}.bin", entry.key()));
            cu::fs::write(file_path, entry.value())?;
        }
        Ok(())
    }

    fn get<'a, 'b, 'c>(
        &'a self,
        key: &String,
        working: &'b DashMap<String, Self::Storage>,
    ) -> cu::Result<Option<Self::ValueRef<'c>>>
    where
        'a: 'c,
        'b: 'c {
        let entry = cu::some!(working.get(key));
        rkyv::access::<Archived<V>, rancor::Error>(entry.value())?;
        Ok(Some(DashRefRkyvAccessor(entry, PhantomData)))
    }

    fn to_storage(value: V) -> cu::Result<Self::Storage> {
        cu::check!(rkyv::to_bytes(&value), "failed to archive new value")
    }
}

pub struct DashRefRkyvAccessor<'a, K, V: Archive>(dashmap::mapref::one::Ref<'a, K, AlignedVec>, PhantomData<V>);
impl<K: Hash + Eq, V: Archive> Deref for DashRefRkyvAccessor<'_, K, V> {
    type Target = Archived<V>;
    fn deref(&self) -> &Self::Target {
        unsafe { rkyv::access_unchecked(self.0.value()) }
    }
}
