//! WAL writer implementation

use crate::entry::{encode_entry, WalEntry, WalHeader};
use crate::error::{Result, WalError};
use crate::WalConfig;
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

/// WAL writer with batched fsync
pub struct WalWriter<T = ()> {
    file: BufWriter<File>,
    path: PathBuf,
    position: u64,
    unflushed_bytes: usize,
    last_fsync: Instant,
    config: WalConfig,
    data_dir: PathBuf,
    _marker: PhantomData<T>,
}

impl<T: Serialize> WalWriter<T> {
    /// Create a new WAL writer
    pub fn new(data_dir: impl AsRef<Path>, config: WalConfig) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();

        // Ensure data directory exists
        fs::create_dir_all(&data_dir)
            .map_err(|e| WalError::DataDirectory(format!("Failed to create directory: {}", e)))?;

        let wal_path = data_dir.join("wal-current.log");

        // Open or create WAL file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wal_path)?;

        let metadata = file.metadata()?;
        let position = metadata.len();

        let mut writer = BufWriter::with_capacity(64 * 1024, file); // 64KB buffer

        // Write header if new file
        if position == 0 {
            let header = WalHeader::new();
            header.write(&mut writer)?;
            writer.flush()?;
            debug!("Created new WAL file: {}", wal_path.display());
        }

        let position = writer.seek(SeekFrom::End(0))?;

        Ok(Self {
            file: writer,
            path: wal_path,
            position,
            unflushed_bytes: 0,
            last_fsync: Instant::now(),
            config,
            data_dir,
            _marker: PhantomData,
        })
    }

    /// Append an operation to the WAL
    pub fn append(&mut self, op: &T) -> Result<u64>
    where
        T: Clone,
    {
        let entry = WalEntry::new(op.clone());
        let encoded = encode_entry(&entry)?;

        let position = self.position;
        self.file.write_all(&encoded)?;
        self.position += encoded.len() as u64;
        self.unflushed_bytes += encoded.len();

        Ok(position)
    }

    /// Flush pending writes to disk (fsync)
    pub fn flush(&mut self) -> Result<()> {
        if self.unflushed_bytes == 0 {
            return Ok(());
        }

        self.file.flush()?;
        self.file.get_ref().sync_data()?;

        debug!(
            "WAL fsynced at position {}, {} bytes flushed",
            self.position, self.unflushed_bytes
        );

        self.unflushed_bytes = 0;
        self.last_fsync = Instant::now();

        Ok(())
    }

    /// Check if flush is needed based on time
    pub fn should_flush(&self) -> bool {
        self.last_fsync.elapsed().as_millis() >= self.config.flush_interval_ms as u128
    }

    /// Check if rotation is needed based on file size
    pub fn should_rotate(&self) -> bool {
        self.position >= self.config.max_file_size
    }

    /// Rotate to a new WAL file
    pub fn rotate(&mut self) -> Result<PathBuf> {
        // Flush and close current file
        self.flush()?;

        let rotated_path = rotated_path_for_now(&self.data_dir);

        // Create new WAL file first (before renaming old one)
        let new_path = self.data_dir.join("wal-current-new.log");
        let new_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&new_path)?;

        let mut new_writer = BufWriter::with_capacity(64 * 1024, new_file);

        // Write header to new file
        let header = WalHeader::new();
        header.write(&mut new_writer)?;
        new_writer.flush()?;

        // Now rename old file
        fs::rename(&self.path, &rotated_path)?;

        // Rename new file to current
        fs::rename(&new_path, &self.path)?;

        info!(
            "Rotated WAL: {} -> {}",
            self.path.display(),
            rotated_path.display()
        );

        // Update writer state
        self.file = new_writer;
        self.position = WalHeader::SIZE as u64;
        self.unflushed_bytes = 0;
        self.last_fsync = Instant::now();

        Ok(rotated_path)
    }

    /// Get current file position
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get the WAL file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get unflushed bytes count
    pub fn unflushed_bytes(&self) -> usize {
        self.unflushed_bytes
    }
}

fn rotated_path_for_now(data_dir: &Path) -> PathBuf {
    let mut token = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0);

    loop {
        let path = data_dir.join(format!("wal-{}.log", token));
        if !path.exists() {
            return path;
        }
        token = token.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use tempfile::TempDir;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestOp {
        key: String,
        value: i32,
    }

    #[test]
    fn test_writer_create() {
        let temp_dir = TempDir::new().unwrap();
        let writer = WalWriter::<TestOp>::new(temp_dir.path(), WalConfig::default()).unwrap();

        assert!(writer.path().exists());
        assert_eq!(writer.position(), WalHeader::SIZE as u64);
    }

    #[test]
    fn test_writer_append() {
        let temp_dir = TempDir::new().unwrap();
        let mut writer = WalWriter::new(temp_dir.path(), WalConfig::default()).unwrap();

        let initial_pos = writer.position();
        writer
            .append(&TestOp {
                key: "test".to_string(),
                value: 42,
            })
            .unwrap();

        assert!(writer.position() > initial_pos);
        assert!(writer.unflushed_bytes() > 0);
    }

    #[test]
    fn test_writer_flush() {
        let temp_dir = TempDir::new().unwrap();
        let mut writer = WalWriter::new(temp_dir.path(), WalConfig::default()).unwrap();

        writer
            .append(&TestOp {
                key: "test".to_string(),
                value: 42,
            })
            .unwrap();

        assert!(writer.unflushed_bytes() > 0);

        writer.flush().unwrap();

        assert_eq!(writer.unflushed_bytes(), 0);
    }

    #[test]
    fn test_writer_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WalConfig {
            flush_interval_ms: 10,
            max_file_size: 100, // Very small to trigger rotation
        };

        let mut writer = WalWriter::new(temp_dir.path(), config).unwrap();

        // Write enough to trigger rotation
        for i in 0..10 {
            writer
                .append(&TestOp {
                    key: format!("key{}", i),
                    value: i,
                })
                .unwrap();

            if writer.should_rotate() {
                writer.rotate().unwrap();
            }
        }

        // Should have created rotated files
        let files: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| s.starts_with("wal-"))
                    .unwrap_or(false)
            })
            .collect();

        assert!(
            files.len() > 1,
            "Expected multiple WAL files after rotation"
        );
    }

    #[test]
    fn test_rotation_path_avoids_existing_archive_name() {
        let temp_dir = TempDir::new().unwrap();
        let first = rotated_path_for_now(temp_dir.path());
        File::create(&first).unwrap();

        let second = rotated_path_for_now(temp_dir.path());
        assert_ne!(first, second);
        assert!(!second.exists());
    }
}
