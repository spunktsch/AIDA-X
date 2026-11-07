#!/usr/bin/env bash
# Bundle AIDA-X (CLAP + VST3) and run clap-validator and pluginval on the bundles (TICKET-021).
#
#   1. cargo xtask bundle aidadx_plugin --release -> target/bundled/AIDA-X.clap, AIDA-X.vst3
#   2. Downloads pinned clap-validator / pluginval releases into target/tools/<tool>-<version>/
#      if they are not there yet (nothing is installed system-wide).
#   3. clap-validator validate target/bundled/AIDA-X.clap
#      pluginval --strictness-level 5 --validate target/bundled/AIDA-X.vst3
#   Exits non-zero if any step fails.
#
# Usage: scripts/validate-plugin.sh [--skip-bundle] [--strictness N]
#
# Linux, macOS and Windows (Git Bash/MSYS) are supported. Needs curl, unzip and tar.
# On headless Linux pluginval may need a display: run under `xvfb-run -a`.
#
# The versions and download URLs below are UNVERIFIED (written from memory, never
# downloaded). Check them against the GitHub release pages before the first run and fill in
# the SHA-256 constants so later downloads are verified.

set -euo pipefail

# --- Pinned tool versions (adjust here) ---------------------------------------------------
# https://github.com/free-audio/clap-validator/releases   (tags have no "v" prefix)
CLAP_VALIDATOR_VERSION="0.3.2"
CLAP_VALIDATOR_BASE_URL="https://github.com/free-audio/clap-validator/releases/download/${CLAP_VALIDATOR_VERSION}"
CLAP_VALIDATOR_ASSET_LINUX="clap-validator-${CLAP_VALIDATOR_VERSION}-ubuntu-18.04.tar.gz"
CLAP_VALIDATOR_ASSET_MACOS="clap-validator-${CLAP_VALIDATOR_VERSION}-macos-universal.tar.gz"
CLAP_VALIDATOR_ASSET_WINDOWS="clap-validator-${CLAP_VALIDATOR_VERSION}-windows.zip"
CLAP_VALIDATOR_SHA256=""   # optional: fill in (for this platform's asset) to verify the download
# https://github.com/Tracktion/pluginval/releases          (tags have a "v" prefix)
PLUGINVAL_VERSION="1.0.4"
PLUGINVAL_BASE_URL="https://github.com/Tracktion/pluginval/releases/download/v${PLUGINVAL_VERSION}"
PLUGINVAL_ASSET_LINUX="pluginval_Linux.zip"
PLUGINVAL_ASSET_MACOS="pluginval_macOS.zip"
PLUGINVAL_ASSET_WINDOWS="pluginval_Windows.zip"
PLUGINVAL_SHA256=""        # optional: fill in (for this platform's asset) to verify the download
# ------------------------------------------------------------------------------------------

SKIP_BUNDLE=0
STRICTNESS=5
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-bundle) SKIP_BUNDLE=1 ;;
        --strictness) STRICTNESS="$2"; shift ;;
        -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE_DIR="$REPO_ROOT/target/bundled"
TOOLS_DIR="$REPO_ROOT/target/tools"
CLAP_BUNDLE="$BUNDLE_DIR/AIDA-X.clap"
VST3_BUNDLE="$BUNDLE_DIR/AIDA-X.vst3"

case "$(uname -s)" in
    Linux*)
        CLAP_VALIDATOR_ASSET="$CLAP_VALIDATOR_ASSET_LINUX"
        PLUGINVAL_ASSET="$PLUGINVAL_ASSET_LINUX"
        CLAP_VALIDATOR_EXE="clap-validator"
        PLUGINVAL_EXE="pluginval" ;;
    Darwin*)
        CLAP_VALIDATOR_ASSET="$CLAP_VALIDATOR_ASSET_MACOS"
        PLUGINVAL_ASSET="$PLUGINVAL_ASSET_MACOS"
        CLAP_VALIDATOR_EXE="clap-validator"
        PLUGINVAL_EXE="pluginval" ;;   # inside pluginval.app/Contents/MacOS/
    MINGW*|MSYS*|CYGWIN*)
        CLAP_VALIDATOR_ASSET="$CLAP_VALIDATOR_ASSET_WINDOWS"
        PLUGINVAL_ASSET="$PLUGINVAL_ASSET_WINDOWS"
        CLAP_VALIDATOR_EXE="clap-validator.exe"
        PLUGINVAL_EXE="pluginval.exe" ;;
    *) echo "Unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
    else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# get_tool <name> <version> <url> <sha256> <exe-name>: download + extract into
# target/tools/<name>-<version>/ unless already present; prints the executable's path.
get_tool() {
    local name="$1" version="$2" url="$3" expected="$4" exe_name="$5"
    local dir="$TOOLS_DIR/$name-$version"
    local exe
    exe="$(find "$dir" -type f -name "$exe_name" 2>/dev/null | head -n 1 || true)"
    if [[ -z "$exe" ]]; then
        echo "==> Downloading $name $version" >&2
        echo "    $url" >&2
        mkdir -p "$dir"
        local archive="$dir/${url##*/}"
        curl -fL --retry 3 -o "$archive" "$url"
        if [[ -n "$expected" ]]; then
            local actual
            actual="$(sha256 "$archive")"
            if [[ "$actual" != "$expected" ]]; then
                rm -f "$archive"
                echo "$name download hash mismatch: expected $expected, got $actual" >&2
                exit 1
            fi
        else
            echo "WARNING: $name download not hash-verified (set the SHA-256 constant in $0)" >&2
        fi
        case "$archive" in
            *.zip) unzip -q -o "$archive" -d "$dir" ;;
            *.tar.gz|*.tgz) tar -xzf "$archive" -C "$dir" ;;
            *) echo "Unknown archive type: $archive" >&2; exit 1 ;;
        esac
        rm -f "$archive"
        exe="$(find "$dir" -type f -name "$exe_name" | head -n 1 || true)"
        if [[ -z "$exe" ]]; then
            echo "$exe_name not found in $url" >&2
            exit 1
        fi
        chmod +x "$exe"
    fi
    echo "$exe"
}

cd "$REPO_ROOT"

if [[ "$SKIP_BUNDLE" -eq 0 ]]; then
    echo "==> Bundle plugin"
    cargo xtask bundle aidadx_plugin --release
fi
[[ -e "$CLAP_BUNDLE" ]] || { echo "Missing $CLAP_BUNDLE" >&2; exit 1; }
[[ -e "$VST3_BUNDLE" ]] || { echo "Missing $VST3_BUNDLE" >&2; exit 1; }

CLAP_VALIDATOR="$(get_tool clap-validator "$CLAP_VALIDATOR_VERSION" \
    "$CLAP_VALIDATOR_BASE_URL/$CLAP_VALIDATOR_ASSET" "$CLAP_VALIDATOR_SHA256" "$CLAP_VALIDATOR_EXE")"
PLUGINVAL="$(get_tool pluginval "$PLUGINVAL_VERSION" \
    "$PLUGINVAL_BASE_URL/$PLUGINVAL_ASSET" "$PLUGINVAL_SHA256" "$PLUGINVAL_EXE")"

failed=0
echo "==> clap-validator"
echo "    $CLAP_VALIDATOR validate $CLAP_BUNDLE"
"$CLAP_VALIDATOR" validate "$CLAP_BUNDLE" || { echo "clap-validator failed (exit $?)" >&2; failed=1; }

echo "==> pluginval (strictness $STRICTNESS)"
echo "    $PLUGINVAL --strictness-level $STRICTNESS --validate $VST3_BUNDLE"
"$PLUGINVAL" --strictness-level "$STRICTNESS" --validate "$VST3_BUNDLE" \
    || { echo "pluginval failed (exit $?)" >&2; failed=1; }

if [[ "$failed" -ne 0 ]]; then
    echo "Validation failed" >&2
    exit 1
fi
echo "==> All validators passed"
