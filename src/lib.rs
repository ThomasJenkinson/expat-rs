//! expat-rs — memory-safe XML 1.0 parser, libexpat-compatible conformance.
//!
//! Built clean-room from the W3C XML 1.0 Recommendation. See
//! [`METHODOLOGY.md`](../METHODOLOGY.md) at the crate root for the
//! clean-room declaration and audit trail.

pub mod error;
pub mod token;
pub mod lexer;
pub mod event;
pub mod parser;
pub mod entities;

pub use error::{Position, XmlError, Result};
pub use event::Event;
pub use lexer::Lexer;
pub use parser::Parser;
pub use token::{Attr, Token, XmlDecl};
pub use entities::{EntityTable, ExpansionLimits};
