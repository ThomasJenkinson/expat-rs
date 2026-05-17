# expat-rs

[![CI](https://github.com/ThomasJenkinson/expat-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ThomasJenkinson/expat-rs/actions/workflows/ci.yml)

A Rust XML 1.0 (Fourth Edition) parser. Aims for the same conformance as
[libexpat](https://github.com/libexpat/libexpat) without C's
memory-safety bugs.

> **Status:** early development. Tokeniser, well-formedness checker, and
> entity expansion with billion-laughs / quadratic-blowup defences.
> On the W3C XML conformance suite: **94% acceptance on well-formed
> input**, ~82% pass rate on tests of features we've shipped. The full
> 1810-test suite scores 51.2% — the gap is unimplemented features
> (DTDs, namespaces, encoding detection). See `conformance/STATUS.md`.

## What this is

A from-scratch Rust implementation of an XML 1.0 parser, designed to:

1. **Pass the W3C XML Conformance Test Suite** (the same suite libexpat passes — 1,801 of 1,809 tests).
2. **Eliminate an entire class of memory-safety vulnerabilities.** libexpat has a long history of CVEs spanning integer overflows (e.g. CVE-2024-45491, -45492, -45493), denial-of-service via resource exhaustion (e.g. CVE-2024-8176, CVE-2023-52425), external entity issues (e.g. CVE-2024-28757, CVE-2013-0340), and other logic-level vulnerabilities (e.g. CVE-2018-20843). Rust eliminates the memory-safety subset by construction; the remaining classes require careful implementation regardless of language.
3. **Provide a drop-in replacement** for libexpat's C ABI, so existing consumers (CPython's `pyexpat`, Apache HTTPD's `mod_dav`, CPython's pyexpat, Apache HTTPD's mod_dav, D-Bus, fontconfig) can adopt it without recompiling.

## What this is not

- **Not a translation of libexpat.** See `METHODOLOGY.md` for the clean-room declaration. The implementation is built from the W3C XML 1.0 Fifth Edition specification, with no contributor with no code derived from the libexpat source.
- **Not a fork of an existing Rust XML parser.** `quick-xml`, `xml-rs`,`roxmltree` exist and are good, but none target full libexpat-compatible conformance (DTDs, entity expansion, namespace processing, encoding detection).

## Why

libexpat parses XML in CPython's stdlib (xml.parsers.expat), Apache HTTPD's mod_dav, D-Bus, fontconfig, CMake, and many embedded systems. Bugs in libexpat translate directly to RCE in all of them.
Replacing it with a memory-safe parser closes that path.

## Build & test

```sh
cargo build --release
cargo test --release
```

## Project layout

```
.
├── METHODOLOGY.md             # clean-room declaration
├── src/
│   ├── lib.rs                 # public API
│   ├── token.rs               # Token enum (every W3C production cited)
│   ├── lexer.rs               # tokeniser
│   ├── event.rs               # high-level Event enum (parser output)
│   ├── parser.rs              # well-formedness checker
│   ├── entities.rs            # DTD entity decls + bounded expansion
│   ├── error.rs               # XmlError + Position
│   └── bin/
│       └── xmlwf.rs           # CLI well-formedness checker
├── tests/
│   ├── tokeniser_tests.rs       # 21 tests, one per spec production
│   ├── well_formedness_tests.rs # 20 tests, one per §2.1 constraint
│   └── entity_security_tests.rs # 9 tests: billion-laughs, quadratic-blowup, …
└── conformance/
    ├── README.md              # how to run the W3C XML test suite
    ├── runner.sh              # iterates the suite, reports pass/fail
    └── STATUS.md              # current conformance numbers
```

## License

MIT — see `LICENSE`. Same as libexpat (so contributions can flow either way
if a consumer needs a feature in both).

## Roadmap

- [x] Tokeniser (21 tests, every W3C XML 1.0 production)
- [x] Well-formedness checker (20 tests — tag balance, root uniqueness,
      attribute uniqueness, prolog/epilog rules, built-in entities)
- [x] Full Unicode `NameStartChar` / `NameChar` per §2.3
- [x] Entity expansion with defensive limits — 9 security tests including
      billion-laughs and quadratic-blowup
- [x] W3C XML conformance runner — **94% accept on valid input**,
      ~82% pass rate on tests of shipped features
      (51.2% on the full 1810-test suite; gap is unimplemented features)
- [ ] Encoding detection (UTF-16 BOM, declared encodings) — target +5-7%
- [ ] Namespaces (W3C XML Namespaces 1.0) — target +15%
- [ ] DTDs and validity constraints — target +25-30%
- [ ] Full W3C conformance — match libexpat's 1801/1809
- [ ] `libexpat.so` ABI shim — drop-in replacement
