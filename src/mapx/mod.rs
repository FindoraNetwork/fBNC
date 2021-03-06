//!
//! # A disk-storage replacement for the pure in-memory BTreeMap
//!
//! This module is non-invasive to external code except the `new` method.
//!

mod backend;
#[cfg(test)]
mod test;

use crate::serde::{CacheMeta, CacheVisitor};
use ruc::*;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    cmp::Ordering,
    fmt,
    hash::Hash,
    iter::Iterator,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

/// To solve the problem of unlimited memory usage,
/// use this to replace the original in-memory `BTreeMap<_, _>`.
#[derive(PartialEq, Debug, Clone)]
pub struct Mapx<K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    in_disk: backend::Mapx<K, V>,
}

///////////////////////////////////////////////
// Begin of the self-implementation for Mapx //
/*********************************************/

impl<K, V> Mapx<K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    /// Create an instance.
    #[inline(always)]
    pub fn new(path: &str) -> Result<Self> {
        let in_disk = backend::Mapx::load_or_create(path).c(d!())?;
        Ok(Mapx { in_disk })
    }

    /// Get the database storage path
    pub fn get_path(&self) -> &str {
        self.in_disk.get_path()
    }

    /// Imitate the behavior of 'BTreeMap<_>.get(...)'
    ///
    /// Any faster/better choice other than JSON ?
    #[inline(always)]
    pub fn get(&self, key: &K) -> Option<V> {
        self.in_disk.get(key)
    }

    /// Imitate the behavior of 'BTreeMap<_>.get_mut(...)'
    #[inline(always)]
    pub fn get_mut(&mut self, key: &K) -> Option<ValueMut<'_, K, V>> {
        self.in_disk
            .get(key)
            .map(move |v| ValueMut::new(self, key.clone(), v))
    }

    /// Imitate the behavior of 'BTreeMap<_>.len()'.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.in_disk.len()
    }

    /// A helper func
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.in_disk.is_empty()
    }

    /// Imitate the behavior of 'BTreeMap<_>.insert(...)'.
    #[inline(always)]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.in_disk.insert(key, value)
    }

    /// Similar with `insert`, but ignore the old value.
    #[inline(always)]
    pub fn set_value(&mut self, key: K, value: V) {
        self.in_disk.set_value(key, value);
    }

    /// Imitate the behavior of '.entry(...).or_insert(...)'
    #[inline(always)]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        Entry { key, db: self }
    }

    /// Imitate the behavior of '.iter()'
    #[inline(always)]
    pub fn iter(&self) -> Box<dyn Iterator<Item = (K, V)> + '_> {
        Box::new(MapxIter {
            iter: self.in_disk.iter(),
        })
    }

    /// Check if a key is exists.
    #[inline(always)]
    pub fn contains_key(&self, key: &K) -> bool {
        self.in_disk.contains_key(key)
    }

    /// Remove a <K, V> from mem and disk.
    #[inline(always)]
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.in_disk.remove(key)
    }

    /// Remove a <K, V> from mem and disk.
    #[inline(always)]
    pub fn unset_value(&mut self, key: &K) {
        self.in_disk.unset_value(key);
    }
}

/*******************************************/
// End of the self-implementation for Mapx //
/////////////////////////////////////////////

//////////////////////////////////////////////////////////////////////////////////
// Begin of the implementation of ValueMut(returned by `self.get_mut`) for Mapx //
/********************************************************************************/

/// Returned by `<Mapx>.get_mut(...)`
#[derive(Debug)]
pub struct ValueMut<'a, K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    mapx: &'a mut Mapx<K, V>,
    key: ManuallyDrop<K>,
    value: ManuallyDrop<V>,
}

impl<'a, K, V> ValueMut<'a, K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn new(mapx: &'a mut Mapx<K, V>, key: K, value: V) -> Self {
        ValueMut {
            mapx,
            key: ManuallyDrop::new(key),
            value: ManuallyDrop::new(value),
        }
    }

    /// Clone the inner value.
    pub fn clone_inner(self) -> V {
        ManuallyDrop::into_inner(self.value.clone())
    }
}

///
/// **NOTE**: VERY IMPORTANT !!!
///
impl<'a, K, V> Drop for ValueMut<'a, K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn drop(&mut self) {
        // This operation is safe within a `drop()`.
        // SEE: [**ManuallyDrop::take**](std::mem::ManuallyDrop::take)
        unsafe {
            self.mapx.set_value(
                ManuallyDrop::take(&mut self.key),
                ManuallyDrop::take(&mut self.value),
            );
        };
    }
}

impl<'a, K, V> Deref for ValueMut<'a, K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a, K, V> DerefMut for ValueMut<'a, K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<'a, K, V> PartialEq for ValueMut<'a, K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn eq(&self, other: &ValueMut<'a, K, V>) -> bool {
        self.value == other.value
    }
}

impl<'a, K, V> PartialEq<V> for ValueMut<'a, K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn eq(&self, other: &V) -> bool {
        self.value.deref() == other
    }
}

impl<'a, K, V> PartialOrd<V> for ValueMut<'a, K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Ord + PartialOrd + Serialize + DeserializeOwned + fmt::Debug,
{
    fn partial_cmp(&self, other: &V) -> Option<Ordering> {
        self.value.deref().partial_cmp(other)
    }
}

/******************************************************************************/
// End of the implementation of ValueMut(returned by `self.get_mut`) for Mapx //
////////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////
// Begin of the implementation of Entry for Mapx //
/*************************************************/

/// Imitate the `btree_map/btree_map::Entry`.
pub struct Entry<'a, K, V>
where
    K: 'a
        + fmt::Debug
        + Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned,
    V: 'a + fmt::Debug + Clone + PartialEq + Serialize + DeserializeOwned,
{
    key: K,
    db: &'a mut Mapx<K, V>,
}

impl<'a, K, V> Entry<'a, K, V>
where
    K: 'a
        + fmt::Debug
        + Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned,
    V: 'a + fmt::Debug + Clone + PartialEq + Serialize + DeserializeOwned,
{
    /// Imitate the `btree_map/btree_map::Entry.or_insert(...)`.
    pub fn or_insert(self, default: V) -> ValueMut<'a, K, V> {
        if !self.db.contains_key(&self.key) {
            self.db.set_value(self.key.clone(), default);
        }
        pnk!(self.db.get_mut(&self.key))
    }

    /// Imitate the `btree_map/btree_map::Entry.or_insert_with(...)`.
    pub fn or_insert_with<F>(self, default: F) -> ValueMut<'a, K, V>
    where
        F: FnOnce() -> V,
    {
        if !self.db.contains_key(&self.key) {
            self.db.set_value(self.key.clone(), default());
        }
        pnk!(self.db.get_mut(&self.key))
    }
}

/***********************************************/
// End of the implementation of Entry for Mapx //
/////////////////////////////////////////////////

//////////////////////////////////////////////////
// Begin of the implementation of Iter for Mapx //
/************************************************/

/// Iter over [Mapx](self::Mapx).
pub struct MapxIter<'a, K, V>
where
    K: Clone + PartialEq + Eq + Hash + Serialize + DeserializeOwned + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    iter: backend::MapxIter<'a, K, V>,
}

impl<'a, K, V> Iterator for MapxIter<'a, K, V>
where
    K: Clone + PartialEq + Eq + Hash + Serialize + DeserializeOwned + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    type Item = (K, V);
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/**********************************************/
// End of the implementation of Iter for Mapx //
////////////////////////////////////////////////

/////////////////////////////////////////////////////////
// Begin of the implementation of Eq for Mapx //
/*******************************************************/

impl<K, V> Eq for Mapx<K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
}

/*****************************************************/
// End of the implementation of Eq for Mapx //
///////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////
// Begin of the implementation of Serialize/Deserialize for Mapx //
/*****************************************************************/

impl<K, V> serde::Serialize for Mapx<K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let v = pnk!(serde_json::to_string(&CacheMeta {
            path: self.get_path(),
        }));

        serializer.serialize_str(&v)
    }
}

impl<'de, K, V> serde::Deserialize<'de> for Mapx<K, V>
where
    K: Clone
        + PartialEq
        + Eq
        + PartialOrd
        + Ord
        + Hash
        + Serialize
        + DeserializeOwned
        + fmt::Debug,
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(CacheVisitor).map(|meta| {
            let meta = pnk!(serde_json::from_str::<CacheMeta>(&meta));
            pnk!(Mapx::new(meta.path))
        })
    }
}

/***************************************************************/
// End of the implementation of Serialize/Deserialize for Mapx //
/////////////////////////////////////////////////////////////////
