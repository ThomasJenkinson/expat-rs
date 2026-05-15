//! DTD entity declarations + safe expansion.
//!
//! The XML 1.0 spec [§4.7] permits internal general entity declarations of
//! the form:
//!
//!   <!ENTITY name "value">       — double-quoted
//!   <!ENTITY name 'value'>       — single-quoted
//!
//! Entity values may themselves reference other entities. Naïve recursive
//! expansion is the **billion laughs** vulnerability class:
//!
//!   <!ENTITY a "AAA">
//!   <!ENTITY b "&a;&a;&a;&a;&a;">     <!-- 5× a -->
//!   <!ENTITY c "&b;&b;&b;&b;&b;">     <!-- 5×b = 25×a -->
//!   ...                                <!-- exponential -->
//!
//! Mitigation here is a hard recursion-depth cap and a hard cumulative
//! expansion-size cap. Either limit triggers an error rather than letting
//! the parser allocate unbounded memory.

use std::collections::HashMap;
use crate::error::{Position, Result, XmlError};

/// Hard limits on entity expansion. Defaults match the conservative end of
/// what's reasonable for adversarial input; library callers will be able to
/// tune these in a future config API.
#[derive(Clone, Copy, Debug)]
pub struct ExpansionLimits {
    /// Maximum depth of nested entity references during a single expansion.
    pub max_depth: usize,
    /// Maximum *total* number of bytes any single entity reference may
    /// expand to (sums across all nested references).
    pub max_expanded_bytes: usize,
}

impl Default for ExpansionLimits {
    fn default() -> Self {
        Self {
            max_depth: 20,
            max_expanded_bytes: 1 * 1024 * 1024, // 1 MiB
        }
    }
}

#[derive(Default, Debug)]
pub struct EntityTable {
    /// name → literal value. The value may contain nested `&other;` references
    /// which are resolved at expansion time (not at declaration time — that
    /// would itself be vulnerable to billion-laughs).
    entities: HashMap<String, String>,
}

impl EntityTable {
    pub fn new() -> Self { Self::default() }

    /// Add a declaration. Last-write-wins, mirroring upstream behaviour.
    pub fn declare(&mut self, name: String, value: String) {
        self.entities.insert(name, value);
    }

    pub fn is_declared(&self, name: &str) -> bool {
        self.entities.contains_key(name)
    }

    /// Compute the *length* a reference would expand to, recursing through
    /// nested references with the given limits. Returns the byte length on
    /// success, or an error if the limits are exceeded.
    ///
    /// We compute length without materialising the full string — this keeps
    /// the implementation cheap and means a billion-laughs payload errors
    /// out before we allocate anything large.
    pub fn validate_expansion(&self, name: &str, limits: ExpansionLimits, pos: Position) -> Result<usize> {
        let mut budget = limits.max_expanded_bytes;
        self.validate_recursive(name, limits.max_depth, &mut budget, pos)
    }

    fn validate_recursive(
        &self,
        name: &str,
        depth_remaining: usize,
        budget: &mut usize,
        pos: Position,
    ) -> Result<usize> {
        if depth_remaining == 0 {
            return Err(XmlError::NotWellFormed {
                pos,
                reason: format!(
                    "entity expansion of {name:?} exceeded the maximum nesting depth — \
                     possible billion-laughs / quadratic-blowup payload"),
            });
        }
        let value = match self.entities.get(name) {
            Some(v) => v.as_str(),
            None    => return Err(XmlError::NotWellFormed {
                pos, reason: format!("undeclared entity {name:?}"),
            }),
        };

        let mut total: usize = 0;
        let bytes = value.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'&' {
                // Find the ;
                let start = i + 1;
                let end = bytes[start..].iter().position(|&b| b == b';')
                    .ok_or_else(|| XmlError::NotWellFormed {
                        pos,
                        reason: format!("malformed reference inside entity {name:?} — no ';'"),
                    })? + start;
                let inner = std::str::from_utf8(&bytes[start..end])
                    .map_err(|_| XmlError::NotWellFormed {
                        pos, reason: "non-UTF-8 in entity reference".into(),
                    })?;
                let added = if let Some(s) = builtin_entity(inner) {
                    s.len()
                } else if inner.starts_with('#') {
                    // Numeric character reference — at most 4 bytes (one Unicode codepoint)
                    4
                } else {
                    // Recursive entity reference
                    self.validate_recursive(inner, depth_remaining - 1, budget, pos)?
                };
                if added > *budget {
                    return Err(XmlError::NotWellFormed {
                        pos,
                        reason: format!(
                            "entity expansion of {name:?} exceeded {} bytes — \
                             possible billion-laughs / quadratic-blowup payload",
                            crate::entities::ExpansionLimits::default().max_expanded_bytes),
                    });
                }
                *budget -= added;
                total = total.saturating_add(added);
                i = end + 1;
            } else {
                if *budget == 0 {
                    return Err(XmlError::NotWellFormed {
                        pos,
                        reason: format!("entity expansion of {name:?} exceeded byte budget"),
                    });
                }
                *budget -= 1;
                total = total.saturating_add(1);
                i += 1;
            }
        }
        Ok(total)
    }
}

/// Built-in entities defined by the XML spec (always available, no DTD needed).
/// Per §4.6 [Production 67].
pub fn builtin_entity(name: &str) -> Option<&'static str> {
    match name {
        "lt"   => Some("<"),
        "gt"   => Some(">"),
        "amp"  => Some("&"),
        "quot" => Some("\""),
        "apos" => Some("'"),
        _      => None,
    }
}

/// Parse `<!ENTITY name "value">` declarations out of a DOCTYPE internal
/// subset's body text. Other declarations (`<!ELEMENT>`, `<!ATTLIST>`,
/// `<!NOTATION>`) are skipped for now — week 4 only handles general entities.
pub fn parse_internal_subset(body: &str, table: &mut EntityTable) {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find the next '<!ENTITY'
        if bytes[i..].starts_with(b"<!ENTITY") {
            i += b"<!ENTITY".len();
            // Skip whitespace
            while i < bytes.len() && matches!(bytes[i], b' '|b'\t'|b'\r'|b'\n') { i += 1; }
            // Parameter entities (<!ENTITY % …>) are skipped in week 4.
            if i < bytes.len() && bytes[i] == b'%' {
                if let Some(end) = find_end_of_decl(bytes, i) { i = end; }
                else { break; }
                continue;
            }
            // Read the name
            let name_start = i;
            while i < bytes.len() && !matches!(bytes[i], b' '|b'\t'|b'\r'|b'\n'|b'>') { i += 1; }
            let name = match std::str::from_utf8(&bytes[name_start..i]) {
                Ok(s) if !s.is_empty() => s.to_string(),
                _ => { if let Some(end) = find_end_of_decl(bytes, i) { i = end; } else { break; } continue; }
            };
            // Skip whitespace
            while i < bytes.len() && matches!(bytes[i], b' '|b'\t'|b'\r'|b'\n') { i += 1; }
            // Read quoted value
            if i >= bytes.len() || (bytes[i] != b'"' && bytes[i] != b'\'') {
                if let Some(end) = find_end_of_decl(bytes, i) { i = end; } else { break; }
                continue;
            }
            let quote = bytes[i];
            i += 1;
            let val_start = i;
            while i < bytes.len() && bytes[i] != quote { i += 1; }
            let value = std::str::from_utf8(&bytes[val_start..i]).unwrap_or("").to_string();
            if i < bytes.len() { i += 1; } // consume closing quote
            // Skip to end of declaration
            if let Some(end) = find_end_of_decl(bytes, i) { i = end; }
            table.declare(name, value);
        } else {
            i += 1;
        }
    }
}

fn find_end_of_decl(bytes: &[u8], from: usize) -> Option<usize> {
    bytes[from..].iter().position(|&b| b == b'>').map(|p| from + p + 1)
}
