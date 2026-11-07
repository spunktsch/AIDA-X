---
id: TICKET-020
status: active
assignee:
priority: medium
created: 2026-11-07
updated: 2026-11-07
---

# DPF compatibility: plugin IDs, parameter IDs, state

## Description
The Rust plugin uses its own VST3 class ID and nih-plug hashed parameter IDs, so projects saved with the C++ DPF build do not load.

## Acceptance Criteria
- [x] VST3 class ID reproduced from DPF (`modules/dpf`) for the AIDA-X unique/brand IDs
- [x] CLAP id identical to the DPF CLAP build
- [x] Parameter IDs/indices map to the DPF ones where nih-plug allows; document what is impossible
- [ ] State: read DPF-format state keys (`json`, `cabinet`) on load (impossible without patching nih-plug, see Findings)
- [x] Findings documented in the ticket

## Implementation Notes
Reference: `modules/dpf` at `c1ef624` (submodule pin), nih-plug at `de42101` (Cargo pin).
AIDA-X DPF config: `src/DistrhoPluginCommon.hpp` (included by `src/plugin/DistrhoPluginInfo.h`).

- `plugin_info.rs`: `DPF_VST3_CLASS_TUID`, `DPF_VST3_CLASS_ID_BYTES`, `VST3_CLASS_ID` (pre-swapped
  for nih-plug's Windows byte shuffle). Tests assert the bytes and that nih-plug's
  `PLATFORM_VST3_CLASS_ID` equals DPF's bytes.
- `parameters.rs`: hand-written `Params` impl; string IDs `"<SYMBOL>#<7 chars>"` constructed to
  hash (nih-plug `hash_param_id`) to the DPF host ID. `ParamIdScheme::{Clap, Vst3}`.
- `lib.rs`: `AidaX<F: HostFormat>`; `AidaXClap` exported via `clap_entry`, `AidaXVst3` via
  `GetPluginFactory`, each with its scheme. `filter_state` migrates states of earlier Rust builds
  (bare-symbol keys).

**Breaking (Rust build only):** nih-plug state parameter keys changed from bare DPF symbols to the
constructed IDs; old states are migrated by `filter_state`. The ID construction must never change.

## Findings

### VST3 class ID — compatible
DPF (`DistrhoPluginVST3.cpp`): `dpf_tuid_class = { d_cconst("DPF "), d_cconst("clas"), getUniqueId(), dpf_id_brand }`
as `uint32_t[4]`, `memcpy`'d into `PClassInfo::cid`. `getUniqueId()` = `d_cconst("aida")`
(`DISTRHO_PLUGIN_UNIQUE_ID`); `dpf_id_brand` = 0 because AIDA-X sets `DPF_VST3_DONT_USE_BRAND_ID`
(brand ID `Aida` is *not* used). Words: `0x44504620 0x636c6173 0x61696461 0x00000000`;
bytes on the wire (LE): `20 46 50 44 73 61 6c 63 61 64 69 61 00 00 00 00`.
nih-plug swaps bytes 0..8 on Windows only, so `VST3_CLASS_ID` is `b"DPF ascladia\0\0\0\0"` on Windows
and the raw bytes elsewhere; the host sees DPF's bytes on all platforms (unit-tested).

### CLAP id — compatible
`cc.aidadsp.rt-neural-loader` (`DISTRHO_PLUGIN_CLAP_ID`), used verbatim by `DistrhoPluginCLAP.cpp`.
Was already identical.

### Parameter IDs — compatible (VST3 and CLAP), value ranges partly not (CLAP)
- DPF CLAP: `info->id = index` (0..=20 inputs, 21..=23 outputs).
- DPF VST3: `param_id = index + kVst3InternalParameterCount`; AIDA-X has a UI and no direct access, so
  `DPF_VST3_USES_SEPARATE_CONTROLLER` adds two hidden internal params (0 = buffer size, 1 = sample
  rate); no latency/programs/MIDI. Inputs are 2..=22, bypass (index 17) is 19, outputs 23..=25.
- nih-plug hashes the string ID with `hash_param_id` (`h = h*31 + byte`, top bit cleared) and offers no
  override. The hash is invertible: each string ID is `"<DPF symbol>#"` plus a 7-char suffix (`'0'..='N'`)
  solved in base 31 so the hash equals the DPF ID (e.g. CLAP `PREGAIN#08NL4:N` → 1,
  VST3 `dpf_bypass#227=;E9` → 19). Because CLAP and VST3 need different IDs from the same hash, the
  plugin is exported as two types (`AidaXClap`, `AidaXVst3`). Verified by unit tests against a mirror
  of nih-plug's hash; no fork needed.
- VST3 values are normalized on both sides with identical ranges (linear, same min/max, step counts
  1 for bool/enum), so VST3 automation lanes map 1:1.
- CLAP: DPF reports plain ranges (`min_value..max_value`, e.g. BFREQ 75..600), nih-plug always
  reports continuous params as `0..1` (normalized) and stepped params as `0..step_count`. Bool/enum
  params (all `0..1` in DPF) match exactly. For continuous params, CLAP automation is only preserved if
  the host stores it relative to the declared range; hosts storing plain values will replay them
  unscaled. Fixing this needs nih-plug to expose plain ranges in `params.get_info` (fork/patch of
  `src/wrapper/clap/wrapper.rs`).
- DPF output params (`ModelInSize`, `MeterIn`, `MeterOut`) are not exposed by nih-plug (no read-only
  params); their IDs stay unused. Irrelevant for project loading.

### State — NOT compatible (needs nih-plug patch)
DPF VST3 (`getState`/`setState`) and CLAP (`stateSave`/`stateLoad`) write the same NUL-separated text
blob: `__dpf_state_begin__\0json\0<path>\0cabinet\0<path>\0__dpf_state_end__\0__dpf_parameters_begin__\0`
`<symbol>\0<value>\0...__dpf_parameters_end__\0\xfe\0` (plain values; integer params as integers,
others as decimal strings; outputs skipped). nih-plug's VST3 `IComponent::set_state` and CLAP `state.load`
pass the raw bytes straight to `wrapper::state::deserialize_json` (serde JSON, optionally zstd) and
return failure when parsing fails; `Plugin::filter_state` only runs *after* a successful JSON parse,
and there is no plugin hook for raw state bytes. So a DPF project loads this plugin (same class
ID/CLAP id) and keeps automation, but the stored model/cabinet paths and parameter values are not
restored (plugin starts at defaults).
What would be needed (vendored nih-plug patch, separate ticket): in `wrapper/state.rs`
`deserialize_json`, on JSON failure call an optional `Plugin::deserialize_foreign_state(&[u8]) ->
Option<PluginState>` hook (default `None`); AIDA-X would parse the DPF blob into `PluginState`
(`params`: symbol → value, migrated by `filter_state`; `fields`: `json`/`cabinet` as JSON strings).
Also the VST3 controller-side `set_component_state` is a no-op in nih-plug, which is fine because
nih-plug is a single-component plugin.

## Checklist
- [x] Code implemented
- [x] Tests written
- [x] Self-review passed
- [ ] PR created
- [ ] Merged
