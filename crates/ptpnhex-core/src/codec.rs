//! Bounds-checked little-endian read primitives shared by binary parsers.

use crate::{Error, Result};

/// Sequential reader over a byte slice with little-endian decoding.
///
/// Every method fails with [`Error::Malformed`] instead of panicking when
/// the input is shorter than expected, carrying the name of the structure
/// being parsed for diagnostics.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    what: &'static str,
}

impl<'a> Reader<'a> {
    /// Creates a reader over `buf`; `what` names the structure being parsed
    /// (for example `"PARAM.SFO"`) and is used in error messages.
    pub fn new(buf: &'a [u8], what: &'static str) -> Self {
        Self { buf, pos: 0, what }
    }

    /// Current read position from the start of the buffer.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Moves the read position to `pos`.
    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.buf.len() {
            return Err(self.eof(pos - self.pos));
        }
        self.pos = pos;
        Ok(())
    }

    /// Reads `len` raw bytes.
    pub fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|&end| end <= self.buf.len())
            .ok_or_else(|| self.eof(len))?;
        let bytes = &self.buf[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    /// Reads a little-endian `u16`.
    pub fn u16_le(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// Reads a little-endian `u32`.
    pub fn u32_le(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a NUL-terminated string starting at absolute offset `pos`
    /// without moving the read position.
    pub fn cstr_at(&self, pos: usize) -> Result<&'a str> {
        let tail = self.buf.get(pos..).ok_or_else(|| self.eof(0))?;
        let nul = tail
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| self.malformed("unterminated string"))?;
        std::str::from_utf8(&tail[..nul]).map_err(|_| self.malformed("string is not valid UTF-8"))
    }

    /// Builds a [`Error::Malformed`] for this reader's structure.
    pub fn malformed(&self, reason: impl Into<String>) -> Error {
        Error::Malformed {
            what: self.what,
            reason: reason.into(),
        }
    }

    fn eof(&self, wanted: usize) -> Error {
        self.malformed(format!(
            "unexpected end of data at offset {:#x} (wanted {wanted} more bytes, have {})",
            self.pos,
            self.buf.len().saturating_sub(self.pos),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_integers() {
        let mut r = Reader::new(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06], "test");
        assert_eq!(r.u16_le().unwrap(), 0x0201);
        assert_eq!(r.u32_le().unwrap(), 0x0605_0403);
        assert_eq!(r.position(), 6);
    }

    #[test]
    fn fails_instead_of_panicking_on_short_input() {
        let mut r = Reader::new(&[0x01], "test");
        assert!(r.u32_le().is_err());
    }

    #[test]
    fn reads_nul_terminated_strings_at_offsets() {
        let r = Reader::new(b"ab\0TITLE\0", "test");
        assert_eq!(r.cstr_at(0).unwrap(), "ab");
        assert_eq!(r.cstr_at(3).unwrap(), "TITLE");
        assert!(r.cstr_at(99).is_err());
    }

    #[test]
    fn rejects_unterminated_strings() {
        let r = Reader::new(b"abc", "test");
        assert!(r.cstr_at(0).is_err());
    }
}
