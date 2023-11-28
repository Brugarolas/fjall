use crate::{
    compaction::{tiered, CompactionStrategy},
    Tree,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
pub use tiered::Strategy as SizeTiered;

/// Tree configuration
pub struct Config {
    /// Folder path
    ///
    /// Defaults to `./.lsm.data`
    pub(crate) path: PathBuf,

    /// Block size of data and index blocks
    ///
    /// Defaults to 4 KiB (4096 bytes)
    pub(crate) block_size: u32,

    /// Block cache size in # blocks
    ///
    /// Defaults to 1,024
    pub(crate) block_cache_size: u32,

    /// [`MemTable`] maximum size in bytes
    ///
    /// Defaults to 64 MiB, like RocksDB
    pub(crate) max_memtable_size: u32,

    /// Amount of levels of the LSM tree (depth of tree)
    ///
    /// Defaults to 7, like RocksDB
    pub(crate) levels: u8,

    /// Maximum amount of concurrent flush threads
    ///
    /// You may want to increase this the more CPU cores you have
    ///
    /// Defaults to 4
    pub(crate) flush_threads: u8,

    /// Compaction strategy to use
    ///
    /// Defaults to SizeTiered
    pub(crate) compaction_strategy: Arc<dyn CompactionStrategy + Send + Sync>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: ".lsm.data".into(),
            block_size: 4_096,
            block_cache_size: 1_024,
            max_memtable_size: 128 * 1_024 * 1_024,
            levels: 7,
            compaction_strategy: Arc::new(tiered::Strategy::default()),
            flush_threads: 4,
        }
    }
}

impl Config {
    /// Initializes a new config
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().into(),
            ..Default::default()
        }
    }

    /// Maximum amount of concurrent flush threads
    ///
    /// You may want to increase this the more CPU cores you have
    ///
    /// Defaults to 4
    ///
    /// # Panics
    ///
    /// Panics if count is 0
    #[must_use]
    pub fn flush_threads(mut self, count: u8) -> Self {
        assert!(count > 0);

        self.flush_threads = count;
        self
    }

    /// Sets the amount of levels of the LSM tree (depth of tree)
    ///
    /// Defaults to 7, like `LevelDB` and `RocksDB`
    ///
    /// # Panics
    ///
    /// Panics if count is 0
    #[must_use]
    pub fn level_count(mut self, count: u8) -> Self {
        assert!(count > 0);

        self.levels = count;
        self
    }

    /// Sets the maximum memtable size
    ///
    /// Defaults to 64 MiB, like `RocksDB`
    #[must_use]
    pub fn max_memtable_size(mut self, bytes: u32) -> Self {
        self.max_memtable_size = bytes;
        self
    }

    /// Sets the block size
    ///
    /// Defaults to 4 KiB (4096 bytes)
    ///
    /// # Panics
    ///
    /// Panics if the block size is smaller than 1 KiB (1024 bytes)
    #[must_use]
    pub fn block_size(mut self, block_size: u32) -> Self {
        assert!(block_size >= 1024);

        self.block_size = block_size;
        self
    }

    /// Sets the block cache size in # blocks
    ///
    /// Defaults to 1,024
    #[must_use]
    pub fn block_cache_size(mut self, block_cache_size: u32) -> Self {
        self.block_cache_size = block_cache_size;
        self
    }

    /// Sets the compaction strategy to use
    ///
    /// Defaults to [`SizeTiered`]
    #[must_use]
    pub fn compaction_strategy(
        mut self,
        strategy: Arc<dyn CompactionStrategy + Send + Sync>,
    ) -> Self {
        self.compaction_strategy = strategy;
        self
    }

    /// Opens a tree using the config
    ///
    /// # Errors
    ///
    /// - Will return `Err` if an IO error occurs
    pub fn open(self) -> crate::Result<Tree> {
        Tree::open(self)
    }
}
