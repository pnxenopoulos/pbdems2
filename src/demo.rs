//! Game-independent PBDEMS2 command framing.

use crate::error::{Error, Result};
use crate::io::ByteReader;
use crate::limits::DecodeLimits;

mod stream;

pub use stream::{
    COMPRESSED_FLAG, CommandFrame, CommandIter, CommandPosition, Demo, DemoIndex, HEADER_SIZE,
    command,
};

/// Magic bytes at the beginning of every Source 2 demo.
pub const MAGIC: [u8; 8] = *b"PBDEMS2\0";

/// Header shared by every command in a demo stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CmdHeader {
    /// Command identifier with compression flags removed.
    pub cmd: i32,
    /// Demo tick associated with the command.
    pub tick: i32,
    /// Whether the command body uses Snappy compression.
    pub compressed: bool,
    /// Command payload size before decompression.
    pub body_size: u32,
}

impl CmdHeader {
    /// Construct a command header, primarily for adapters and test fixtures.
    pub const fn new(cmd: i32, tick: i32, compressed: bool, body_size: u32) -> Self {
        Self {
            cmd,
            tick,
            compressed,
            body_size,
        }
    }
}

/// Verify the PBDEMS2 magic prefix and fixed-size file header.
pub fn verify_header(data: &[u8], header_size: usize) -> Result<()> {
    if data.len() < header_size {
        return Err(Error::Parse {
            context: format!(
                "file too small for demo header: expected at least {header_size} bytes, got {}",
                data.len()
            ),
        });
    }

    let mut got = [0_u8; 8];
    got.copy_from_slice(&data[..8]);
    if got != MAGIC {
        return Err(Error::InvalidMagic { got });
    }
    Ok(())
}

/// Read one command header from the current byte position.
pub fn read_cmd_header(reader: &mut ByteReader<'_>, compressed_flag: u32) -> Result<CmdHeader> {
    let raw_cmd = reader.read_uvarint32()?;
    Ok(CmdHeader {
        cmd: (raw_cmd & !compressed_flag) as i32,
        tick: reader.read_uvarint32()? as i32,
        compressed: raw_cmd & compressed_flag != 0,
        body_size: reader.read_uvarint32()?,
    })
}

/// Read and, when necessary, decompress a command body into the output buffer.
pub fn read_cmd_body(
    reader: &mut ByteReader<'_>,
    header: &CmdHeader,
    output: &mut Vec<u8>,
) -> Result<()> {
    read_cmd_body_with_limits(reader, header, output, &DecodeLimits::default())
}

/// Limit-aware variant of [`read_cmd_body`].
pub fn read_cmd_body_with_limits(
    reader: &mut ByteReader<'_>,
    header: &CmdHeader,
    output: &mut Vec<u8>,
    limits: &DecodeLimits,
) -> Result<()> {
    let body_size = header.body_size as usize;
    limits.ensure(
        "encoded command body",
        body_size,
        limits.max_command_body_bytes(),
    )?;
    let body = reader.read_bytes(body_size)?;
    output.clear();

    if header.compressed {
        let decompressed_len = snap::raw::decompress_len(body)
            .map_err(|error| Error::Decompress(error.to_string()))?;
        limits.ensure(
            "decompressed command body",
            decompressed_len,
            limits.max_decompressed_command_bytes(),
        )?;
        output
            .try_reserve(decompressed_len)
            .map_err(|_| Error::Allocation {
                resource: "decompressed command body",
                requested: decompressed_len,
            })?;
        output.resize(decompressed_len, 0);
        snap::raw::Decoder::new()
            .decompress(body, output)
            .map_err(|error| Error::Decompress(error.to_string()))?;
    } else {
        output
            .try_reserve(body.len())
            .map_err(|_| Error::Allocation {
                resource: "command body",
                requested: body.len(),
            })?;
        output.extend_from_slice(body);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_header() {
        let mut data = Vec::from(MAGIC);
        data.extend_from_slice(&[0; 8]);
        assert!(verify_header(&data, 16).is_ok());
        assert!(matches!(
            verify_header(b"NOTADEMO--------", 16),
            Err(Error::InvalidMagic { .. })
        ));
        assert!(verify_header(&MAGIC, 16).is_err());
    }

    #[test]
    fn reads_command_header() {
        let mut reader = ByteReader::new(&[7, 42, 3]);
        let header = read_cmd_header(&mut reader, 1 << 31).unwrap();
        assert_eq!(
            header,
            CmdHeader {
                cmd: 7,
                tick: 42,
                compressed: false,
                body_size: 3,
            }
        );
    }
}
