// TODO: figure out why dashmap is faster (perhaps just because it saves more, instead of overwriting)
// and then implement that into ours, making ours actually faster

#![allow(unused)]

use hashbrown::HashMap;
use nohash_hasher::BuildNoHashHasher;

#[allow(unused)]
pub trait CacheTrait {
    fn cache_get(&self, z_hash: u64) -> Option<Item>;
    fn cache_insert(&mut self, z_hash: u64, item: Item);
    fn cache_new() -> Self;
}

const CACHE_SIZE_MB: usize = 2_usize.pow(4);
const CACHE_SIZE_ENTRIES: usize = (CACHE_SIZE_MB * 1024 * 1024) / size_of::<CacheBucket>();
const _: () = assert!(size_of::<CacheEntry>() == 16);
const _: () = assert!(size_of::<CacheBucket>() == 32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheBound {
    Exact = 0,
    Lower = 1,
    Upper = 2,
}

type Item = (i64, u16, CacheBound);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CacheEntry {
    score: i64,
    depth: u16,
    bound: CacheBound,
    key_upper: u32,
}

impl CacheEntry {
    pub fn is_empty(&self) -> bool {
        self.key_upper == 0
    }

    pub fn to_item(&self) -> Item {
        (self.score, self.depth, self.bound)
    }

    pub fn from_item(item: Item, key_upper: u32) -> Self {
        Self {
            score: item.0,
            depth: item.1,
            bound: item.2,
            key_upper,
        }
    }
}

impl Default for CacheEntry {
    fn default() -> Self {
        Self {
            score: 0,
            depth: 0,
            key_upper: 0,
            bound: CacheBound::Exact,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CacheBucket {
    fresh: CacheEntry,
    deep: CacheEntry,
}

#[derive(Debug, Clone)]
pub struct Cache {
    entries: Box<[CacheBucket]>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            entries: vec![CacheBucket::default(); CACHE_SIZE_ENTRIES].into_boxed_slice(),
        }
    }

    fn get_index(z_hash: u64) -> usize {
        z_hash as usize & (CACHE_SIZE_ENTRIES - 1)
    }

    #[inline]
    fn partial_key(z_hash: u64) -> u32 {
        (z_hash >> 32) as u32
    }

    pub fn insert(&mut self, z_hash: u64, item: Item) {
        let bucket = &mut self.entries[Self::get_index(z_hash)];
        let new_entry = CacheEntry::from_item(item, Self::partial_key(z_hash));

        bucket.fresh = new_entry;

        if item.1 >= bucket.deep.depth || bucket.deep.is_empty() {
            bucket.deep = new_entry;
        }
    }

    pub fn get(&self, z_hash: u64) -> Option<Item> {
        let pk = Self::partial_key(z_hash);
        let bucket = &self.entries[Self::get_index(z_hash)];

        if bucket.fresh.key_upper == pk {
            return Some(bucket.fresh.to_item());
        }

        if bucket.deep.key_upper == pk {
            return Some(bucket.deep.to_item());
        }

        None
    }
}

impl CacheTrait for Cache {
    fn cache_get(&self, z_hash: u64) -> Option<Item> {
        self.get(z_hash)
    }
    fn cache_insert(&mut self, z_hash: u64, item: Item) {
        self.insert(z_hash, item);
    }
    fn cache_new() -> Self {
        Self::new()
    }
}

pub type DashCache = HashMap<u64, Item, BuildNoHashHasher<u64>>;

impl CacheTrait for DashCache {
    fn cache_get(&self, z_hash: u64) -> Option<Item> {
        self.get(&z_hash).map(|v| *v)
    }

    fn cache_insert(&mut self, z_hash: u64, item: Item) {
        self.insert(z_hash, item);
    }
    fn cache_new() -> Self {
        Self::with_capacity_and_hasher(CACHE_SIZE_ENTRIES, BuildNoHashHasher::new())
    }
}
