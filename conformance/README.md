# W3C XML Conformance Suite Runner

The [W3C XML Conformance Test Suite](https://www.w3.org/XML/Test/) run
against `expat-rs`. libexpat scores 1801 / 1809 on the same suite.

## One-time setup

Download the suite from W3C and unpack it here:

```sh
curl -O https://www.w3.org/XML/Test/xmlts20020606.zip
unzip xmlts20020606.zip            # produces xmlconf/ here
```

(The suite is not vendored in this repo — it's external W3C content.)

## Running

Once an `xmlwf` binary is built (`cargo build --release --bin xmlwf`),
run the conformance pass:

```sh
./runner.sh
```

Output: `Passed: NNN`, `Failed: MM`. Each failure is also written to
`out/<test-id>.diff` for inspection.

## Current status

Numbers live in `STATUS.md`. Last run: 94% accept on valid input,
~82% on tests of shipped features (51.2% on the full 1810-test suite).
