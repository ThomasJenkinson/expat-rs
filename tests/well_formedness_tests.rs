//! Well-formedness conformance tests. One assertion per W3C XML 1.0 §2.1
//! constraint or related rule.

use expat_rs::{Event, Parser, XmlError};

fn events(src: &str) -> Result<Vec<Event<'_>>, XmlError> {
    let mut p = Parser::new(src);
    let mut out = Vec::new();
    while let Some(e) = p.next_event()? {
        out.push(e);
    }
    Ok(out)
}

// ─── Happy paths ─────────────────────────────────────────────────────────────

#[test]
fn smallest_well_formed_doc() {
    let es = events("<root/>").unwrap();
    assert_eq!(es.len(), 2, "<x/> emits StartElement+EndElement, got {es:?}");
    assert!(matches!(&es[0], Event::StartElement { name, .. } if *name == "root"));
    assert!(matches!(&es[1], Event::EndElement(n) if *n == "root"));
}

#[test]
fn nested_elements_balanced() {
    let es = events("<a><b><c/></b></a>").unwrap();
    let opens: Vec<&str> = es.iter().filter_map(|e| match e {
        Event::StartElement { name, .. } => Some(*name), _ => None,
    }).collect();
    let closes: Vec<&str> = es.iter().filter_map(|e| match e {
        Event::EndElement(n) => Some(*n), _ => None,
    }).collect();
    assert_eq!(opens, vec!["a", "b", "c"]);
    assert_eq!(closes, vec!["c", "b", "a"]);
}

#[test]
fn xml_declaration_first_then_root() {
    let es = events(r#"<?xml version="1.0"?><r/>"#).unwrap();
    assert!(matches!(&es[0], Event::XmlDecl(_)));
    assert!(matches!(&es[1], Event::StartElement { name, .. } if *name == "r"));
}

#[test]
fn comments_in_prolog_and_epilog() {
    let es = events("<!-- pre --><r/><!-- post -->").unwrap();
    assert!(matches!(&es[0], Event::Comment(s) if *s == " pre "));
    assert!(matches!(&es[3], Event::Comment(s) if *s == " post "));
}

#[test]
fn whitespace_in_prolog_is_absorbed() {
    let es = events("   \n\n  <r/>").unwrap();
    assert_eq!(es.len(), 2);
    assert!(matches!(&es[0], Event::StartElement { .. }));
}

// ─── Well-formedness violations ──────────────────────────────────────────────
// Per W3C XML 1.0 §2.1 — every one of these documents is not well-formed.

#[test]
fn no_root_element() {
    assert!(matches!(
        events("<!-- only a comment -->").unwrap_err(),
        XmlError::NotWellFormed { .. }
    ));
}

#[test]
fn unclosed_root() {
    assert!(events("<root>").is_err(), "missing </root> must be rejected");
}

#[test]
fn mismatched_tags() {
    assert!(events("<a></b>").is_err(),
        "end tag </b> with start tag <a> must be rejected");
}

#[test]
fn improperly_nested() {
    assert!(events("<a><b></a></b>").is_err(),
        "improper nesting must be rejected");
}

#[test]
fn second_root_element_rejected() {
    assert!(events("<a/><b/>").is_err(),
        "two root elements must be rejected");
}

#[test]
fn text_outside_root_rejected() {
    assert!(events("hello<r/>").is_err(),
        "non-whitespace text before root must be rejected");
    assert!(events("<r/>hello").is_err(),
        "non-whitespace text after root must be rejected");
}

#[test]
fn duplicate_attribute_rejected() {
    assert!(events(r#"<r a="1" a="2"/>"#).is_err(),
        "duplicate attribute names must be rejected (§3.1 Unique Att Spec)");
}

#[test]
fn xmldecl_not_at_start_rejected() {
    assert!(events("<r/><?xml version=\"1.0\"?>").is_err(),
        "XML declaration must be first");
}

#[test]
fn duplicate_xmldecl_rejected() {
    let src = r#"<?xml version="1.0"?><?xml version="1.0"?><r/>"#;
    assert!(events(src).is_err(), "two XML declarations must be rejected");
}

#[test]
fn duplicate_doctype_rejected() {
    let src = "<!DOCTYPE a><!DOCTYPE b><a/>";
    assert!(events(src).is_err(), "two DOCTYPE declarations must be rejected");
}

#[test]
fn doctype_after_root_rejected() {
    assert!(events("<a/><!DOCTYPE x>").is_err(),
        "DOCTYPE must appear before the root element");
}

#[test]
fn end_tag_with_no_open_rejected() {
    assert!(events("</foo>").is_err(),
        "end tag without matching start must be rejected");
}

// ─── Built-in entities (week-2 minimal handling) ────────────────────────────

#[test]
fn builtin_entities_inside_root_are_accepted() {
    // We don't yet expand entities into Text bytes; we just confirm they're
    // accepted inside the root and rejected outside.
    let _ = events("<r>&amp;&lt;&gt;</r>").expect("built-in entities inside root must parse");
}

#[test]
fn entity_reference_outside_root_rejected() {
    assert!(events("&amp;<r/>").is_err(),
        "entity reference in prolog must be rejected");
}

// ─── Comments and PIs may appear in prolog AND epilog ───────────────────────

#[test]
fn pi_in_prolog_and_epilog() {
    let es = events("<?xml-stylesheet href=\"a.xsl\"?><r/><?gen done?>").unwrap();
    let pi_targets: Vec<&str> = es.iter().filter_map(|e| match e {
        Event::ProcessingInstruction { target, .. } => Some(*target),
        _ => None,
    }).collect();
    assert_eq!(pi_targets, vec!["xml-stylesheet", "gen"]);
}
