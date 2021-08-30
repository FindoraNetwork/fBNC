//!
//! # Common Types and Macros
//!

use lazy_static::lazy_static;
use rocksdb::{DBCompressionType, Options, DB};
use ruc::*;
use serde::{de::DeserializeOwned, Serialize};
use std::{borrow::Cow, cmp::Ordering, convert::TryInto, env, fmt, fs, mem, ops::Deref};

lazy_static! {
    /// The path to store the cache data.
    /// Is it necessary to be compatible with the Windows operating system?
    pub static ref CACHE_DIR: String = env::var("LEDGER_DIR").unwrap_or_else(|_|"/tmp".to_owned());
}

/// Try once more when we fail to open a db.
#[macro_export]
macro_rules! try_twice {
    ($ops: expr) => {
        ruc::pnk!($ops.c(d!()).or_else(|e| {
            e.print();
            $ops.c(d!())
        }))
    };
}

/// Generate a unique path for each instance.
#[macro_export]
macro_rules! unique_path {
    () => {
        format!(
            "{}/.bnc/{}/{}_{}_{}_{}",
            *$crate::helper::CACHE_DIR,
            ts!(),
            file!(),
            line!(),
            column!(),
            rand::random::<u32>()
        )
    };
}

/// A helper for creating Vecx.
#[macro_export]
macro_rules! new_vecx {
    (@$ty: ty, $in_mem_cnt: expr) => {
        $crate::new_vecx_custom!($ty, $in_mem_cnt)
    };
    (@$ty: ty) => {
        $crate::new_vecx_custom!($ty)
    };
    ($path:expr) => {
        $crate::new_vecx_custom!($path)
    };
    ($path:expr, $in_mem_cnt: expr) => {
        $crate::new_vecx_custom!($path, $in_mem_cnt)
    };
    () => {
        $crate::new_vecx_custom!()
    };
}

/// A helper for creating Vecx.
#[macro_export]
macro_rules! new_vecx_custom {
    (@$ty: ty, $in_mem_cnt: expr) => {{
        let obj: $crate::Vecx<$ty> = $crate::try_twice!($crate::Vecx::new(
            &$crate::unique_path!(),
            Some($in_mem_cnt),
        ));
        obj
    }};
    (@$ty: ty) => {{
        let obj: $crate::Vecx<$ty> =
            $crate::try_twice!($crate::Vecx::new(&$crate::unique_path!(), None,));
        obj
    }};
    ($path: expr, $in_mem_cnt: expr) => {
        $crate::try_twice!($crate::Vecx::new($path, Some($in_mem_cnt)))
    };
    ($path: expr) => {
        $crate::try_twice!($crate::Vecx::new($path, None))
    };
    (%$in_mem_cnt: expr) => {
        $crate::try_twice!($crate::Vecx::new(
            &$crate::unique_path!(),
            Some($in_mem_cnt),
        ))
    };
    () => {
        $crate::try_twice!($crate::Vecx::new(&$crate::unique_path!(), None))
    };
}

/// A helper for creating Mapx.
#[macro_export]
macro_rules! new_mapx {
    (@$ty: ty, $in_mem_cnt: expr) => {
        $crate::new_mapx_custom!($ty, $in_mem_cnt)
    };
    (@$ty: ty) => {
        $crate::new_mapx_custom!($ty)
    };
    ($path:expr, $in_mem_cnt: expr) => {
        $crate::new_mapx_custom!($path, $in_mem_cnt)
    };
    ($path:expr) => {
        $crate::new_mapx_custom!($path)
    };
    () => {
        $crate::new_mapx_custom!()
    };
}

/// A helper for creating Mapx.
#[macro_export]
macro_rules! new_mapx_custom {
    (@$ty: ty, $in_mem_cnt: expr) => {{
        let obj: $crate::Mapx<$ty> =
            $crate::try_twice!($crate::Mapx::new(&$crate::unique_path!(), $in_mem_cnt));
        obj
    }};
    (@$ty: ty) => {{
        let obj: $crate::Mapx<$ty> =
            $crate::try_twice!($crate::Mapx::new(&$crate::unique_path!(), None));
        obj
    }};
    ($path: expr, $in_mem_cnt: expr) => {
        $crate::try_twice!($crate::Mapx::new(&*$path, $in_mem_cnt))
    };
    ($path: expr) => {
        $crate::try_twice!($crate::Mapx::new(&*$path, None))
    };
    (&$in_mem_cnt: expr) => {
        $crate::try_twice!($crate::Mapx::new(&$crate::unique_path!(), $in_mem_cnt))
    };
    () => {
        $crate::try_twice!($crate::Mapx::new(&$crate::unique_path!(), None))
    };
}

////////////////////////////////////////////////////////////////////////////////
// Begin of the implementation of Value(returned by `self.get`) for Vecx/Mapx //
/******************************************************************************/

/// Returned by `.get(...)`
#[derive(Debug, Clone)]
pub struct Value<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    value: Cow<'a, V>,
}

impl<'a, V> Value<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    pub(crate) fn new(value: Cow<'a, V>) -> Self {
        Value { value }
    }

    /// Comsume the ownship and get the inner value.
    pub fn into_inner(self) -> Cow<'a, V> {
        self.value
    }
}

impl<'a, V> Deref for Value<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a, V> PartialEq for Value<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn eq(&self, other: &Value<'a, V>) -> bool {
        self.value == other.value
    }
}

impl<'a, V> PartialEq<V> for Value<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn eq(&self, other: &V) -> bool {
        self.value.as_ref() == other
    }
}

impl<'a, V> PartialOrd<V> for Value<'a, V>
where
    V: fmt::Debug + Clone + PartialEq + Ord + PartialOrd + Serialize + DeserializeOwned,
{
    fn partial_cmp(&self, other: &V) -> Option<Ordering> {
        self.value.as_ref().partial_cmp(other)
    }
}

impl<'a, V> From<V> for Value<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn from(v: V) -> Self {
        Value::new(Cow::Owned(v))
    }
}

impl<'a, V> From<Cow<'a, V>> for Value<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn from(v: Cow<'a, V>) -> Self {
        Value::new(v)
    }
}

impl<'a, V> From<Value<'a, V>> for Cow<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn from(v: Value<'a, V>) -> Self {
        v.into_inner()
    }
}

impl<'a, V> From<&V> for Value<'a, V>
where
    V: Clone + PartialEq + Serialize + DeserializeOwned + fmt::Debug,
{
    fn from(v: &V) -> Self {
        Value::new(Cow::Owned(v.clone()))
    }
}

/****************************************************************************/
// End of the implementation of Value(returned by `self.get`) for Vecx/Mapx //
//////////////////////////////////////////////////////////////////////////////

#[inline(always)]
pub(crate) fn rocksdb_open(path: &str) -> Result<DB> {
    let mut cfg = Options::default();
    cfg.create_if_missing(true);
    cfg.set_compression_type(DBCompressionType::Lz4);
    cfg.set_max_open_files(4096);

    DB::open(&cfg, path).c(d!())
}

#[inline(always)]
pub(crate) fn read_db_len(path: &str) -> Result<usize> {
    fs::read(path).c(d!(path.to_owned())).map(|bytes| {
        usize::from_le_bytes(bytes[..mem::size_of::<usize>()].try_into().unwrap())
    })
}

#[inline(always)]
pub(crate) fn write_db_len(path: &str, len: usize) -> Result<()> {
    fs::write(path, usize::to_le_bytes(len)).c(d!(path.to_owned()))
}
