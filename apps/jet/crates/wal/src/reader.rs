//! WAL reader implementation

use crate::entry::{decode_entry, WalEntry, WalHeader};
use crate::error::{Result, WalError};
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::Read;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use tracing::debug;

/// WAL reader for replaying entries
pub struct WalReader<T> {
    file: File,
    path: PathBuf,
    position: u64,
    file_size: u64,
    _marker: PhantomData<T>,
}

impl<T: DeserializeOwned> WalReader<T> {
    /// Open a WAL file for reading
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;

        // Read and validate header
        let _header = WalHeader::read(&mut file)?;

        let file_size = file.metadata()?.len();
        let position = WalHeader::SIZE as u64;

        debug!(
            "Opened WAL for reading: {} ({} bytes)",
            path.display(),
            file_size
        );

        Ok(Self {
            file,
            path,
            position,
            file_size,
            _marker: PhantomData,
        })
    }

    /// Read the next entry from the WAL
    pub fn read_entry(&mut self) -> Result<Option<WalEntry<T>>> {
        if self.position >= self.file_size {
            return Ok(None); // EOF
        }

        // Read enough bytes for the entry
        // First read length prefix (4 bytes)
        let mut length_bytes = [0u8; 4];
        match self.file.read_exact(&mut length_bytes) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None); // Graceful EOF
            }
            Err(e) => return Err(e.into()),
        }

        let length = u32::from_be_bytes(length_bytes) as usize;

        // Validate length is reasonable (< 10MB)
        if length > 10 * 1024 * 1024 {
            return Err(WalError::Corrupted {
                pos: self.position,
                reason: format!("Entry length too large: {} bytes", length),
            });
        }

        // Read rest of entry (data + checksum)
        let mut entry_bytes = vec![0u8; 4 + length + 4];
        entry_bytes[0..4].copy_from_slice(&length_bytes);
        self.file.read_exact(&mut entry_bytes[4..])?;

        let entry_pos = self.position;
        self.position += entry_bytes.len() as u64;

        // Decode and verify checksum
        let (entry, _) = decode_entry(&entry_bytes, entry_pos)?;

        Ok(Some(entry))
    }

    /// Read all entries from the WAL
    pub fn read_all(&mut self) -> Result<Vec<WalEntry<T>>> {
        let mut entries = Vec::new();
        while let Some(entry) = self.read_entry()? {
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Get current read position
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get the WAL file path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get file size
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Check if there are more entries to read
    pub fn has_more(&self) -> bool {
        self.position < self.file_size
    }
}

/// Iterator adapter for WalReader
pub struct WalIterator<T> {
    reader: WalReader<T>,
}

impl<T: DeserializeOwned> Iterator for WalIterator<T> {
    type Item = Result<WalEntry<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.read_entry() {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

impl<T: DeserializeOwned> IntoIterator for WalReader<T> {
    type Item = Result<WalEntry<T>>;
    type IntoIter = WalIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        WalIterator { reader: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WalConfig;
    use crate::WalWriter;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestOp {
        key: String,
        value: i32,
    }

    #[test]
    fn test_reader_empty_wal() {
        let temp_dir = TempDir::new().unwrap();

        // Create WAL with no entries
        let writer = WalWriter::<TestOp>::new(temp_dir.path(), WalConfig::default()).unwrap();
        drop(writer);

        let mut reader = WalReader::<TestOp>::new(temp_dir.path().join("wal-current.log")).unwrap();

        assert!(reader.read_entry().unwrap().is_none());
    }

    #[test]
    fn test_reader_with_entries() {
        let temp_dir = TempDir::new().unwrap();

        let ops = vec![
            TestOp {
                key: "a".to_string(),
                value: 1,
            },
            TestOp {
                key: "b".to_string(),
                value: 2,
            },
            TestOp {
                key: "c".to_string(),
                value: 3,
            },
        ];

        // Write entries
        let mut writer = WalWriter::new(temp_dir.path(), WalConfig::default()).unwrap();
        for op in &ops {
            writer.append(op).unwrap();
        }
        writer.flush().unwrap();
        drop(writer);

        // Read entries
        let mut reader = WalReader::<TestOp>::new(temp_dir.path().join("wal-current.log")).unwrap();

        let read_entries = reader.read_all().unwrap();

        assert_eq!(read_entries.len(), ops.len());
        for (entry, expected) in read_entries.iter().zip(ops.iter()) {
            assert_eq!(&entry.op, expected);
        }
    }

    #[test]
    fn test_reader_iterator() {
        let temp_dir = TempDir::new().unwrap();

        // Write some entries
        let mut writer = WalWriter::new(temp_dir.path(), WalConfig::default()).unwrap();
        for i in 0..5 {
            writer
                .append(&TestOp {
                    key: format!("key{}", i),
                    value: i,
                })
                .unwrap();
        }
        writer.flush().unwrap();
        drop(writer);

        // Use iterator
        let reader = WalReader::<TestOp>::new(temp_dir.path().join("wal-current.log")).unwrap();

        let entries: Vec<_> = reader.into_iter().collect();
        assert_eq!(entries.len(), 5);

        for (i, result) in entries.into_iter().enumerate() {
            let entry = result.unwrap();
            assert_eq!(entry.op.value, i as i32);
        }
    }
}
