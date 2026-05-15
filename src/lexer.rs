//! XML 1.0 tokeniser.
//!
//! Stream-style: borrows the input, returns successive `Token`s. No
//! allocation per token (attributes vector is the only allocation, and it
//! lives in the token).
//!
//! Implements the lexical rules of W3C XML 1.0 (Fifth Edition) §2 and §3.
//! Each non-trivial scan function carries a spec citation.

use crate::error::{Position, Result, XmlError};
use crate::token::{Attr, Token, XmlDecl};

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: Position,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: Position::start(),
        }
    }

    /// Advance past one byte, updating line/column.
    fn bump(&mut self) {
        let b = self.src[self.pos.byte_offset];
        self.pos.byte_offset += 1;
        if b == b'\n' {
            self.pos.line += 1;
            self.pos.column = 1;
        } else {
            self.pos.column += 1;
        }
    }

    fn peek(&self, n: usize) -> Option<u8> {
        self.src.get(self.pos.byte_offset + n).copied()
    }

    fn is_eof(&self) -> bool {
        self.pos.byte_offset >= self.src.len()
    }

    fn current(&self) -> Option<u8> {
        self.peek(0)
    }

    /// Skip XML whitespace per §2.3 [Production 3]: S ::= (#x20 | #x9 | #xD | #xA)+
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current() {
            if matches!(c, b' ' | b'\t' | b'\r' | b'\n') {
                self.bump();
            } else {
                break;
            }
        }
    }

    /// Return the slice from `start` to current byte offset.
    fn slice(&self, start: usize) -> &'a str {
        // SAFETY: `start..byte_offset` is always within the original `&str`
        // because `bump` only ever increments through valid UTF-8 boundaries
        // (we never bump mid-codepoint — this lexer is byte-oriented for ASCII
        // metacharacters and treats non-ASCII as opaque payload bytes).
        std::str::from_utf8(&self.src[start..self.pos.byte_offset]).unwrap_or("")
    }

    /// Per §2.3 [Production 4]: NameStartChar — full Unicode ranges.
    pub(crate) fn is_name_start_char(c: char) -> bool {
        matches!(c,
            ':' | '_' | 'A'..='Z' | 'a'..='z' |
            '\u{C0}'..='\u{D6}'    | '\u{D8}'..='\u{F6}'   |
            '\u{F8}'..='\u{2FF}'   | '\u{370}'..='\u{37D}' |
            '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}' |
            '\u{2070}'..='\u{218F}'| '\u{2C00}'..='\u{2FEF}' |
            '\u{3001}'..='\u{D7FF}'| '\u{F900}'..='\u{FDCF}' |
            '\u{FDF0}'..='\u{FFFD}'| '\u{10000}'..='\u{EFFFF}'
        )
    }

    /// Per §2.3 [Production 4a]: NameChar = NameStartChar | "-" | "." |
    /// [0-9] | #xB7 | [#x0300-#x036F] | [#x203F-#x2040].
    pub(crate) fn is_name_char(c: char) -> bool {
        Self::is_name_start_char(c) || matches!(c,
            '-' | '.' | '0'..='9' | '\u{B7}' |
            '\u{0300}'..='\u{036F}' | '\u{203F}'..='\u{2040}'
        )
    }

    /// Decode the char at the current byte position. UTF-8 codepoints are
    /// 1–4 bytes; we look ahead at most 4 bytes.
    fn current_char(&self) -> Option<(char, usize)> {
        let bytes = &self.src[self.pos.byte_offset..];
        if bytes.is_empty() { return None; }
        let chunk = &bytes[..bytes.len().min(4)];
        let s = std::str::from_utf8(chunk).ok()?;
        let c = s.chars().next()?;
        Some((c, c.len_utf8()))
    }

    /// Advance past one Unicode codepoint by its UTF-8 byte length.
    fn bump_char(&mut self, byte_len: usize) {
        // \n is single-byte in UTF-8, so newline tracking works at byte level.
        let is_newline = byte_len == 1 && self.src[self.pos.byte_offset] == b'\n';
        self.pos.byte_offset += byte_len;
        if is_newline {
            self.pos.line += 1;
            self.pos.column = 1;
        } else {
            self.pos.column += 1;
        }
    }

    /// Scan a Name per §2.3 [Production 5]. Honours full Unicode
    /// NameStartChar / NameChar ranges.
    fn scan_name(&mut self) -> Result<&'a str> {
        let start = self.pos.byte_offset;
        match self.current_char() {
            Some((c, len)) if Self::is_name_start_char(c) => self.bump_char(len),
            Some((c, _))   => return Err(XmlError::InvalidChar { pos: self.pos, char: c }),
            None           => return Err(XmlError::UnexpectedEof { pos: self.pos, context: "Name" }),
        }
        while let Some((c, len)) = self.current_char() {
            if Self::is_name_char(c) { self.bump_char(len); } else { break; }
        }
        Ok(self.slice(start))
    }

    /// Scan an attribute value per §3.1 [Production 10]: AttValue ::= '"' ([^<&"] | Reference)* '"'
    /// (or single-quoted variant). Returns the *literal* slice between the
    /// quotes; entity expansion happens later in the parser layer.
    fn scan_attr_value(&mut self) -> Result<&'a str> {
        let quote = match self.current() {
            Some(q @ (b'"' | b'\'')) => q,
            Some(c) => return Err(XmlError::NotWellFormed {
                pos: self.pos,
                reason: format!("expected quote to start attribute value, got {:?}", c as char),
            }),
            None => return Err(XmlError::UnexpectedEof { pos: self.pos, context: "AttValue" }),
        };
        self.bump(); // consume opening quote
        let start = self.pos.byte_offset;
        loop {
            match self.current() {
                Some(c) if c == quote => {
                    let s = self.slice(start);
                    self.bump(); // consume closing quote
                    return Ok(s);
                }
                Some(b'<') => return Err(XmlError::NotWellFormed {
                    pos: self.pos, reason: "'<' not allowed in attribute value".into(),
                }),
                Some(_) => self.bump(),
                None => return Err(XmlError::UnexpectedEof {
                    pos: self.pos, context: "AttValue",
                }),
            }
        }
    }

    /// Scan a `<!-- ... -->` comment per §2.5 [Production 15]. The leading
    /// `<!--` has been consumed by the caller. Returns the body (without the
    /// surrounding markers) and consumes the `-->`.
    fn scan_comment_body(&mut self) -> Result<&'a str> {
        let start = self.pos.byte_offset;
        loop {
            if self.is_eof() {
                return Err(XmlError::UnexpectedEof { pos: self.pos, context: "Comment" });
            }
            if self.current() == Some(b'-') && self.peek(1) == Some(b'-') {
                let body = self.slice(start);
                // Per §2.5: "for compatibility, the string '--' MUST NOT occur
                // within comments." If we see -- followed by anything other
                // than > we error.
                if self.peek(2) != Some(b'>') {
                    return Err(XmlError::NotWellFormed {
                        pos: self.pos,
                        reason: "'--' not allowed within a comment".into(),
                    });
                }
                self.bump(); self.bump(); self.bump(); // consume -->
                return Ok(body);
            }
            self.bump();
        }
    }

    /// Scan a `<![CDATA[...]]>` section per §2.7 [Production 18]. The leading
    /// `<![CDATA[` has been consumed.
    fn scan_cdata_body(&mut self) -> Result<&'a str> {
        let start = self.pos.byte_offset;
        loop {
            if self.is_eof() {
                return Err(XmlError::UnexpectedEof { pos: self.pos, context: "CDATA" });
            }
            if self.current() == Some(b']') && self.peek(1) == Some(b']') && self.peek(2) == Some(b'>') {
                let body = self.slice(start);
                self.bump(); self.bump(); self.bump();
                return Ok(body);
            }
            self.bump();
        }
    }

    /// Scan a processing instruction body per §2.6 [Production 16]:
    /// `<?target ...?>`. The leading `<?` has been consumed; the target is
    /// scanned by the caller.
    fn scan_pi_body(&mut self) -> Result<&'a str> {
        // Optionally skip whitespace between target and body
        self.skip_whitespace();
        let start = self.pos.byte_offset;
        loop {
            if self.is_eof() {
                return Err(XmlError::UnexpectedEof { pos: self.pos, context: "PI" });
            }
            if self.current() == Some(b'?') && self.peek(1) == Some(b'>') {
                let body = self.slice(start);
                self.bump(); self.bump();
                return Ok(body);
            }
            self.bump();
        }
    }

    /// Top-level: produce the next token, or `Ok(None)` at end of input.
    pub fn next_token(&mut self) -> Result<Option<Token<'a>>> {
        if self.is_eof() {
            return Ok(None);
        }
        // After the prolog, characters between tags are Text. Inside the
        // prolog (before the root element), only whitespace is allowed.
        match self.current().unwrap() {
            b'<' => self.scan_open_construct().map(Some),
            b'&' => self.scan_reference().map(Some),
            _    => self.scan_text().map(Some),
        }
    }

    /// Dispatch on whatever follows a `<`.
    fn scan_open_construct(&mut self) -> Result<Token<'a>> {
        debug_assert_eq!(self.current(), Some(b'<'));
        match self.peek(1) {
            Some(b'/') => {
                self.bump(); self.bump(); // </
                let name = self.scan_name()?;
                self.skip_whitespace();
                if self.current() != Some(b'>') {
                    return Err(XmlError::NotWellFormed {
                        pos: self.pos, reason: "expected '>' to close end tag".into(),
                    });
                }
                self.bump();
                Ok(Token::EndTag(name))
            }
            Some(b'!') => {
                // Could be <!--, <![CDATA[, <!DOCTYPE
                if self.src[self.pos.byte_offset..].starts_with(b"<!--") {
                    self.pos.byte_offset += 4; // shortcut bump
                    self.pos.column += 4;
                    Ok(Token::Comment(self.scan_comment_body()?))
                } else if self.src[self.pos.byte_offset..].starts_with(b"<![CDATA[") {
                    self.pos.byte_offset += 9;
                    self.pos.column += 9;
                    Ok(Token::CData(self.scan_cdata_body()?))
                } else if self.src[self.pos.byte_offset..].starts_with(b"<!DOCTYPE") {
                    self.scan_doctype()
                } else {
                    Err(XmlError::NotWellFormed {
                        pos: self.pos, reason: "unrecognised <! construct".into(),
                    })
                }
            }
            Some(b'?') => {
                self.bump(); self.bump(); // <?
                let target = self.scan_name()?;
                // §2.8: <?xml ...?> is the XML declaration, with constraints
                if target.eq_ignore_ascii_case("xml") {
                    self.scan_xmldecl_after_target()
                } else {
                    let body = self.scan_pi_body()?;
                    Ok(Token::ProcessingInstruction { target, body })
                }
            }
            Some(_) => self.scan_start_or_empty(),
            None    => Err(XmlError::UnexpectedEof { pos: self.pos, context: "tag" }),
        }
    }

    fn scan_start_or_empty(&mut self) -> Result<Token<'a>> {
        debug_assert_eq!(self.current(), Some(b'<'));
        self.bump(); // <
        let name = self.scan_name()?;
        let mut attributes = Vec::new();
        loop {
            self.skip_whitespace();
            match self.current() {
                Some(b'>') => {
                    self.bump();
                    return Ok(Token::StartTag { name, attributes });
                }
                Some(b'/') => {
                    self.bump();
                    if self.current() != Some(b'>') {
                        return Err(XmlError::NotWellFormed {
                            pos: self.pos, reason: "expected '>' after '/'".into(),
                        });
                    }
                    self.bump();
                    return Ok(Token::EmptyTag { name, attributes });
                }
                Some(c) if Self::is_name_start_char(c as char) => {
                    let attr_pos = self.pos;
                    let aname = self.scan_name()?;
                    self.skip_whitespace();
                    if self.current() != Some(b'=') {
                        return Err(XmlError::NotWellFormed {
                            pos: self.pos, reason: "expected '=' after attribute name".into(),
                        });
                    }
                    self.bump();
                    self.skip_whitespace();
                    let value = self.scan_attr_value()?;
                    attributes.push(Attr { name: aname, value, pos: attr_pos });
                }
                Some(c) => return Err(XmlError::NotWellFormed {
                    pos: self.pos,
                    reason: format!("unexpected {:?} in start tag", c as char),
                }),
                None => return Err(XmlError::UnexpectedEof {
                    pos: self.pos, context: "start tag",
                }),
            }
        }
    }

    /// XML declaration per §2.8 [Production 23]: `<?xml VersionInfo ?>` —
    /// the leading `<?xml` has been consumed; `target` was already verified.
    fn scan_xmldecl_after_target(&mut self) -> Result<Token<'a>> {
        // Required: version="..."
        self.skip_whitespace();
        self.expect_keyword(b"version")?;
        self.skip_whitespace();
        if self.current() != Some(b'=') {
            return Err(XmlError::NotWellFormed { pos: self.pos, reason: "expected '=' after version".into() });
        }
        self.bump(); self.skip_whitespace();
        let version = self.scan_attr_value()?;

        let mut encoding = None;
        let mut standalone = None;
        loop {
            self.skip_whitespace();
            if self.current() == Some(b'?') && self.peek(1) == Some(b'>') {
                self.bump(); self.bump();
                return Ok(Token::XmlDecl(XmlDecl { version, encoding, standalone }));
            }
            // optional encoding=, standalone= in any order (relaxed for week 1)
            if self.try_keyword(b"encoding") {
                self.skip_whitespace();
                if self.current() != Some(b'=') {
                    return Err(XmlError::NotWellFormed { pos: self.pos, reason: "expected '=' after encoding".into() });
                }
                self.bump(); self.skip_whitespace();
                encoding = Some(self.scan_attr_value()?);
            } else if self.try_keyword(b"standalone") {
                self.skip_whitespace();
                if self.current() != Some(b'=') {
                    return Err(XmlError::NotWellFormed { pos: self.pos, reason: "expected '=' after standalone".into() });
                }
                self.bump(); self.skip_whitespace();
                let v = self.scan_attr_value()?;
                standalone = Some(match v {
                    "yes" => true,
                    "no"  => false,
                    other => return Err(XmlError::NotWellFormed {
                        pos: self.pos,
                        reason: format!("standalone must be 'yes' or 'no', got {other:?}"),
                    }),
                });
            } else {
                return Err(XmlError::NotWellFormed {
                    pos: self.pos, reason: "unexpected token in XML declaration".into(),
                });
            }
        }
    }

    fn try_keyword(&mut self, kw: &[u8]) -> bool {
        if self.src[self.pos.byte_offset..].starts_with(kw) {
            // Make sure it's not a prefix of a longer name
            let after = self.pos.byte_offset + kw.len();
            if after >= self.src.len() || !Self::is_name_char(self.src[after] as char) {
                self.pos.byte_offset += kw.len();
                self.pos.column += kw.len() as u32;
                return true;
            }
        }
        false
    }

    fn expect_keyword(&mut self, kw: &[u8]) -> Result<()> {
        if self.try_keyword(kw) {
            Ok(())
        } else {
            Err(XmlError::NotWellFormed {
                pos: self.pos,
                reason: format!("expected keyword {:?}", std::str::from_utf8(kw).unwrap_or("?")),
            })
        }
    }

    /// Scan a DOCTYPE declaration per §2.8 [Production 28]. Week 1: capture
    /// the name and the body bytes between `<!DOCTYPE` and the matching `>`,
    /// without parsing internal subset / external IDs.
    fn scan_doctype(&mut self) -> Result<Token<'a>> {
        debug_assert!(self.src[self.pos.byte_offset..].starts_with(b"<!DOCTYPE"));
        self.pos.byte_offset += 9;
        self.pos.column += 9;
        self.skip_whitespace();
        let name = self.scan_name()?;
        let body_start = self.pos.byte_offset;
        // Track bracket depth for internal subset
        let mut depth = 0i32;
        loop {
            match self.current() {
                None => return Err(XmlError::UnexpectedEof { pos: self.pos, context: "DOCTYPE" }),
                Some(b'[') => { depth += 1; self.bump(); }
                Some(b']') => { depth -= 1; self.bump(); }
                Some(b'>') if depth == 0 => {
                    let body = std::str::from_utf8(&self.src[body_start..self.pos.byte_offset])
                        .unwrap_or("").trim();
                    self.bump();
                    return Ok(Token::Doctype { name, body });
                }
                Some(_) => self.bump(),
            }
        }
    }

    /// Scan character data per §2.4 [Production 14] until the next `<` or `&`.
    fn scan_text(&mut self) -> Result<Token<'a>> {
        let start = self.pos.byte_offset;
        while let Some(c) = self.current() {
            if c == b'<' || c == b'&' { break; }
            // Per §2.4: `]]>` MUST NOT occur in character data
            if c == b']' && self.peek(1) == Some(b']') && self.peek(2) == Some(b'>') {
                return Err(XmlError::NotWellFormed {
                    pos: self.pos,
                    reason: "']]>' not allowed in character data".into(),
                });
            }
            self.bump();
        }
        Ok(Token::Text(self.slice(start)))
    }

    /// Reference per §4.1 [Productions 66–68]: `&name;` or `&#nnn;` / `&#xhh;`.
    fn scan_reference(&mut self) -> Result<Token<'a>> {
        debug_assert_eq!(self.current(), Some(b'&'));
        self.bump();
        if self.current() == Some(b'#') {
            self.bump();
            let (radix, num_start) = if self.current() == Some(b'x') {
                self.bump();
                (16, self.pos.byte_offset)
            } else {
                (10, self.pos.byte_offset)
            };
            while let Some(c) = self.current() {
                if c == b';' { break; }
                if !c.is_ascii_hexdigit() {
                    return Err(XmlError::NotWellFormed {
                        pos: self.pos,
                        reason: format!("invalid character {:?} in numeric character reference", c as char),
                    });
                }
                self.bump();
            }
            let digits = std::str::from_utf8(&self.src[num_start..self.pos.byte_offset]).unwrap_or("");
            if self.current() != Some(b';') {
                return Err(XmlError::NotWellFormed { pos: self.pos, reason: "expected ';' to close character reference".into() });
            }
            self.bump();
            let n = u32::from_str_radix(digits, radix).map_err(|_| XmlError::NotWellFormed {
                pos: self.pos, reason: format!("invalid numeric reference {digits:?}"),
            })?;
            let c = char::from_u32(n).ok_or(XmlError::NotWellFormed {
                pos: self.pos, reason: format!("character reference {n:#x} is not a valid Unicode scalar"),
            })?;
            Ok(Token::CharRef(c))
        } else {
            let name = self.scan_name()?;
            if self.current() != Some(b';') {
                return Err(XmlError::NotWellFormed { pos: self.pos, reason: "expected ';' to close entity reference".into() });
            }
            self.bump();
            Ok(Token::EntityRef(name))
        }
    }
}
