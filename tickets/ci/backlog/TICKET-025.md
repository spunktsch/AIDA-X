---
id: TICKET-025
status: backlog
assignee: sesch
priority: medium
created: 2026-11-07
updated: 2026-11-07
---

# macOS universal plugin bundle (bundle-universal in xtask)

## Description
`.github/workflows/rust.yml` currently builds two separate macOS bundles (`macos-arm64` native, `macos-x64` cross-compiled on the arm64 runner). Users expect one universal `AIDA-X.clap` / `AIDA-X.vst3` for macOS, as the C++ build ships (`macos-universal-10.15` in `build.yml`). The `xtask` wrapper (`xtask/src/main.rs`) only implements `bundle`; nih_plug_xtask's `bundle-universal` (lipo of x86_64 + aarch64) is missing.

## Acceptance Criteria
- [ ] `cargo xtask bundle-universal aidadx_plugin --release` builds `x86_64-apple-darwin` and `aarch64-apple-darwin` and produces universal `target/bundled/AIDA-X.clap` and `AIDA-X.vst3` (`lipo -info` lists both archs)
- [ ] Works with the C++ RTNeural build in `aidadx_rtneural` (cc crate compiles for both targets; set `MACOSX_DEPLOYMENT_TARGET`, matching the C++ build's 10.15 if feasible)
- [ ] Wrapper keeps the fix that the workspace root is the xtask parent dir (no outer-workspace detection)
- [ ] `rust.yml`: replace the two macOS matrix entries with one `macos-universal` job (tests still run natively on arm64), artifact `AIDA-X-macos-universal.zip` + `.sha256`
- [ ] Bundle loads in at least one host on Apple Silicon and on Intel (or Rosetta) — documented

## Implementation Notes
- `nih_plug_xtask` (same pinned rev as `aidadx_plugin`) exposes the universal path; check its public API (`build`/`bundle` with `bundle_universal`-style helpers or `main_with_args`) and wrap it like `bundle` in `xtask/src/main.rs`.
- Cross-compiling the C++ shim for x86_64 on an arm64 runner was never tested (risk noted in TICKET-013).
- Codesigning/notarization stays out of scope (TICKET-013, needs secrets).
- Related: TICKET-013 (CI), TICKET-021 (bundling/validation).

## Checklist
- [ ] Code implemented
- [ ] Tests written
- [ ] Self-review passed
- [ ] PR created
- [ ] Merged
