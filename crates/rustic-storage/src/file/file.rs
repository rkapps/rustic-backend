use crate::file::errors::RecordHeaderError;
use anyhow::Result;
use bson::serialize_to_vec;
use crc32fast::Hasher;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};

/// Binary size of a [`RecordHeader`] in bytes.  All fields are little-endian.
pub(super) const HEADER_SIZE: u64 = 32;
/// Sentinel value that must appear at the start of every record; used to
/// detect misaligned reads and foreign files.
pub(super) const MAGIC: u32 = 0xDEADBEEF;
pub(super) const CURRENT_VERSION: u8 = 1;

/// Record is live and should be included in query results.
pub(super) const RECORD_TYPE_ACTIVE: u8 = 0x01;
/// Tombstone — the record with this id has been logically deleted.
pub(super) const RECORD_TYPE_DELETED: u8 = 0x02;

/// Flag indicating the BSON payload contains an embedded float vector.
pub(super) const FLAG_HAS_VECTOR: u16 = 0x0010;

/// Fixed-size binary header prepended to every record in a collection file.
///
/// Layout (32 bytes, little-endian):
///
/// | Offset | Size | Field         |
/// |--------|------|---------------|
/// | 0      | 4    | `magic`       |
/// | 4      | 1    | `version`     |
/// | 5      | 1    | `record_type` |
/// | 6      | 2    | `flags`       |
/// | 8      | 8    | `length`      |
/// | 16     | 8    | `timestamp`   |
/// | 24     | 4    | `crc32`       |
/// | 28     | 4    | `reserved`    |
///
/// `length` is the total record size including this header, so the BSON
/// payload size is `length - HEADER_SIZE`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(super) struct RecordHeader {
    pub(super) magic: u32,
    pub(super) version: u8,
    pub(super) record_type: u8,
    pub(super) flags: u16,
    /// Total record length in bytes (header + payload).
    pub(super) length: u64,
    /// Write timestamp in microseconds since the Unix epoch.
    pub(super) timestamp: u64,
    /// CRC32 of the BSON payload; checked on every read.
    pub(super) crc32: u32,
    pub(super) reserved: u32,
}

impl RecordHeader {
    fn new(record_type: u8, data_length: u64) -> Self {
        Self {
            magic: MAGIC,
            version: CURRENT_VERSION,
            record_type,
            flags: 0,
            length: HEADER_SIZE + data_length,
            timestamp: current_timestamp_micros(),
            crc32: 0, // Set after computing data CRC
            reserved: 0,
        }
    }

    pub(super) fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; HEADER_SIZE as usize];
        reader.read_exact(&mut buf)?;

        let header = Self {
            magic: u32::from_le_bytes(buf[0..4].try_into()?),
            version: buf[4],
            record_type: buf[5],
            flags: u16::from_le_bytes(buf[6..8].try_into()?),
            length: u64::from_le_bytes(buf[8..16].try_into()?),
            timestamp: u64::from_le_bytes(buf[16..24].try_into()?),
            crc32: u32::from_le_bytes(buf[24..28].try_into()?),
            reserved: u32::from_le_bytes(buf[28..32].try_into()?),
        };

        // Validate magic number
        if header.magic != MAGIC {
            return Err(anyhow::anyhow!(RecordHeaderError::InvalidMagic {
                magic: header.magic
            }));
        }

        // Check version
        if header.version > CURRENT_VERSION {
            return Err(anyhow::anyhow!(RecordHeaderError::UnsupportedVersion {
                version: header.version
            }));
        }

        Ok(header)
    }

    pub(super) fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.magic.to_le_bytes())?;
        writer.write_all(&[self.version])?;
        writer.write_all(&[self.record_type])?;
        writer.write_all(&self.flags.to_le_bytes())?;
        writer.write_all(&self.length.to_le_bytes())?;
        writer.write_all(&self.timestamp.to_le_bytes())?;
        writer.write_all(&self.crc32.to_le_bytes())?;
        writer.write_all(&self.reserved.to_le_bytes())?;
        Ok(())
    }

    // fn has_flag(&self, flag: u16) -> bool {
    //     self.flags & flag != 0
    // }

    fn set_flag(&mut self, flag: u16) {
        self.flags |= flag;
    }

    fn data_size(&self) -> u64 {
        self.length - HEADER_SIZE
    }
}

fn current_timestamp_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn compute_crc32(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

/// Append a record to `file` and return the byte offset at which it was written.
///
/// The payload is serialised as BSON, a CRC32 is computed over the bytes, and
/// then a [`RecordHeader`] followed by the payload is written atomically via a
/// single `seek + write` to the end of the file.
///
/// `record_type` must be one of [`RECORD_TYPE_ACTIVE`] or [`RECORD_TYPE_DELETED`].
/// Pass `has_vector = true` to set [`FLAG_HAS_VECTOR`] in the header flags.
pub(super) fn write_active_record<T: Serialize>(
    file: &mut File,
    record_type: u8,
    data: &T,
    has_vector: bool,
) -> Result<u64> {
    let bson_bytes = serialize_to_vec(&data)?;
    // Compute CRC
    let crc = compute_crc32(&bson_bytes);

    // Create header
    let mut header = RecordHeader::new(record_type, bson_bytes.len() as u64);
    header.crc32 = crc;

    if has_vector {
        header.set_flag(FLAG_HAS_VECTOR);
    }

    let offset = file.seek(SeekFrom::End(0))?;
    header.write(file)?;
    file.write_all(&bson_bytes)?;

    Ok(offset)
}

/// Read and deserialise the record at `offset` in `file`.
///
/// Seeks to `offset`, reads the [`RecordHeader`], verifies the magic and
/// version, reads the BSON payload, checks the CRC32, and finally
/// deserialises the payload into `T`.
///
/// Returns an error on any I/O failure, magic/version mismatch, or CRC
/// mismatch — the caller (typically [`super::repository::FileRepository::initialize`])
/// treats this as end-of-log and stops replaying.
pub(super) fn read_record<T: DeserializeOwned>(
    file: &mut File,
    offset: u64,
) -> Result<(RecordHeader, T)> {
    file.seek(SeekFrom::Start(offset))?;
    let header = RecordHeader::read(file)?;

    // Read data
    let data_size = header.data_size();
    let mut data = vec![0u8; data_size as usize];
    file.read_exact(&mut data)?;

    // Verify CRC
    let computed_crc = compute_crc32(&data);
    if computed_crc != header.crc32 {
        return Err(anyhow::anyhow!(RecordHeaderError::CorruptedData {
            offset,
            expected: header.crc32,
            actual: computed_crc,
        }));
    }

    // Deserialize
    let model: T = bson::deserialize_from_slice(&data)?;

    Ok((header, model))
}
