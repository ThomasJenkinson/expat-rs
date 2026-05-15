# Methodology — Clean-Room Implementation

This project is a clean-room implementation of XML 1.0. This document
exists so contributors and reviewers can check that.

## Reference materials consulted

The implementation is derived **only** from:

1. **[W3C XML 1.0 (Fifth Edition) Recommendation](https://www.w3.org/TR/xml/)**
   — the normative spec. This is the source of truth for what XML *means*.
2. **[Namespaces in XML 1.0 (Third Edition)](https://www.w3.org/TR/xml-names/)**
   — the namespace layer.
3. **[W3C XML Conformance Test Suite](https://www.w3.org/XML/Test/)**
   (`xmlts20020606.zip` and successors) — the conformance contract.
4. **[Annotated XML 1.0 specification](https://www.xml.com/axml/testaxml.htm)**
   by Tim Bray — the canonical commentary on the spec, used to disambiguate
   intent where the normative text is terse.

## Reference materials NOT consulted

- The libexpat source code is not read by anyone writing parser code.
  If you've read it before for some other reason, that's fine — just
  don't paste from memory. Reviewers will flag PRs that look like they
  came from libexpat.

## How libexpat is used (legitimately)

libexpat appears in this project's lifecycle in two narrow ways:

1. **As a behavioural reference for ambiguous spec edges.** When XML 1.0
   has multiple defensible interpretations of a corner case, we run the
   same input through libexpat to see what it does, and document our
   choice in the relevant test file. We do not look at libexpat's source
   to learn *how* it produced the answer.
2. **As a benchmark target.** Performance is measured against libexpat;
   any implementation choice motivated by perf parity is documented.

## Why

Two reasons we go clean-room rather than translating:

Legal: Even with both projects under MIT, downstream users in
regulated industries want a clean provenance trail. Clean-room
gives them that.

Engineering: A line-by-line translation of libexpat ends up looking
like C-with-Rust-syntax — same ownership patterns, same error model,
same API shape. None of those are good Rust. Working from the spec
gets us a codebase that uses the type system properly.

## Per-PR checklist

Reviewers verify:
- [ ] No code is copied from libexpat or any other XML library
- [ ] Spec citations are present for any non-obvious behavioural choice
- [ ] If libexpat was consulted as a behavioural reference, the test case
      documents what was checked and why our choice matches
- [ ] No `unsafe` blocks except in narrowly justified primitives (encoding
      conversion buffers, etc.) — each `unsafe` block has a `// SAFETY:`
      comment

## Audit trail

Every file in `src/` and `tests/` may cite the spec section it implements
or tests. For example:

```rust
/// Per XML 1.0 §2.3 [Production 4]: NameStartChar
fn is_name_start_char(c: char) -> bool { ... }
```

This makes the spec-to-code mapping verifiable.
