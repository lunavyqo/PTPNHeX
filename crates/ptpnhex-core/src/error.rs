//! Error type shared across the core library.

/// Errors produced while reading, decrypting, editing, or writing save data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The input file or buffer does not match the expected format.
    #[error("malformed {what}: {reason}")]
    Malformed {
        /// Short name of the structure being parsed (for example `PARAM.SFO`).
        what: &'static str,
        /// Human-readable explanation of the mismatch.
        reason: String,
    },

    /// The save uses a region or feature this build cannot handle (for
    /// example an unrecognized serial, or a region with no available key).
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// An underlying I/O operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
