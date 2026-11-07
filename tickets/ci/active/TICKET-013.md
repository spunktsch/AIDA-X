---
id: TICKET-013
status: active
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-07
---

# Set up CI/CD GitHub Actions with multi-platform build matrix

## Description
Create comprehensive GitHub Actions workflow that builds all plugin formats for all supported platforms. Replaces the current CMake-based build.yml with a Rust/cargo-based pipeline.

## Acceptance Criteria
- [ ] `.github/workflows/build.yml` with matrix covering: linux-x64, linux-arm64, linux-riscv64, macos, windows, ios, android, wasm
- [ ] Each job runs `cargo build` with appropriate target and feature flags
- [ ] Linux jobs: build VST3, CLAP, LV2, standalone; run `cargo test`
- [ ] macOS job: build VST3, AU, CLAP, standalone; run `pluginval` on VST3 and AU
- [ ] Windows job: build VST3, CLAP, standalone; generate Inno Setup installer
- [ ] iOS job: build AUv3 + standalone ipa using `cargo-lipo` and Xcode toolchain
- [ ] WASM job: build with emscripten or wasm32 target; produce optimized zip
- [x] Artifact upload with SHA256 checksums
- [ ] Tag-based releases (v*) trigger full release pipeline with codesigning + notarization

## Implementation Notes
- Current workflow uses `distrho/dpf-cmake-action` for each platform
- New workflow uses `dtolnay/rust-toolchain` + `taiki-e/cargo-action` for cross-compilation
- QEMU for ARM64/RISC-V cross-compilation on Ubuntu runners
- macOS runner must have Xcode command line tools installed for AU builds
- codesigning requires stored secrets (`APPLE_CERT`, `APPLE_CERT_PASSWORD`, `APPLE_PROVISIONAL_PROFILE`)

## Results

New workflow `.github/workflows/rust.yml` (the C++/DPF `build.yml` is untouched and keeps
building the shipping plugin; replacing it is deferred until the Rust plugin ships with
feature parity). Everything below is **unverified until the first CI run** on GitHub; locally
(Windows/MSVC) `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets
--locked -- -D warnings` (clean, no fixes needed) and `cargo xtask bundle aidadx_plugin
--release --locked` were run successfully.

Workflow structure:
- Triggers: push (all branches, tags `v*`), pull_request. `permissions: contents: read`;
  concurrency group per PR/ref, cancel-in-progress for PRs only.
- `clippy` (ubuntu-24.04): `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- `build` matrix: linux-x64 (ubuntu-24.04), linux-arm64 (native ubuntu-24.04-arm, no QEMU),
  windows-x64 (windows-latest, MSVC), macos-arm64 (macos-14), macos-x64 (cross-compiled
  `x86_64-apple-darwin` on macos-14, tests skipped). Each: checkout + only
  `modules/rtneural` (shallow, non-recursive: Eigen/json are in-tree, xsimd unused),
  `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `cargo test --workspace`,
  `cargo xtask bundle aidadx_plugin --release`, zip `AIDA-X.clap` + `AIDA-X.vst3` into
  `AIDA-X-<name>.zip` + `.sha256`, `actions/upload-artifact@v4`.
- `validate` (needs build; linux-x64 under xvfb, windows-x64): runs
  `scripts/validate-plugin.{sh,ps1} --skip-bundle` on the downloaded artifact,
  `continue-on-error: true` because the validator download URLs are unverified.
- `release` (tags `v*`, needs build, `contents: write`): downloads all artifacts, verifies
  checksums, writes `SHA256SUMS`, attaches zips + sums via `softprops/action-gh-release@v2`.
- No Linux apt packages: nih-plug is used with default features off (no GUI, no
  standalone/JACK/ALSA); Cargo.lock has no `*-sys` system-library crates.

Criteria status:
- [x] Artifact upload with SHA256 checksums (unverified until first CI run).
- [ ] build.yml matrix: implemented as separate `rust.yml` for linux-x64, linux-arm64,
  windows, macos (arm64 + x64). Blockers: replacing `build.yml` would drop the shipping C++
  build; linux-riscv64 needs a cross toolchain/QEMU (no native runner) and is not supported
  yet; ios/android/wasm targets do not exist in the Rust codebase (`aidadx_ios` is a stub).
- [ ] `cargo build` per target with feature flags: done via `cargo xtask bundle` (incl.
  `--target x86_64-apple-darwin`), but only for the targets above.
- [ ] Linux VST3/CLAP/LV2/standalone + `cargo test`: VST3, CLAP and tests done; LV2 is not
  supported by nih-plug and standalone is not implemented.
- [ ] macOS VST3/AU/CLAP/standalone + pluginval: VST3/CLAP built per arch; no universal
  binary because `xtask/src/main.rs` only implements `bundle`, not nih_plug_xtask's
  `bundle-universal` (follow-up: add it, then lipo in CI); AU and standalone do not exist
  in the Rust plugin; pluginval only runs on Linux/Windows (non-blocking).
- [ ] Windows VST3/CLAP/standalone + Inno Setup installer: VST3/CLAP done; no standalone,
  no installer script.
- [ ] iOS AUv3 job: blocked, no iOS target in the Rust codebase.
- [ ] WASM job: blocked, no WASM target in the Rust codebase.
- [ ] Tag releases with codesigning + notarization: release job exists (unsigned);
  codesigning/notarization need Apple/Windows signing secrets.

Risks for the first CI run: validator URLs/asset names in `scripts/validate-plugin.*` are
unverified (non-blocking); pluginval on Linux may need more X/audio libs than xvfb; both
`build.yml` and `rust.yml` publish to the same GitHub release on `v*` tags; MSRV
(`rust-version = 1.75`) is not tested, only stable.
