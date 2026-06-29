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

const CACHE_SIZE_MB: usize = 2_usize.pow(3);
const CACHE_SIZE_ENTRIES: usize = (CACHE_SIZE_MB * 1024 * 1024) / size_of::<CacheBucket>();
const CACHE_SIZE_ENTRIES_ITEM: usize = (CACHE_SIZE_MB * 1024 * 1024) / size_of::<Item>();
const _: () = assert!(size_of::<CacheEntry>() == 16);
// is pow of 2
const _: () = assert!((size_of::<CacheBucket>() & (size_of::<CacheBucket>() - 1)) == 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheBound {
    Exact = 0,
    Lower = 1,
    Upper = 2,
}

type Item = (i32, u16, CacheBound);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CacheEntry {
    score: i32,
    depth: u16,
    bound: CacheBound,
    key: u64,
}

impl CacheEntry {
    pub fn is_empty(&self) -> bool {
        self.key == 0
    }

    pub fn to_item(&self) -> Item {
        (self.score, self.depth, self.bound)
    }

    pub fn from_item(item: Item, key: u64) -> Self {
        Self {
            score: item.0,
            depth: item.1,
            bound: item.2,
            key,
        }
    }
}

impl Default for CacheEntry {
    fn default() -> Self {
        Self {
            score: 0,
            depth: 0,
            key: 0,
            bound: CacheBound::Exact,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CacheBucket {
    entries: [CacheEntry; 8],
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
        let new_entry = CacheEntry::from_item(item, z_hash);

        // find first empty
        let idx = bucket
            .entries
            .iter()
            .enumerate()
            .find_map(|(i, e)| e.is_empty().then_some(i));

        if let Some(idx) = idx {
            bucket.entries[idx] = new_entry;
        } else {
            bucket.entries[0] = new_entry;
            bucket.entries.rotate_right(1);
        }
    }

    pub fn get(&self, z_hash: u64) -> Option<Item> {
        let bucket = &self.entries[Self::get_index(z_hash)];

        bucket
            .entries
            .iter()
            .find(|e| e.key == z_hash)
            .map(|i| i.to_item())
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
        Self::with_capacity_and_hasher(CACHE_SIZE_ENTRIES_ITEM, BuildNoHashHasher::new())
    }
}
