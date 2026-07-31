use std::fmt;

/// Errors that can occur while parsing a demo file.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid demo file: magic bytes mismatch (expected PBDEMS2\\0, got {got:?})")]
    InvalidMagic { got: [u8; 8] },
    #[error("unexpected end of data: needed {needed} bits, have {available}")]
    Overflow { needed: usize, available: usize },
    #[error("decompression error: {0}")]
    Decompress(String),
    #[error("{resource} exceeds decode limit: {actual} > {limit}")]
    LimitExceeded {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    #[error("could not reserve {requested} bytes for {resource}")]
    Allocation {
        resource: &'static str,
        requested: usize,
    },
    #[error("command at byte {offset} (command {command:?}, tick {tick:?}): {source}")]
    Command {
        offset: usize,
        command: Option<i32>,
        tick: Option<i32>,
        #[source]
        source: Box<Error>,
    },
    #[error("parse error: {context}")]
    Parse { context: String },
}

// Manual impl because `snap::Error` doesn't implement `std::error::Error`,
// so thiserror's `#[from]` derive can't be used.
impl From<snap::Error> for Error {
    fn from(e: snap::Error) -> Self {
        Error::Decompress(e.to_string())
    }
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Lightweight error type for field value conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldValueConversionError;

impl fmt::Display for FieldValueConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "incompatible types or out of range integer conversion attempted"
        )
    }
}

impl std::error::Error for FieldValueConversionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_display() {
        let err = Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        let msg = format!("{err}");
        assert!(msg.contains("IO error"));
    }

    #[test]
    fn invalid_magic_display() {
        let err = Error::InvalidMagic { got: [0; 8] };
        let msg = format!("{err}");
        assert!(msg.contains("magic bytes"));
    }

    #[test]
    fn overflow_display() {
        let err = Error::Overflow {
            needed: 64,
            available: 8,
        };
        let msg = format!("{err}");
        assert!(msg.contains("64"));
        assert!(msg.contains("8"));
    }

    #[test]
    fn parse_error_display() {
        let err = Error::Parse {
            context: "bad data".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("bad data"));
    }

    #[test]
    fn field_value_conversion_error() {
        let err = FieldValueConversionError;
        let msg = format!("{err}");
        assert!(msg.contains("incompatible"));
        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &err;
    }
}
