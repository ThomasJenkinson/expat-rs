#!/usr/bin/env bash
# W3C XML Conformance Suite runner for expat-rs.
#
# Modelled on libexpat's xmltest.sh. Iterates the well-formed and
# not-well-formed test categories libexpat targets:
#
#   well-formed:     ibm/valid/P*, ibm/invalid/P*, xmltest/valid/{ext-sa,not-sa,sa},
#                    xmltest/invalid, xmltest/invalid/not-sa, sun/valid, sun/invalid,
#                    oasis (*pass*.xml)
#   not well-formed: ibm/not-wf/P*, ibm/not-wf/p28a, ibm/not-wf/misc,
#                    xmltest/not-wf/{ext-sa,not-sa,sa}, sun/not-wf,
#                    oasis (*fail*.xml)
#
# Usage:
#   ./runner.sh <path-to-xmlconf-root> [<xmlwf-binary>]
# e.g.
#   ./runner.sh /tmp/xmlconf-w3c/xmlconf ../target/release/xmlwf

set -e

XMLCONF="${1:-/tmp/xmlconf-w3c/xmlconf}"
XMLWF="${2:-$(dirname "$0")/../target/release/xmlwf}"

if [ ! -d "$XMLCONF" ]; then
    echo "ERROR: xmlconf directory not found: $XMLCONF" >&2
    echo "Download with: curl -O https://www.w3.org/XML/Test/xmlts20130923.zip && unzip" >&2
    exit 2
fi
if [ ! -x "$XMLWF" ]; then
    echo "ERROR: xmlwf binary not found or not executable: $XMLWF" >&2
    echo "Build with: cargo build --release --bin xmlwf" >&2
    exit 2
fi

# Counters
WF_PASS=0; WF_FAIL=0
NOT_WF_PASS=0; NOT_WF_FAIL=0
declare -a WF_FAILURES NOT_WF_FAILURES

# Run xmlwf, expecting it to succeed.
run_wf() {
    local file="$1"
    if "$XMLWF" "$file" > /dev/null 2>&1; then
        WF_PASS=$((WF_PASS+1))
    else
        WF_FAIL=$((WF_FAIL+1))
        WF_FAILURES+=("$file")
    fi
}

# Run xmlwf, expecting it to fail.
run_not_wf() {
    local file="$1"
    if "$XMLWF" "$file" > /dev/null 2>&1; then
        NOT_WF_FAIL=$((NOT_WF_FAIL+1))
        NOT_WF_FAILURES+=("$file")
    else
        NOT_WF_PASS=$((NOT_WF_PASS+1))
    fi
}

###################
# Well-formed set #
###################

# ibm/valid (P02, P03, ...) and ibm/invalid (still well-formed)
for d in "$XMLCONF/ibm/valid"/P* "$XMLCONF/ibm/invalid"/P*; do
    [ -d "$d" ] || continue
    for f in "$d"/*.xml; do
        [ -f "$f" ] && run_wf "$f"
    done
done

# xmltest valid
for d in "$XMLCONF/xmltest/valid/sa" \
         "$XMLCONF/xmltest/valid/not-sa" \
         "$XMLCONF/xmltest/valid/ext-sa" \
         "$XMLCONF/xmltest/invalid" \
         "$XMLCONF/xmltest/invalid/not-sa"; do
    [ -d "$d" ] || continue
    for f in "$d"/*.xml; do
        [ -f "$f" ] && run_wf "$f"
    done
done

# sun valid + invalid (still well-formed)
for d in "$XMLCONF/sun/valid" "$XMLCONF/sun/invalid"; do
    [ -d "$d" ] || continue
    for f in "$d"/*.xml; do
        [ -f "$f" ] && run_wf "$f"
    done
done

# oasis pass*
if [ -d "$XMLCONF/oasis" ]; then
    for f in "$XMLCONF/oasis"/*pass*.xml; do
        [ -f "$f" ] && run_wf "$f"
    done
fi

#######################
# Not-well-formed set #
#######################

# ibm not-wf
for d in "$XMLCONF/ibm/not-wf"/P* \
         "$XMLCONF/ibm/not-wf/p28a" \
         "$XMLCONF/ibm/not-wf/misc"; do
    [ -d "$d" ] || continue
    for f in "$d"/*.xml; do
        [ -f "$f" ] && run_not_wf "$f"
    done
done

# xmltest not-wf
for d in "$XMLCONF/xmltest/not-wf/sa" \
         "$XMLCONF/xmltest/not-wf/not-sa" \
         "$XMLCONF/xmltest/not-wf/ext-sa"; do
    [ -d "$d" ] || continue
    for f in "$d"/*.xml; do
        [ -f "$f" ] && run_not_wf "$f"
    done
done

# sun not-wf
if [ -d "$XMLCONF/sun/not-wf" ]; then
    for f in "$XMLCONF/sun/not-wf"/*.xml; do
        [ -f "$f" ] && run_not_wf "$f"
    done
fi

# oasis fail*
if [ -d "$XMLCONF/oasis" ]; then
    for f in "$XMLCONF/oasis"/*fail*.xml; do
        [ -f "$f" ] && run_not_wf "$f"
    done
fi

##########
# Report #
##########

WF_TOTAL=$((WF_PASS + WF_FAIL))
NOT_WF_TOTAL=$((NOT_WF_PASS + NOT_WF_FAIL))
TOTAL=$((WF_TOTAL + NOT_WF_TOTAL))
PASS_TOTAL=$((WF_PASS + NOT_WF_PASS))

cat <<EOF
=== W3C XML Conformance — expat-rs ===
Well-formed cases:    $WF_PASS / $WF_TOTAL accepted
Not-well-formed:      $NOT_WF_PASS / $NOT_WF_TOTAL correctly rejected
TOTAL:                $PASS_TOTAL / $TOTAL  ($(awk "BEGIN{printf \"%.1f\", 100*$PASS_TOTAL/$TOTAL}")%)

(libexpat reference: 1801 / 1809  =  99.6%)
EOF

# If verbose mode requested, print failure lists
if [ "${VERBOSE:-0}" = "1" ]; then
    if [ "$WF_FAIL" -gt 0 ]; then
        echo
        echo "Well-formed cases incorrectly REJECTED ($WF_FAIL):"
        printf '  %s\n' "${WF_FAILURES[@]}"
    fi
    if [ "$NOT_WF_FAIL" -gt 0 ]; then
        echo
        echo "Not-well-formed cases incorrectly ACCEPTED ($NOT_WF_FAIL):"
        printf '  %s\n' "${NOT_WF_FAILURES[@]}"
    fi
fi
