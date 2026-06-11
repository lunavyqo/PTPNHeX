//! `PARAM.SFO` (PSF) parsing and serialization.
//!
//! `PARAM.SFO` is the metadata file of every PSP save directory: a small
//! key/value store holding the game title, the save's display texts, and
//! the `SAVEDATA_PARAMS` block that drives save-data encryption.
//!
//! The parser keeps all layout information (table offsets, per-entry
//! offsets, allocation sizes), so [`ParamSfo::to_bytes`] reproduces an
//! unmodified file byte for byte. Format notes live in `docs/sfo.md`.

mod entry;

pub use entry::{DataFormat, Entry};

use crate::codec::Reader;
use crate::{Error, Result};

/// Magic bytes at the start of every PSF file: `"\0PSF"`.
const MAGIC: [u8; 4] = [0x00, b'P', b'S', b'F'];
/// Size of the fixed header in bytes.
const HEADER_LEN: usize = 0x14;
/// Size of one index-table entry in bytes.
const INDEX_ENTRY_LEN: usize = 0x10;
/// Structure name used in error messages.
const WHAT: &str = "PARAM.SFO";

/// A parsed `PARAM.SFO` file.
#[derive(Debug, Clone)]
pub struct ParamSfo {
    version: u32,
    key_table_start: u32,
    data_table_start: u32,
    entries: Vec<Entry>,
}

impl ParamSfo {
    /// Parses a `PARAM.SFO` file.
    pub fn parse(raw: &[u8]) -> Result<Self> {
        let mut r = Reader::new(raw, WHAT);
        if r.take(4)? != MAGIC {
            return Err(r.malformed("bad magic, not a PSF file"));
        }
        let version = r.u32_le()?;
        let key_table_start = r.u32_le()?;
        let data_table_start = r.u32_le()?;
        let entry_count = r.u32_le()?;

        let mut entries = Vec::with_capacity(entry_count as usize);
        for i in 0..entry_count {
            r.seek(HEADER_LEN + INDEX_ENTRY_LEN * i as usize)?;
            let key_offset = r.u16_le()?;
            let format = DataFormat::from_raw(r.u16_le()?);
            let data_len = r.u32_le()?;
            let data_max_len = r.u32_le()?;
            let data_offset = r.u32_le()?;

            if data_len > data_max_len {
                return Err(r.malformed(format!(
                    "entry {i}: data_len {data_len:#x} exceeds max_len {data_max_len:#x}"
                )));
            }
            let key = r
                .cstr_at(key_table_start as usize + key_offset as usize)?
                .to_owned();

            r.seek(data_table_start as usize + data_offset as usize)?;
            let data = r.take(data_max_len as usize)?.to_vec();

            entries.push(Entry {
                key,
                key_offset,
                format,
                data_len,
                data_offset,
                data,
            });
        }

        Ok(Self {
            version,
            key_table_start,
            data_table_start,
            entries,
        })
    }

    /// Serializes back to the on-disk representation.
    ///
    /// For a file that was parsed and not modified, the output is
    /// byte-identical to the input.
    pub fn to_bytes(&self) -> Vec<u8> {
        let data_end = self
            .entries
            .iter()
            .map(|e| e.data_offset as usize + e.data.len())
            .max()
            .unwrap_or(0);
        let mut out = vec![0u8; self.data_table_start as usize + data_end];

        out[..4].copy_from_slice(&MAGIC);
        out[0x04..0x08].copy_from_slice(&self.version.to_le_bytes());
        out[0x08..0x0C].copy_from_slice(&self.key_table_start.to_le_bytes());
        out[0x0C..0x10].copy_from_slice(&self.data_table_start.to_le_bytes());
        out[0x10..0x14].copy_from_slice(&(self.entries.len() as u32).to_le_bytes());

        for (i, e) in self.entries.iter().enumerate() {
            let at = HEADER_LEN + INDEX_ENTRY_LEN * i;
            out[at..at + 2].copy_from_slice(&e.key_offset.to_le_bytes());
            out[at + 2..at + 4].copy_from_slice(&e.format.to_raw().to_le_bytes());
            out[at + 4..at + 8].copy_from_slice(&e.data_len.to_le_bytes());
            out[at + 8..at + 12].copy_from_slice(&(e.data.len() as u32).to_le_bytes());
            out[at + 12..at + 16].copy_from_slice(&e.data_offset.to_le_bytes());

            let key_at = self.key_table_start as usize + e.key_offset as usize;
            out[key_at..key_at + e.key.len()].copy_from_slice(e.key.as_bytes());
            // The NUL terminator is already present: the buffer is zeroed.

            let data_at = self.data_table_start as usize + e.data_offset as usize;
            out[data_at..data_at + e.data.len()].copy_from_slice(&e.data);
        }
        out
    }

    /// PSF format version (`0x0101` for version 1.1).
    pub fn version(&self) -> u32 {
        self.version
    }

    /// All entries in index-table order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Looks up an entry by key.
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// Returns a string value ([`DataFormat::Utf8`] entries only).
    pub fn get_str(&self, key: &str) -> Option<&str> {
        let e = self.get(key)?;
        if e.format != DataFormat::Utf8 {
            return None;
        }
        let bytes = e.data().strip_suffix(&[0]).unwrap_or(e.data());
        std::str::from_utf8(bytes).ok()
    }

    /// Returns an integer value ([`DataFormat::Int32`] entries only).
    pub fn get_int(&self, key: &str) -> Option<u32> {
        let e = self.get(key)?;
        if e.format != DataFormat::Int32 {
            return None;
        }
        let b: [u8; 4] = e.data().try_into().ok()?;
        Some(u32::from_le_bytes(b))
    }

    /// Sets a string value on an existing [`DataFormat::Utf8`] entry.
    ///
    /// The encoded value (including its NUL terminator) must fit in the
    /// entry's allocated storage; the remainder is zero-filled.
    pub fn set_str(&mut self, key: &str, value: &str) -> Result<()> {
        let e = self.get_entry_mut(key, DataFormat::Utf8)?;
        let needed = value.len() + 1;
        if needed > e.data.len() {
            return Err(Error::Malformed {
                what: WHAT,
                reason: format!(
                    "value for {key} needs {needed} bytes but only {} are allocated",
                    e.data.len()
                ),
            });
        }
        e.data.fill(0);
        e.data[..value.len()].copy_from_slice(value.as_bytes());
        e.data_len = needed as u32;
        Ok(())
    }

    /// Sets an integer value on an existing [`DataFormat::Int32`] entry.
    pub fn set_int(&mut self, key: &str, value: u32) -> Result<()> {
        let e = self.get_entry_mut(key, DataFormat::Int32)?;
        e.data.fill(0);
        e.data[..4].copy_from_slice(&value.to_le_bytes());
        e.data_len = 4;
        Ok(())
    }

    fn get_entry_mut(&mut self, key: &str, format: DataFormat) -> Result<&mut Entry> {
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.key == key)
            .ok_or(Error::Malformed {
                what: WHAT,
                reason: format!("no such entry: {key}"),
            })?;
        if e.format != format {
            return Err(Error::Malformed {
                what: WHAT,
                reason: format!("entry {key} has format {:?}, expected {format:?}", e.format),
            });
        }
        if (e.data.len() < 4) && format == DataFormat::Int32 {
            return Err(Error::Malformed {
                what: WHAT,
                reason: format!("entry {key} is too small for an int32 value"),
            });
        }
        Ok(e)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Builds a minimal synthetic SFO with a NUL-terminated string entry
    /// (`TITLE`), an int32 entry (`PARENTAL_LEVEL`), and a raw-bytes entry
    /// (`PARAMS`), mirroring the layout rules of real files.
    fn synthetic_sfo() -> Vec<u8> {
        let keys: &[&[u8]] = &[b"PARAMS\0", b"PARENTAL_LEVEL\0", b"TITLE\0"];
        let key_table_start = HEADER_LEN + 3 * INDEX_ENTRY_LEN;
        let key_table_len: usize = keys.iter().map(|k| k.len()).sum();
        // Real files pad the key table with zeros to 4-byte alignment.
        let data_table_start = (key_table_start + key_table_len).next_multiple_of(4);

        // (key_offset, format, data_len, max_len, data_offset)
        let index: [(u16, u16, u32, u32, u32); 3] = [
            (0, 0x0004, 8, 8, 0),
            (7, 0x0404, 4, 4, 8),
            (22, 0x0204, 4, 16, 12),
        ];

        let mut out = vec![0u8; data_table_start + 28];
        out[..4].copy_from_slice(&MAGIC);
        out[0x04..0x08].copy_from_slice(&0x0101u32.to_le_bytes());
        out[0x08..0x0C].copy_from_slice(&(key_table_start as u32).to_le_bytes());
        out[0x0C..0x10].copy_from_slice(&(data_table_start as u32).to_le_bytes());
        out[0x10..0x14].copy_from_slice(&3u32.to_le_bytes());
        for (i, (ko, fmt, dl, ml, doff)) in index.iter().enumerate() {
            let at = HEADER_LEN + INDEX_ENTRY_LEN * i;
            out[at..at + 2].copy_from_slice(&ko.to_le_bytes());
            out[at + 2..at + 4].copy_from_slice(&fmt.to_le_bytes());
            out[at + 4..at + 8].copy_from_slice(&dl.to_le_bytes());
            out[at + 8..at + 12].copy_from_slice(&ml.to_le_bytes());
            out[at + 12..at + 16].copy_from_slice(&doff.to_le_bytes());
        }
        let mut at = key_table_start;
        for k in keys {
            out[at..at + k.len()].copy_from_slice(k);
            at += k.len();
        }
        out[data_table_start..data_table_start + 8]
            .copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]);
        out[data_table_start + 8..data_table_start + 12].copy_from_slice(&5u32.to_le_bytes());
        out[data_table_start + 12..data_table_start + 16].copy_from_slice(b"PON\0");
        out
    }

    #[test]
    fn parses_synthetic_file() {
        let sfo = ParamSfo::parse(&synthetic_sfo()).unwrap();
        assert_eq!(sfo.version(), 0x0101);
        assert_eq!(sfo.entries().len(), 3);
        assert_eq!(sfo.get_str("TITLE"), Some("PON"));
        assert_eq!(sfo.get_int("PARENTAL_LEVEL"), Some(5));
        assert_eq!(
            sfo.get("PARAMS").unwrap().data(),
            &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]
        );
    }

    #[test]
    fn roundtrip_is_byte_identical() {
        let raw = synthetic_sfo();
        let sfo = ParamSfo::parse(&raw).unwrap();
        assert_eq!(sfo.to_bytes(), raw);
    }

    #[test]
    fn typed_getters_check_the_format() {
        let sfo = ParamSfo::parse(&synthetic_sfo()).unwrap();
        assert_eq!(sfo.get_str("PARENTAL_LEVEL"), None);
        assert_eq!(sfo.get_int("TITLE"), None);
        assert_eq!(sfo.get_str("MISSING"), None);
    }

    #[test]
    fn set_str_updates_value_and_survives_roundtrip() {
        let mut sfo = ParamSfo::parse(&synthetic_sfo()).unwrap();
        sfo.set_str("TITLE", "PATAPON").unwrap();
        let reparsed = ParamSfo::parse(&sfo.to_bytes()).unwrap();
        assert_eq!(reparsed.get_str("TITLE"), Some("PATAPON"));
        // Unrelated entries are untouched.
        assert_eq!(reparsed.get_int("PARENTAL_LEVEL"), Some(5));
    }

    #[test]
    fn set_str_zero_fills_the_remainder() {
        let mut sfo = ParamSfo::parse(&synthetic_sfo()).unwrap();
        sfo.set_str("TITLE", "LONGER TITLE").unwrap();
        sfo.set_str("TITLE", "X").unwrap();
        let e = sfo.get("TITLE").unwrap();
        assert_eq!(e.data(), b"X\0");
        assert!(e.data_full()[2..].iter().all(|&b| b == 0));
    }

    #[test]
    fn set_str_rejects_values_exceeding_allocation() {
        let mut sfo = ParamSfo::parse(&synthetic_sfo()).unwrap();
        // TITLE has 16 bytes allocated; 16 chars + NUL do not fit.
        assert!(sfo.set_str("TITLE", "ABCDEFGHIJKLMNOP").is_err());
        assert_eq!(sfo.get_str("TITLE"), Some("PON"));
    }

    #[test]
    fn set_int_roundtrips() {
        let mut sfo = ParamSfo::parse(&synthetic_sfo()).unwrap();
        sfo.set_int("PARENTAL_LEVEL", 11).unwrap();
        let reparsed = ParamSfo::parse(&sfo.to_bytes()).unwrap();
        assert_eq!(reparsed.get_int("PARENTAL_LEVEL"), Some(11));
    }

    #[test]
    fn rejects_bad_magic() {
        let mut raw = synthetic_sfo();
        raw[1] = b'X';
        assert!(ParamSfo::parse(&raw).is_err());
    }

    #[test]
    fn rejects_truncated_input() {
        let raw = synthetic_sfo();
        for len in [0, 3, HEADER_LEN - 1, HEADER_LEN + 5] {
            assert!(ParamSfo::parse(&raw[..len]).is_err(), "len {len}");
        }
    }

    #[test]
    fn rejects_data_len_beyond_allocation() {
        let mut raw = synthetic_sfo();
        // Corrupt TITLE's data_len (third index entry) to exceed max_len.
        let at = HEADER_LEN + INDEX_ENTRY_LEN * 2 + 4;
        raw[at..at + 4].copy_from_slice(&999u32.to_le_bytes());
        assert!(ParamSfo::parse(&raw).is_err());
    }
}
