//! WAL entry types and header

use crate::error::{Result, WalError};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{Read, Write};

/// WAL file magic number: "CWAL" in ASCII
pub const WAL_MAGIC: [u8; 4] = [0x43, 0x57, 0x41, 0x4C];

/// Current WAL format version
pub const WAL_VERSION: u32 = 1;

/// WAL file header
#[derive(Debug, Clone)]
pub struct WalHeader {
    /// Magic number for file identification
    pub magic: [u8; 4],
    /// Format version
    pub version: u32,
    /// Creation timestamp (nanos since epoch)
    pub created_at: i64,
    /// Reserved for future use
    pub reserved: [u8; 16],
}

impl WalHeader {
    /// Create a new WAL header
    pub fn new() -> Self {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;

        Self {
            magic: WAL_MAGIC,
            version: WAL_VERSION,
            created_at,
            reserved: [0; 16],
        }
    }

    /// Header size in bytes
    pub const SIZE: usize = 32;

    /// Write header to a writer
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_all(&self.version.to_be_bytes())?;
        writer.write_all(&self.created_at.to_be_bytes())?;
        writer.write_all(&self.reserved)?;
        Ok(())
    }

    /// Read header from a reader
    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;

        if magic != WAL_MAGIC {
            return Err(WalError::InvalidMagic {
                expected: WAL_MAGIC.to_vec(),
                actual: magic.to_vec(),
            });
        }

        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)?;
        let version = u32::from_be_bytes(version_bytes);

        if version > WAL_VERSION {
            return Err(WalError::UnsupportedVersion(version));
        }

        let mut created_at_bytes = [0u8; 8];
        reader.read_exact(&mut created_at_bytes)?;
        let created_at = i64::from_be_bytes(created_at_bytes);

        let mut reserved = [0u8; 16];
        reader.read_exact(&mut reserved)?;

        Ok(Self {
            magic,
            version,
            created_at,
            reserved,
        })
    }
}

impl Default for WalHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// WAL entry wrapper with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry<T> {
    /// Timestamp when entry was created (nanos since epoch)
    pub timestamp: i64,
    /// The actual operation
    pub op: T,
}

impl<T> WalEntry<T> {
    /// Create a new WAL entry with current timestamp
    pub fn new(op: T) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;

        Self { timestamp, op }
    }
}

/// Encode a WAL entry to bytes with length prefix and checksum
///
/// Format: [length: u32][data: JSON bytes][checksum: u32]
///
/// Uses JSON for serialization to support complex enum types that bincode
/// doesn't handle well (like tagged enums).
pub fn encode_entry<T: Serialize>(entry: &WalEntry<T>) -> Result<Vec<u8>> {
    let data = serde_json::to_vec(entry).map_err(|e| WalError::Corrupted {
        pos: 0,
        reason: format!("JSON serialization error: {}", e),
    })?;
    let checksum = crc32fast::hash(&data);

    let mut buf = Vec::with_capacity(4 + data.len() + 4);
    buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
    buf.extend_from_slice(&data);
    buf.extend_from_slice(&checksum.to_be_bytes());

    Ok(buf)
}

/// Decode a WAL entry from bytes, verifying checksum
///
/// Returns the entry and total bytes consumed
pub fn decode_entry<T: DeserializeOwned>(buf: &[u8], pos: u64) -> Result<(WalEntry<T>, usize)> {
    if buf.len() < 8 {
        return Err(WalError::Corrupted {
            pos,
            reason: "Buffer too small for entry header".to_string(),
        });
    }

    let length = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;

    // Validate length is reasonable (< 10MB)
    if length > 10 * 1024 * 1024 {
        return Err(WalError::Corrupted {
            pos,
            reason: format!("Entry length too large: {} bytes", length),
        });
    }

    let total_size = 4 + length + 4;
    if buf.len() < total_size {
        return Err(WalError::Corrupted {
            pos,
            reason: format!(
                "Buffer too small: need {} bytes, have {}",
                total_size,
                buf.len()
            ),
        });
    }

    let data = &buf[4..4 + length];
    let expected_checksum = u32::from_be_bytes([
        buf[4 + length],
        buf[4 + length + 1],
        buf[4 + length + 2],
        buf[4 + length + 3],
    ]);

    let actual_checksum = crc32fast::hash(data);

    if actual_checksum != expected_checksum {
        return Err(WalError::ChecksumMismatch {
            pos,
            expected: expected_checksum,
            actual: actual_checksum,
        });
    }

    let entry: WalEntry<T> = serde_json::from_slice(data).map_err(|e| WalError::Corrupted {
        pos,
        reason: format!("JSON deserialization error: {}", e),
    })?;

    Ok((entry, total_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestOp {
        key: String,
        value: i32,
    }

    #[test]
    fn test_header_roundtrip() {
        let header = WalHeader::new();
        let mut buf = Vec::new();
        header.write(&mut buf).unwrap();

        assert_eq!(buf.len(), WalHeader::SIZE);

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = WalHeader::read(&mut cursor).unwrap();

        assert_eq!(decoded.magic, header.magic);
        assert_eq!(decoded.version, header.version);
        assert_eq!(decoded.created_at, header.created_at);
    }

    #[test]
    fn test_entry_roundtrip() {
        let entry = WalEntry::new(TestOp {
            key: "test".to_string(),
            value: 42,
        });

        let encoded = encode_entry(&entry).unwrap();
        let (decoded, size): (WalEntry<TestOp>, _) = decode_entry(&encoded, 0).unwrap();

        assert_eq!(size, encoded.len());
        assert_eq!(decoded.op, entry.op);
    }

    #[test]
    fn test_checksum_validation() {
        let entry = WalEntry::new(TestOp {
            key: "test".to_string(),
            value: 42,
        });

        let mut encoded = encode_entry(&entry).unwrap();

        // Corrupt data
        encoded[10] ^= 0xFF;

        let result: Result<(WalEntry<TestOp>, _)> = decode_entry(&encoded, 0);
        assert!(matches!(result, Err(WalError::ChecksumMismatch { .. })));
    }
}
