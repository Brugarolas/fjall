use super::writer::Writer;
use crate::{time::unix_timestamp, value::SeqNo};
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Metadata {
    /// Path of segment folder
    pub path: PathBuf,

    /// Segment ID
    pub id: String,

    /// Creation time as unix timestamp (in µs)
    pub created_at: u128,

    /// Number of items in the segment
    ///
    /// This may include tombstones
    pub item_count: u64,

    /// Block size (uncompressed)
    pub block_size: u32,

    /// Number of written blocks
    pub block_count: u32,

    /// Whether LZ4 is used
    ///
    /// Is always true
    pub is_compressed: bool,

    /// compressed size in bytes (on disk)
    pub file_size: u64,

    /// true size in bytes (if no compression were used)
    pub uncompressed_size: u64,

    /// Key range
    pub key_range: (Vec<u8>, Vec<u8>),

    /// Sequence number range
    pub seqnos: (SeqNo, SeqNo),

    /// Number of tombstones
    pub tombstone_count: u64,

    #[cfg(feature = "bloom")]
    pub bloom_filter_size: u64,
}

impl Metadata {
    /// Consumes a writer and its metadata to create the segment metadata
    pub fn from_writer(id: String, writer: Writer) -> crate::Result<Self> {
        #[cfg(feature = "bloom")]
        let bloom_filter_size = std::fs::File::open(writer.opts.path.join("bloom"))?
            .metadata()?
            .len();

        Ok(Self {
            id,
            path: writer.opts.path,
            block_count: writer.block_count as u32,
            block_size: writer.opts.block_size,

            // NOTE: Using seconds is not granular enough
            // But because millis already returns u128, might as well use micros :)
            created_at: unix_timestamp().as_micros(),

            file_size: writer.file_pos,
            is_compressed: true,
            item_count: writer.item_count as u64,
            key_range: (
                writer
                    .first_key
                    .expect("should have written at least 1 item"),
                writer
                    .last_key
                    .expect("should have written at least 1 item"),
            ),
            seqnos: (writer.lowest_seqno, writer.highest_seqno),
            tombstone_count: writer.tombstone_count as u64,
            uncompressed_size: writer.uncompressed_size,

            #[cfg(feature = "bloom")]
            bloom_filter_size,
        })
    }

    pub(crate) fn key_range_contains<K: AsRef<[u8]>>(&self, key: K) -> bool {
        let key = key.as_ref();
        key >= &self.key_range.0 && key <= &self.key_range.1
    }

    /// Stores segment metadata in a file
    ///
    /// Will be stored as JSON
    pub fn write_to_file(&self) -> std::io::Result<()> {
        let mut writer = OpenOptions::new()
            .truncate(true)
            .create(true)
            .write(true)
            .open(self.path.join("meta.json"))?;

        writer.write_all(
            serde_json::to_string_pretty(self)
                .expect("Failed to serialize to JSON")
                .as_bytes(),
        )?;
        writer.flush()?;
        writer.sync_all()?;

        // fsync folder
        let folder = std::fs::File::open(&self.path)?;
        folder.sync_all()?;

        Ok(())
    }

    /// Reads and parses a Segment metadata file
    pub fn from_disk<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file_content = std::fs::read_to_string(path)?;
        let item = serde_json::from_str(&file_content)?;
        Ok(item)
    }
}
