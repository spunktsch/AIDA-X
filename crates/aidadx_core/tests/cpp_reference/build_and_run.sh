#!/bin/sh
# Regenerate tests/fixtures/biquad_golden.{bin,idx} from the original C++ code (TICKET-022).
# Needs a C++17 compiler ($CXX, default c++). Usage: crates/aidadx_core/tests/cpp_reference/build_and_run.sh
# DPF headers: $DPF_DIR if set, else <repo>/modules/dpf (git submodule update --init modules/dpf).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../../.." && pwd)
DPF_DIR=${DPF_DIR:-$REPO/modules/dpf}
CXX=${CXX:-c++}
if [ ! -f "$DPF_DIR/distrho/extra/ValueSmoother.hpp" ]; then
    echo "DPF not found at $DPF_DIR - run 'git submodule update --init modules/dpf' or set DPF_DIR" >&2
    exit 1
fi
BUILD=${TMPDIR:-/tmp}/aidadx_biquad_ref
mkdir -p "$BUILD"
# -ffp-contract=off: no FMA contraction, matching MSVC /fp:precise and rustc.
"$CXX" -std=c++17 -O2 -ffp-contract=off \
    -I"$REPO/src" -I"$DPF_DIR/distrho" \
    "$HERE/gen_biquad_ref.cpp" -o "$BUILD/gen_biquad_ref"
"$BUILD/gen_biquad_ref" "$HERE/../fixtures"
