# Conformance Status

Numbers updated as the parser progresses.

## Latest run

| Category | Pass | Total | Rate |
|---|---:|---:|---:|
| Well-formed inputs accepted | **533** | 567 | **94.0%** |
| Not-well-formed inputs rejected | **394** | 1243 | **31.7%** |
| **In-scope tests (features we've shipped)** | **~927** | **~1130** | **~82%** |
| Full suite (limited by unimplemented features) | 927 | 1810 | 51.2% |
| _libexpat reference (full feature parity)_ | _1801_ | _1809_ | _99.6%_ |

The **in-scope** number is the one to read. Of the 1243 not-well-formed
tests in the suite, roughly 680 exercise features we haven't built yet
(DTD validation, namespace constraints, encoding detection,
external-DTD entity loading). Excluding those gives a denominator of
~1130 — the tests that should pass given what we've shipped.

The full-suite 51.2% is the progression metric — it climbs as we
implement more features. We track it for that reason, not as a
correctness claim.

> The 557 → 533 dip happened because we now reject undeclared entities.
> ~24 of the W3C tests use entities defined in external DTDs we don't
> load — they fail accordingly. We could add a permissive mode to
> accept those if needed.

Run with: `./runner.sh /tmp/xmlconf-w3c/xmlconf` (after downloading
`xmlts20130923.zip` from W3C).

## What's in the "out of scope" bucket

The 680 tests we currently miss break down roughly as:

| Category | Count (estimated) | Feature that would cover it |
|---|---:|---|
| DTD validation | ~400 | DTD declarations + validity constraints |
| Namespace constraints | ~150 | Namespace processing |
| Encoding detection | ~80 | UTF-16 BOM, declared encodings |
| Entity-related well-formedness in external subsets | ~50 | External-DTD loading |
| Edge cases | ~the rest | Iterative |

These counts come from a manual scan of failing tests and are
approximate. We'll replace them with exact numbers once the runner
categorises each test (planned).

## Roadmap

- [x] Tokeniser
- [x] Well-formedness checker
- [x] Full Unicode `NameStartChar` / `NameChar` per §2.3
- [x] Defensive entity expansion (billion-laughs / quadratic-blowup mitigation)
- [ ] Encoding detection (UTF-16 BOM, declared encodings)
- [ ] Namespaces (W3C XML Namespaces 1.0)
- [ ] DTD declarations
- [ ] Validity constraints — target match libexpat's 1801/1809
- [ ] `libexpat.so` ABI shim — Python `pyexpat` works unmodified
- [ ] Per-test categorisation in the runner (parse `xmlconf.xml` to make the in-scope number exact rather than approximate)
