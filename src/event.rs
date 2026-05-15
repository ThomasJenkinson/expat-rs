//! Parser events — the layer above tokens.
//!
//! Where `Token` is a lexical unit, `Event` is a semantic unit emitted by the
//! [`crate::parser::Parser`] after applying well-formedness rules. This is the
//! shape callers actually consume — equivalent to libexpat's
//! `XML_StartElementHandler` / `XML_EndElementHandler` callbacks.

use crate::token::{Attr, XmlDecl};

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    /// `<?xml version="..." ...?>`. May appear at most once, as the first
    /// event in the stream. Per W3C XML 1.0 §2.8.
    XmlDecl(XmlDecl<'a>),

    /// `<!DOCTYPE name ...>`. May appear at most once, before the root
    /// element. Per §2.8 [Production 28].
    Doctype { name: &'a str, body: &'a str },

    /// Element start (or empty-element). Per §3.
    StartElement { name: &'a str, attributes: Vec<Attr<'a>> },

    /// Element end. For empty-element tags (`<x/>`), the parser emits both
    /// a `StartElement` and an `EndElement` so callers see a uniform stream.
    EndElement(&'a str),

    /// Character data per §2.4. Includes text emitted by entity expansion
    /// (week 4+).
    Text(&'a str),

    /// `<![CDATA[...]]>`. Surfaces separately from `Text` so callers that
    /// care about the original source representation can distinguish them.
    CData(&'a str),

    /// `<!-- ... -->`. Per §2.5.
    Comment(&'a str),

    /// `<?target body?>`, excluding the XML declaration. Per §2.6.
    ProcessingInstruction { target: &'a str, body: &'a str },
}
