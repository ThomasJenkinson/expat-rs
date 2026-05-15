//! Tokeniser unit tests. One assertion per W3C XML 1.0 production rule.
//!
//! Each test cites the spec section it covers — see METHODOLOGY.md for the
//! audit-trail convention.

use expat_rs::{Lexer, Token, XmlDecl};

fn tokens(src: &str) -> Vec<Token<'_>> {
    let mut lex = Lexer::new(src);
    let mut out = Vec::new();
    loop {
        match lex.next_token().expect("tokenise") {
            Some(t) => out.push(t),
            None    => break,
        }
    }
    out
}

#[test]
fn empty_input_yields_no_tokens() {
    assert!(tokens("").is_empty());
}

// §2.4 [Production 14]: CharData
#[test]
fn plain_text_between_tags() {
    let ts = tokens("hello world");
    assert_eq!(ts.len(), 1);
    assert!(matches!(ts[0], Token::Text(s) if s == "hello world"));
}

// §3.1 [Production 40]: STag
#[test]
fn empty_start_tag() {
    let ts = tokens("<root>");
    assert_eq!(ts.len(), 1);
    if let Token::StartTag { name, attributes } = &ts[0] {
        assert_eq!(*name, "root");
        assert!(attributes.is_empty());
    } else { panic!("not a StartTag: {:?}", ts[0]); }
}

// §3.1 [Production 42]: ETag
#[test]
fn end_tag() {
    let ts = tokens("</root>");
    assert!(matches!(ts[0], Token::EndTag(n) if n == "root"));
}

// §3.1 [Production 44]: EmptyElemTag
#[test]
fn empty_element_tag() {
    let ts = tokens("<br/>");
    assert!(matches!(ts[0], Token::EmptyTag { name, .. } if name == "br"));
}

// §3.1 [Production 41]: Attribute
#[test]
fn start_tag_with_attributes() {
    let ts = tokens(r#"<a href="https://example.com" rel='nofollow'>"#);
    if let Token::StartTag { name, attributes } = &ts[0] {
        assert_eq!(*name, "a");
        assert_eq!(attributes.len(), 2);
        assert_eq!(attributes[0].name, "href");
        assert_eq!(attributes[0].value, "https://example.com");
        assert_eq!(attributes[1].name, "rel");
        assert_eq!(attributes[1].value, "nofollow");
    } else { panic!("not a StartTag"); }
}

// §2.5 [Production 15]: Comment
#[test]
fn comment_basic() {
    let ts = tokens("<!-- hello -->");
    assert!(matches!(ts[0], Token::Comment(s) if s == " hello "));
}

#[test]
fn comment_double_dash_is_error() {
    let mut lex = Lexer::new("<!-- a -- b -->");
    assert!(lex.next_token().is_err(), "'--' inside comment must be rejected");
}

// §2.7 [Production 18]: CDSect
#[test]
fn cdata_section() {
    let ts = tokens("<![CDATA[<b>literal</b>]]>");
    assert!(matches!(ts[0], Token::CData(s) if s == "<b>literal</b>"));
}

// §2.6 [Production 16]: PI
#[test]
fn processing_instruction() {
    let ts = tokens("<?xml-stylesheet href=\"foo.xsl\"?>");
    if let Token::ProcessingInstruction { target, body } = &ts[0] {
        assert_eq!(*target, "xml-stylesheet");
        assert_eq!(*body, "href=\"foo.xsl\"");
    } else { panic!("not a PI: {:?}", ts[0]); }
}

// §2.8 [Production 23]: XMLDecl
#[test]
fn xml_declaration_minimal() {
    let ts = tokens(r#"<?xml version="1.0"?>"#);
    let want = XmlDecl { version: "1.0", encoding: None, standalone: None };
    assert!(matches!(&ts[0], Token::XmlDecl(d) if d == &want));
}

#[test]
fn xml_declaration_with_encoding_and_standalone() {
    let ts = tokens(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    if let Token::XmlDecl(d) = &ts[0] {
        assert_eq!(d.version, "1.0");
        assert_eq!(d.encoding, Some("UTF-8"));
        assert_eq!(d.standalone, Some(true));
    } else { panic!("not an XmlDecl"); }
}

// §2.8 [Production 28]: doctypedecl
#[test]
fn doctype_simple() {
    let ts = tokens("<!DOCTYPE html>");
    assert!(matches!(&ts[0], Token::Doctype { name, body } if *name == "html" && body.is_empty()));
}

#[test]
fn doctype_with_internal_subset() {
    let ts = tokens("<!DOCTYPE root [ <!ELEMENT root (#PCDATA)> ]>");
    if let Token::Doctype { name, body } = &ts[0] {
        assert_eq!(*name, "root");
        assert!(body.contains("<!ELEMENT root"));
    } else { panic!("not a Doctype"); }
}

// §4.1 [Productions 66–68]: References
#[test]
fn entity_reference() {
    let ts = tokens("&amp;");
    assert!(matches!(ts[0], Token::EntityRef(n) if n == "amp"));
}

#[test]
fn decimal_character_reference() {
    let ts = tokens("&#65;");
    assert!(matches!(ts[0], Token::CharRef('A')));
}

#[test]
fn hex_character_reference() {
    let ts = tokens("&#x41;");
    assert!(matches!(ts[0], Token::CharRef('A')));
}

#[test]
fn invalid_character_reference_rejected() {
    let mut lex = Lexer::new("&#xZZZ;");
    assert!(lex.next_token().is_err());
}

// Multi-token document
#[test]
fn full_small_document() {
    let src = r#"<?xml version="1.0"?>
<root>
  <child attr="v">text</child>
</root>"#;
    let ts = tokens(src);
    // 1 XmlDecl, then text (newline+spaces), StartTag root,
    // then text, StartTag child, Text, EndTag, etc.
    assert!(matches!(&ts[0], Token::XmlDecl(_)));
    let names: Vec<&str> = ts.iter().filter_map(|t| match t {
        Token::StartTag { name, .. } => Some(*name),
        _ => None,
    }).collect();
    assert_eq!(names, vec!["root", "child"]);
    let ends: Vec<&str> = ts.iter().filter_map(|t| match t {
        Token::EndTag(n) => Some(*n),
        _ => None,
    }).collect();
    assert_eq!(ends, vec!["child", "root"]);
}

// Per §2.4: `]]>` MUST NOT occur in character data
#[test]
fn cdend_in_text_is_rejected() {
    let mut lex = Lexer::new("oops]]>");
    assert!(lex.next_token().is_err());
}

// Per §3.1: '<' not allowed in attribute value
#[test]
fn lt_in_attribute_value_is_rejected() {
    let mut lex = Lexer::new("<a href=\"<bad>\">");
    assert!(lex.next_token().is_err(), "'<' inside attribute value must be rejected");
}
