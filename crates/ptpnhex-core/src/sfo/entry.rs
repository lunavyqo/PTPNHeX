//! Index-table entries of a `PARAM.SFO` file.

/// Value encoding of an SFO entry, from the index table's format field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    /// Raw bytes / UTF-8 without NUL termination (`0x0004`).
    Bytes,
    /// NUL-terminated UTF-8 string (`0x0204`).
    Utf8,
    /// Little-endian 32-bit integer (`0x0404`).
    Int32,
    /// A format this library does not know; preserved verbatim.
    Unknown(u16),
}

impl DataFormat {
    /// Decodes the raw format field.
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0x0004 => Self::Bytes,
            0x0204 => Self::Utf8,
            0x0404 => Self::Int32,
            other => Self::Unknown(other),
        }
    }

    /// Encodes back to the raw format field.
    pub fn to_raw(self) -> u16 {
        match self {
            Self::Bytes => 0x0004,
            Self::Utf8 => 0x0204,
            Self::Int32 => 0x0404,
            Self::Unknown(other) => other,
        }
    }
}

/// One key/value pair of a `PARAM.SFO` file.
///
/// Offsets are preserved from the parsed file so that serialization
/// reproduces the original layout exactly.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Entry name, for example `TITLE` or `SAVEDATA_PARAMS`.
    pub(super) key: String,
    /// Offset of the key string relative to the key table.
    pub(super) key_offset: u16,
    /// Value encoding.
    pub(super) format: DataFormat,
    /// Number of bytes of `data` currently in use (including the NUL
    /// terminator for [`DataFormat::Utf8`] values).
    pub(super) data_len: u32,
    /// Offset of the value relative to the data table.
    pub(super) data_offset: u32,
    /// Value storage, always exactly `data_max_len` bytes long; bytes past
    /// `data_len` are zero.
    pub(super) data: Vec<u8>,
}

impl Entry {
    /// Entry name.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Value encoding.
    pub fn format(&self) -> DataFormat {
        self.format
    }

    /// The used portion of the value (`data_len` bytes).
    pub fn data(&self) -> &[u8] {
        &self.data[..self.data_len as usize]
    }

    /// The full allocated value storage (`data_max_len` bytes).
    pub fn data_full(&self) -> &[u8] {
        &self.data
    }

    /// Maximum number of bytes the value may occupy.
    pub fn max_len(&self) -> u32 {
        self.data.len() as u32
    }
}
