use thiserror::Error;

/// Position in the input — line and column are 1-based, byte offset 0-based,
/// matching libexpat's error reporting convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub byte_offset: usize,
}

impl Position {
    pub const fn start() -> Self {
        Self { line: 1, column: 1, byte_offset: 0 }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum XmlError {
    /// Malformed XML — not well-formed per W3C XML 1.0 §2.1.
    #[error("not well-formed at {pos:?}: {reason}")]
    NotWellFormed { pos: Position, reason: String },

    /// Input ended in the middle of a construct.
    #[error("unexpected end of input at {pos:?} (in {context})")]
    UnexpectedEof { pos: Position, context: &'static str },

    /// A character that is not allowed in XML 1.0 (per §2.2 [Production 2]).
    #[error("invalid character {char:?} at {pos:?}")]
    InvalidChar { pos: Position, char: char },

    /// Encoding-related error.
    #[error("encoding error at {pos:?}: {reason}")]
    Encoding { pos: Position, reason: String },
}

pub type Result<T> = std::result::Result<T, XmlError>;
