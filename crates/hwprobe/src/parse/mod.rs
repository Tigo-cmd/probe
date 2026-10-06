//! Parsers for binary structures read from hardware.
//!
//! These take untrusted bytes: a malformed or deliberately crafted blob must
//! produce an error or a partial result, never a panic. Every parser is pure
//! so it can be fuzzed and tested against captured fixtures.

pub mod ata;
pub mod edid;
pub mod nvme;

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    TooShort { expected: usize, got: usize },
    BadHeader,
    BadChecksum,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::TooShort { expected, got } => {
                write!(
                    f,
                    "structure too short: expected {expected} bytes, got {got}"
                )
            }
            ParseError::BadHeader => write!(f, "bad header"),
            ParseError::BadChecksum => write!(f, "checksum mismatch"),
        }
    }
}

impl std::error::Error for ParseError {}

fn require_len(buf: &[u8], len: usize) -> Result<(), ParseError> {
    if buf.len() < len {
        Err(ParseError::TooShort {
            expected: len,
            got: buf.len(),
        })
    } else {
        Ok(())
    }
}

fn le_u128(buf: &[u8], offset: usize) -> u128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&buf[offset..offset + 16]);
    u128::from_le_bytes(bytes)
}
