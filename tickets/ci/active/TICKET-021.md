---
id: TICKET-021
status: active
assignee:
priority: medium
created: 2026-11-07
updated: 2026-11-07
---

# Validate plugin with clap-validator and pluginval

## Description
The plugin has never been loaded by a host or validator.

## Acceptance Criteria
- [ ] clap-validator run on the CLAP build, all failures fixed or documented
      (script ready, awaiting approval to download/run validators)
- [ ] pluginval (strictness 5+) on the VST3 bundle
      (script ready, awaiting approval to download/run validators)
- [x] Bundle layout (`.vst3` folder, `.clap`) produced by a script or xtask
- [ ] Smoke test in at least one DAW documented

## Implementation Notes
Depends on TICKET-017/019. Feeds TICKET-013 (CI).

### Bundling (done, verified on Windows/MSVC)
- `cargo xtask bundle aidadx_plugin --release` (alias in `.cargo/config.toml`, crate
  `xtask/` on `nih_plug_xtask` from the same nih-plug rev `de42101` as `aidadx_plugin`;
  bundle name `AIDA-X` from `bundler.toml`). Output in `target/bundled/`:
  - `AIDA-X.clap` (the cdylib)
  - `AIDA-X.vst3/Contents/x86_64-win/AIDA-X.vst3`
  - Linux/macOS layouts (`Contents/x86_64-linux/AIDA-X.so`, macOS `.vst3`/`.clap` bundles
    with `Info.plist`) are produced by `nih_plug_xtask`; not verified here.
- `dumpbin /exports` on both bundled DLLs: `clap_entry`, `GetPluginFactory`, `InitDll`,
  `ExitDll`.
- `xtask/src/main.rs` calls `nih_plug_xtask::{build, bundle}` itself instead of
  `nih_plug_xtask::main()`: upstream picks the *outermost* ancestor directory with a
  `Cargo.toml` as the workspace, which silently builds/bundles the wrong project when the
  checkout is nested in another Cargo project (e.g. agent worktrees under the main checkout).

### Validator scripts (written, NOT run yet)
`scripts/validate-plugin.ps1` (Windows PowerShell 5.1+) and `scripts/validate-plugin.sh`
(Linux/macOS/Git Bash) bundle via xtask, download pinned tools into
`target/tools/<tool>-<version>/` if missing, then run
`clap-validator validate target/bundled/AIDA-X.clap` and
`pluginval --strictness-level 5 --validate target/bundled/AIDA-X.vst3`, failing on non-zero
exit. **The versions and URLs are unverified** (written from memory, nothing downloaded):

| Tool | Version | URL pattern |
|------|---------|-------------|
| clap-validator | 0.3.2 | `https://github.com/free-audio/clap-validator/releases/download/0.3.2/clap-validator-0.3.2-{windows.zip,ubuntu-18.04.tar.gz,macos-universal.tar.gz}` |
| pluginval | 1.0.4 | `https://github.com/Tracktion/pluginval/releases/download/v1.0.4/pluginval_{Windows,Linux,macOS}.zip` |

Check them against the release pages and fill in the optional SHA-256 constants at the top
of both scripts before the first run. Headless Linux likely needs `xvfb-run -a` for pluginval.

### Self-review against common validator checks
`crates/aidadx_plugin/tests/host_behaviour.rs` drives the exported `clap_entry` through the
raw CLAP ABI (clap-sys at nih-plug's pinned rev, dev-dependency only), i.e. through
nih-plug's real wrapper:
- factory/descriptor, unknown plugin ID rejected; CLAP param IDs = DPF indices
- param value -> text -> value -> text stable for every parameter (min/max/default/interior
  values, all steps of stepped params), garbage text rejected, unknown IDs rejected
- state: save -> load into fresh instance -> save is byte-identical (also with 7-byte and
  1-byte stream reads); empty, truncated, non-JSON, wrong-shape JSON and short-length-prefix
  state rejected without changing state; odd-but-valid JSON keeps all values in range
- activate at 22.05k..192k with max block 1..8192, process 0/1/half/max frames, reset,
  deactivate/reactivate; output buffers pre-filled with NaN must be fully overwritten
- param fuzz (random + extreme values every block, noise input) at 22.05k/44.1k/48k/96k
- stereo audio-ports config (2nd channel mirrors 1st), invalid config ID rejected
- multiple instances concurrently, created/destroyed in different orders and threads
- engine directly: empty blocks and blocks longer than the prepared maximum

Bug fixed: ANTIALIASING displayed `Off` at 0 but had no string-to-value parser, so
`text_to_value("Off %")` failed (clap-validator `param-conversions` fails on that).
Added a parser accepting `Off` with/without unit, and values in (0, 0.1) now display `0.1`
instead of `0.0` (the filter is still active there; also keeps the text round-trip stable).

Findings that are upstream nih-plug behaviour (not fixed, document if validators flag them):
- 0-frame `process()` calls are skipped by nih-plug's CLAP wrapper (debug builds log
  `Debug assertion failed: buffer_is_valid`); harmless, the plugin is not called.
- CLAP state load allocates `Vec::with_capacity(length_prefix)` before reading, so a
  garbage state with a huge length prefix (e.g. `u64::MAX`) aborts the process. A validator
  fuzzing state with random bytes could hit this; would need a nih-plug patch.
- ANTIALIASING at 0 displays as `Off %` (nih-plug appends the unit to custom labels).

## Checklist
- [x] Code implemented (bundling + scripts; validator runs pending)
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged
