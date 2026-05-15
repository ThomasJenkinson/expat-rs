//! Entity-expansion security tests.
//!
//! These exercise the defences against the **billion-laughs** and
//! **quadratic-blowup** vulnerability classes that have repeatedly affected
//! libexpat (and many other XML parsers). Each test sends an adversarial
//! payload and asserts the parser rejects it without runaway memory use.

use expat_rs::{ExpansionLimits, Parser, XmlError};

fn drain(p: &mut Parser<'_>) -> Result<(), XmlError> {
    while p.next_event()?.is_some() {}
    Ok(())
}

/// The classic billion-laughs payload: 10 levels of 10× expansion = 10^10.
/// A naïve parser allocates ~10 GB; we must reject before that happens.
#[test]
fn billion_laughs_classic_rejected() {
    let payload = r#"<?xml version="1.0"?>
<!DOCTYPE lolz [
  <!ENTITY lol  "lol">
  <!ENTITY lol2 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">
  <!ENTITY lol3 "&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;&lol2;">
  <!ENTITY lol4 "&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;&lol3;">
  <!ENTITY lol5 "&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;&lol4;">
  <!ENTITY lol6 "&lol5;&lol5;&lol5;&lol5;&lol5;&lol5;&lol5;&lol5;&lol5;&lol5;">
  <!ENTITY lol7 "&lol6;&lol6;&lol6;&lol6;&lol6;&lol6;&lol6;&lol6;&lol6;&lol6;">
  <!ENTITY lol8 "&lol7;&lol7;&lol7;&lol7;&lol7;&lol7;&lol7;&lol7;&lol7;&lol7;">
  <!ENTITY lol9 "&lol8;&lol8;&lol8;&lol8;&lol8;&lol8;&lol8;&lol8;&lol8;&lol8;">
]>
<lolz>&lol9;</lolz>"#;
    let mut p = Parser::new(payload);
    let err = drain(&mut p).expect_err("billion-laughs payload must be rejected");
    let msg = format!("{err}");
    assert!(msg.contains("billion-laughs") || msg.contains("budget") || msg.contains("depth"),
        "error message should explain why ({msg:?})");
}

/// Quadratic blowup: linear-size payload (no exponential nesting) but the
/// parser would do quadratic work. Often used to bypass naïve depth-only caps.
#[test]
fn quadratic_blowup_rejected() {
    let mut payload = String::from(r#"<?xml version="1.0"?>
<!DOCTYPE bomb [
  <!ENTITY a "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">
]>
<bomb>"#);
    // 100,000 references to &a; — 10^5 × 100 chars = 10 MB total expansion
    for _ in 0..100_000 { payload.push_str("&a;"); }
    payload.push_str("</bomb>");

    let mut p = Parser::new(&payload);
    let err = drain(&mut p).expect_err("quadratic-blowup payload must be rejected");
    // The error happens on one of the first ~few thousand &a; references —
    // the budget is exhausted long before all 100k are processed.
    let _ = err;
}

/// Self-referential entity → infinite recursion if depth check missing.
#[test]
fn self_referential_entity_rejected() {
    let payload = r#"<?xml version="1.0"?>
<!DOCTYPE x [
  <!ENTITY a "&a;">
]>
<x>&a;</x>"#;
    let mut p = Parser::new(payload);
    assert!(drain(&mut p).is_err(), "self-referential entity must be rejected");
}

/// Two entities referencing each other → mutual recursion.
#[test]
fn mutual_recursion_rejected() {
    let payload = r#"<?xml version="1.0"?>
<!DOCTYPE x [
  <!ENTITY a "&b;">
  <!ENTITY b "&a;">
]>
<x>&a;</x>"#;
    let mut p = Parser::new(payload);
    assert!(drain(&mut p).is_err(), "mutually-recursive entities must be rejected");
}

/// Sane DTD-defined entity must work.
#[test]
fn declared_entity_accepted() {
    let payload = r#"<?xml version="1.0"?>
<!DOCTYPE x [
  <!ENTITY greeting "Hello, world">
]>
<x>&greeting;</x>"#;
    let mut p = Parser::new(payload);
    drain(&mut p).expect("simple declared entity must parse cleanly");
}

/// Sane recursive (but bounded) entities must work.
#[test]
fn shallow_recursion_accepted() {
    let payload = r#"<?xml version="1.0"?>
<!DOCTYPE x [
  <!ENTITY base "x">
  <!ENTITY one  "&base;&base;">
  <!ENTITY two  "&one;&one;">
]>
<x>&two;</x>"#;
    let mut p = Parser::new(payload);
    drain(&mut p).expect("shallow recursion within limits must parse");
}

/// Caller can tighten the limits.
#[test]
fn custom_tighter_limits_apply() {
    let payload = r#"<?xml version="1.0"?>
<!DOCTYPE x [
  <!ENTITY a "abcdefghij">
  <!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">
]>
<x>&b;</x>"#;
    // Default limits would accept this (100 bytes total). Tighten to 50.
    let mut p = Parser::new(payload).with_expansion_limits(ExpansionLimits {
        max_depth: 20,
        max_expanded_bytes: 50,
    });
    assert!(drain(&mut p).is_err(), "tighter byte budget must reject");
}

/// Undeclared entity reference must be rejected.
#[test]
fn undeclared_entity_rejected() {
    let payload = "<doc>&missing;</doc>";
    let mut p = Parser::new(payload);
    assert!(drain(&mut p).is_err(), "undeclared entity reference must be rejected");
}

/// Built-in entities never need a DTD.
#[test]
fn builtin_entities_without_dtd() {
    let payload = "<doc>&amp;&lt;&gt;&quot;&apos;</doc>";
    let mut p = Parser::new(payload);
    drain(&mut p).expect("built-in entities don't need DTD");
}
