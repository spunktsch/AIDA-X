---
id: TICKET-013
status: backlog
assignee:
priority: high
created: 2026-11-01
updated: 2026-11-01
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
- [ ] Artifact upload with SHA256 checksums
- [ ] Tag-based releases (v*) trigger full release pipeline with codesigning + notarization

## Implementation Notes
- Current workflow uses `distrho/dpf-cmake-action` for each platform
- New workflow uses `dtolnay/rust-toolchain` + `taiki-e/cargo-action` for cross-compilation
- QEMU for ARM64/RISC-V cross-compilation on Ubuntu runners
- macOS runner must have Xcode command line tools installed for AU builds
- codesigning requires stored secrets (`APPLE_CERT`, `APPLE_CERT_PASSWORD`, `APPLE_PROVISIONAL_PROFILE`)
