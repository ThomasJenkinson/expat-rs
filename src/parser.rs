//! Well-formedness checker — the parser layer.
//!
//! Consumes [`Token`]s from [`crate::lexer::Lexer`] and emits [`Event`]s after
//! enforcing the well-formedness constraints of W3C XML 1.0 §2.1:
//!
//! - exactly one root element (§2.1 #1)
//! - all start-tags have matching end-tags, properly nested (§2.1 #2, §3.1)
//! - the XML declaration, if present, is the very first thing (§2.8)
//! - the document type declaration appears at most once and only in the prolog
//! - attribute names are unique per element (§3.1)
//! - no character data appears outside the root element (§2.1)
//! - entity-reference resolution stays within the well-formedness contract
//!   (week 4+ — leases the work to the parser; the lexer surfaces references
//!   raw)

use crate::error::{Position, Result, XmlError};
use crate::event::Event;
use crate::lexer::Lexer;
use crate::token::Token;
use crate::entities::{builtin_entity, parse_internal_subset, EntityTable, ExpansionLimits};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Phase {
    /// Before any element — XmlDecl, Doctype, comments, PIs allowed.
    Prolog,
    /// Inside or after the root element.
    Body,
    /// After the root element has closed — only comments/PIs/whitespace
    /// allowed (epilog, per §2.1).
    Epilog,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    stack: Vec<&'a str>,
    phase: Phase,
    saw_xmldecl: bool,
    saw_doctype: bool,
    /// If a previous `next_event` returned an `EndElement` for an empty-element
    /// tag like `<br/>`, the matching synthetic end is queued here.
    pending_end: Option<&'a str>,
    last_pos: Position,
    /// Entities declared by `<!ENTITY ...>` in the DOCTYPE internal subset.
    entities: EntityTable,
    expansion_limits: ExpansionLimits,
    /// Cumulative bytes of expanded entity content seen so far in this
    /// document. The per-reference budget alone doesn't catch the
    /// quadratic-blowup pattern (linear-size payload with N references to a
    /// single benign-looking entity); the cumulative budget does.
    expanded_bytes_total: usize,
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            lexer: Lexer::new(src),
            stack: Vec::new(),
            phase: Phase::Prolog,
            saw_xmldecl: false,
            saw_doctype: false,
            pending_end: None,
            last_pos: Position::start(),
            entities: EntityTable::new(),
            expansion_limits: ExpansionLimits::default(),
            expanded_bytes_total: 0,
        }
    }

    /// Configure entity-expansion limits — defaults are conservative and
    /// suitable for adversarial input. Lower for stricter mitigation.
    pub fn with_expansion_limits(mut self, limits: ExpansionLimits) -> Self {
        self.expansion_limits = limits;
        self
    }

    /// Produce the next event, or `Ok(None)` at end of well-formed input.
    pub fn next_event(&mut self) -> Result<Option<Event<'a>>> {
        if let Some(name) = self.pending_end.take() {
            // Pop the matching push from the EmptyTag handler and advance to
            // Epilog if this closed the root.
            self.stack.pop();
            if self.stack.is_empty() {
                self.phase = Phase::Epilog;
            }
            return Ok(Some(Event::EndElement(name)));
        }
        loop {
            let tok = match self.lexer.next_token()? {
                Some(t) => t,
                None    => return self.handle_eof(),
            };
            self.last_pos = self.lexer_pos();

            match self.handle(tok)? {
                None    => continue, // event was consumed/folded — keep going
                Some(e) => return Ok(Some(e)),
            }
        }
    }

    fn lexer_pos(&self) -> Position { Position::start() }

    fn handle_eof(&mut self) -> Result<Option<Event<'a>>> {
        if !self.stack.is_empty() {
            return Err(XmlError::NotWellFormed {
                pos: self.last_pos,
                reason: format!("unclosed element {:?}", self.stack.last().unwrap()),
            });
        }
        if self.phase == Phase::Prolog {
            return Err(XmlError::NotWellFormed {
                pos: self.last_pos,
                reason: "no root element".into(),
            });
        }
        Ok(None)
    }

    fn handle(&mut self, tok: Token<'a>) -> Result<Option<Event<'a>>> {
        match tok {
            Token::XmlDecl(d) => {
                if self.saw_xmldecl {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "duplicate XML declaration".into(),
                    });
                }
                if self.phase != Phase::Prolog || self.saw_doctype || !self.stack.is_empty() {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "XML declaration must be the first thing in the document".into(),
                    });
                }
                self.saw_xmldecl = true;
                Ok(Some(Event::XmlDecl(d)))
            }
            Token::Doctype { name, body } => {
                if self.saw_doctype {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "duplicate DOCTYPE declaration".into(),
                    });
                }
                if self.phase != Phase::Prolog {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "DOCTYPE must appear before the root element".into(),
                    });
                }
                self.saw_doctype = true;
                // Parse <!ENTITY ...> declarations from the internal subset
                parse_internal_subset(body, &mut self.entities);
                Ok(Some(Event::Doctype { name, body }))
            }
            Token::StartTag { name, attributes } => {
                if self.phase == Phase::Epilog {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "second root element not allowed".into(),
                    });
                }
                // QName check (Namespaces 1.0 §3) is opt-in via check_qname —
                // pure XML 1.0 allows multiple colons in Names.
                Self::check_unique_attrs(&attributes, self.last_pos)?;
                self.phase = Phase::Body;
                self.stack.push(name);
                Ok(Some(Event::StartElement { name, attributes }))
            }
            Token::EmptyTag { name, attributes } => {
                if self.phase == Phase::Epilog {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "second root element not allowed".into(),
                    });
                }
                Self::check_unique_attrs(&attributes, self.last_pos)?;
                // Push to mirror what a normal start tag does — the matching
                // pending_end will pop it on the next call.
                self.stack.push(name);
                self.pending_end = Some(name);
                self.phase = Phase::Body;
                Ok(Some(Event::StartElement { name, attributes }))
            }
            Token::EndTag(name) => {
                let top = self.stack.pop().ok_or_else(|| XmlError::NotWellFormed {
                    pos: self.last_pos,
                    reason: format!("unexpected end tag </{name}> with no matching start"),
                })?;
                if top != name {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: format!("end tag </{name}> does not match start tag <{top}>"),
                    });
                }
                if self.stack.is_empty() {
                    self.phase = Phase::Epilog;
                }
                Ok(Some(Event::EndElement(name)))
            }
            Token::Text(s) => {
                // Per §2.1: character data only inside the root element.
                if self.stack.is_empty() {
                    if !s.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) {
                        return Err(XmlError::NotWellFormed {
                            pos: self.last_pos,
                            reason: "non-whitespace text outside the root element".into(),
                        });
                    }
                    // Whitespace in the prolog/epilog is silently absorbed.
                    return Ok(None);
                }
                Ok(Some(Event::Text(s)))
            }
            Token::CData(s) => {
                if self.stack.is_empty() {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "CDATA section outside the root element".into(),
                    });
                }
                Ok(Some(Event::CData(s)))
            }
            Token::Comment(s) => Ok(Some(Event::Comment(s))),
            Token::ProcessingInstruction { target, body } => {
                Ok(Some(Event::ProcessingInstruction { target, body }))
            }
            // Entity references: built-in (always available) or DTD-declared.
            // Either is validated for well-formedness; expansion size is
            // capped to defend against billion-laughs / quadratic-blowup.
            Token::EntityRef(name) => {
                if self.stack.is_empty() {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "entity reference outside the root element".into(),
                    });
                }
                if let Some(text) = builtin_entity(name) {
                    return Ok(Some(Event::Text(text)));
                }
                if self.entities.is_declared(name) {
                    // Validate (and budget-check) the expansion against the
                    // per-reference budget, then add to the document-wide
                    // running total. Both must be enforced to defeat
                    // quadratic-blowup payloads (many refs to one entity).
                    let added = self.entities.validate_expansion(
                        name, self.expansion_limits, self.last_pos,
                    )?;
                    self.expanded_bytes_total = self.expanded_bytes_total.saturating_add(added);
                    if self.expanded_bytes_total > self.expansion_limits.max_expanded_bytes {
                        return Err(XmlError::NotWellFormed {
                            pos: self.last_pos,
                            reason: format!(
                                "cumulative entity expansion exceeded {} bytes — \
                                 possible quadratic-blowup payload",
                                self.expansion_limits.max_expanded_bytes),
                        });
                    }
                    return Ok(Some(Event::Text("")));
                }
                Err(XmlError::NotWellFormed {
                    pos: self.last_pos,
                    reason: format!("undeclared entity {name:?}"),
                })
            }
            Token::CharRef(c) => {
                if self.stack.is_empty() {
                    return Err(XmlError::NotWellFormed {
                        pos: self.last_pos,
                        reason: "character reference outside the root element".into(),
                    });
                }
                // Surface as Text for now — eventually we'd allocate or borrow.
                // For week 2 we lose the source slice; that's acceptable for
                // well-formedness checking.
                let _ = c;
                Ok(Some(Event::Text("")))
            }
        }
    }

    /// Per W3C XML Namespaces 1.0 §3 (Qualified Names): a QName has at most
    /// one colon, with non-empty prefix and non-empty local name. Names
    /// without a colon are unprefixed and always valid here.
    ///
    /// Note: pure XML 1.0 (without Namespaces) allows multiple colons in
    /// Names. This check is therefore not applied unconditionally — it's
    /// reserved for callers that have opted into namespace processing
    /// (a future `Parser::namespace_aware()` mode).
    #[allow(dead_code)]
    fn check_qname(name: &str, pos: Position) -> Result<()> {
        let mut parts = name.split(':');
        let first  = parts.next().unwrap_or("");
        let second = parts.next();
        let third  = parts.next();
        if third.is_some() {
            return Err(XmlError::NotWellFormed {
                pos, reason: format!("QName {name:?} has more than one colon"),
            });
        }
        if let Some(local) = second {
            if first.is_empty() || local.is_empty() {
                return Err(XmlError::NotWellFormed {
                    pos, reason: format!("QName {name:?} has empty prefix or local part"),
                });
            }
        }
        Ok(())
    }

    /// Per §3.1 (Unique Att Spec): no element may have two attributes with
    /// the same name.
    fn check_unique_attrs(attrs: &[crate::token::Attr<'_>], pos: Position) -> Result<()> {
        for (i, a) in attrs.iter().enumerate() {
            for b in &attrs[..i] {
                if a.name == b.name {
                    return Err(XmlError::NotWellFormed {
                        pos,
                        reason: format!("duplicate attribute {:?}", a.name),
                    });
                }
            }
        }
        Ok(())
    }
}

// Built-in entities are defined in `crate::entities::builtin_entity`.
