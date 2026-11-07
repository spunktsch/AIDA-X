//! Plugin metadata, from `src/DistrhoPluginCommon.hpp`.

/// `DISTRHO_PLUGIN_NAME`
pub const PLUGIN_NAME: &str = "AIDA-X";
/// `DISTRHO_PLUGIN_BRAND`
pub const PLUGIN_BRAND: &str = "AIDA DSP";
/// DPF plugin label (`getLabel()`).
pub const PLUGIN_LABEL: &str = "AIDA_X";
/// `DISTRHO_PLUGIN_URI` (LV2 URI; also used as the homepage/manual link).
pub const PLUGIN_URI: &str = "http://aidadsp.cc/plugins/aidadsp-bundle/rt-neural-loader";
/// `DISTRHO_PLUGIN_CLAP_ID`
pub const CLAP_ID: &str = "cc.aidadsp.rt-neural-loader";
/// Plugin description (`getDescription()`).
pub const PLUGIN_DESCRIPTION: &str =
    "Simple loader for neural models using RTNeural inference engine.";
/// `getHomePage()`
pub const PLUGIN_HOMEPAGE: &str = "https://aidadsp.github.io/";
/// Support / issue tracker.
pub const PLUGIN_SUPPORT_URL: &str = "https://github.com/AidaDSP/AIDA-X/issues";

/// `DISTRHO_PLUGIN_BRAND_ID` (not part of the VST3 UID, see [`DPF_VST3_CLASS_TUID`]).
pub const DPF_BRAND_ID: &[u8; 4] = b"Aida";
/// `DISTRHO_PLUGIN_UNIQUE_ID`
pub const DPF_UNIQUE_ID: &[u8; 4] = b"aida";

/// DPF's `d_cconst()`: four ASCII characters packed big-endian into a `uint32_t`.
pub const fn d_cconst(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

/// The VST3 class UID of the DPF build, as DPF's `dpf_tuid` (`uint32_t[4]`).
///
/// From `modules/dpf/distrho/src/DistrhoPluginVST3.cpp`:
///
/// ```text
/// dpf_tuid dpf_tuid_class = { dpf_id_entry, dpf_id_clas, 0, dpf_id_brand };
///   dpf_id_entry = d_cconst('D','P','F',' ')                     = 0x44504620
///   dpf_id_clas  = d_cconst('c','l','a','s')                     = 0x636c6173
///   [2]          = sPlugin->getUniqueId()  (set in VST3_ENTRYFNNAME)
///                = d_cconst(STRINGIFY(DISTRHO_PLUGIN_UNIQUE_ID))
///                = d_cconst("aida")                              = 0x61696461
///   dpf_id_brand = 0, because src/DistrhoPluginCommon.hpp defines
///                  DPF_VST3_DONT_USE_BRAND_ID ("AIDA-X predates DPF support for VST3 brand uid")
/// ```
pub const DPF_VST3_CLASS_TUID: [u32; 4] = [
    d_cconst(b"DPF "),
    d_cconst(b"clas"),
    d_cconst(DPF_UNIQUE_ID),
    0,
];

/// The 16 bytes DPF hands to the host as `PClassInfo::cid`: it `memcpy`s the `uint32_t[4]`
/// above, so each word appears in native (little-endian on every supported target) order:
/// `20 46 50 44 | 73 61 6c 63 | 61 64 69 61 | 00 00 00 00` (ASCII `" FPDsalcadia\0\0\0\0"`).
pub const DPF_VST3_CLASS_ID_BYTES: [u8; 16] = tuid_bytes(DPF_VST3_CLASS_TUID);

const fn tuid_bytes(tuid: [u32; 4]) -> [u8; 16] {
    let mut out = [0u8; 16];
    let mut word = 0;
    while word < 4 {
        let bytes = tuid[word].to_ne_bytes();
        let mut i = 0;
        while i < 4 {
            out[word * 4 + i] = bytes[i];
            i += 1;
        }
        word += 1;
    }
    out
}

/// nih-plug's `Vst3Plugin::PLATFORM_VST3_CLASS_ID` applies a COM-GUID byte swap to bytes
/// 0..8 on Windows only (`swap_vst3_uid_byte_order` in `src/plugin/vst3.rs`). The swap is an
/// involution, so applying it here makes the bytes the host finally sees equal
/// [`DPF_VST3_CLASS_ID_BYTES`] on every platform.
const fn undo_nih_plug_platform_swap(uid: [u8; 16]) -> [u8; 16] {
    if cfg!(target_os = "windows") {
        let o = uid;
        let mut uid = uid;
        uid[0] = o[3];
        uid[1] = o[2];
        uid[2] = o[1];
        uid[3] = o[0];
        uid[4] = o[5];
        uid[5] = o[4];
        uid[6] = o[7];
        uid[7] = o[6];
        uid
    } else {
        uid
    }
}

/// VST3 class ID handed to nih-plug, chosen so the host sees DPF's class UID
/// ([`DPF_VST3_CLASS_ID_BYTES`]) and existing projects resolve to this plugin.
pub const VST3_CLASS_ID: [u8; 16] = undo_nih_plug_platform_swap(DPF_VST3_CLASS_ID_BYTES);

/// `kDefaultModelName`: bundled default neural model.
pub const DEFAULT_MODEL: &str = "tw40_california_clean.json";
/// `kDefaultCabinetName`: bundled default cabinet IR.
pub const DEFAULT_CABINET: &str = "V30-P2-audix-i5.wav";

#[cfg(test)]
mod tests {
    use super::*;

    /// Derivation (see [`DPF_VST3_CLASS_TUID`]):
    /// `{ 'DPF ', 'clas', 'aida', 0 }` as `uint32_t[4]`, memcpy'd in little-endian order.
    #[test]
    fn vst3_class_id_matches_dpf() {
        assert_eq!(
            DPF_VST3_CLASS_TUID,
            [0x4450_4620, 0x636c_6173, 0x6169_6461, 0]
        );
        assert_eq!(
            DPF_VST3_CLASS_ID_BYTES,
            [
                0x20, 0x46, 0x50, 0x44, // "DPF " as LE u32
                0x73, 0x61, 0x6c, 0x63, // "clas" as LE u32
                0x61, 0x64, 0x69, 0x61, // "aida" as LE u32 (DISTRHO_PLUGIN_UNIQUE_ID)
                0x00, 0x00, 0x00, 0x00, // brand ID disabled (DPF_VST3_DONT_USE_BRAND_ID)
            ]
        );
    }

    /// What nih-plug actually reports to the host must be DPF's bytes on this platform.
    #[cfg(feature = "vst3")]
    #[test]
    fn nih_plug_reports_dpf_class_id() {
        use nih_plug::prelude::Vst3Plugin;
        assert_eq!(
            <crate::AidaXVst3 as Vst3Plugin>::PLATFORM_VST3_CLASS_ID,
            DPF_VST3_CLASS_ID_BYTES
        );
        #[cfg(target_os = "windows")]
        assert_eq!(&VST3_CLASS_ID, b"DPF ascladia\0\0\0\0");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(VST3_CLASS_ID, DPF_VST3_CLASS_ID_BYTES);
    }

    #[test]
    fn clap_id_matches_dpf() {
        // DISTRHO_PLUGIN_CLAP_ID in src/DistrhoPluginCommon.hpp, used verbatim by
        // DistrhoPluginCLAP.cpp as the descriptor id.
        assert_eq!(CLAP_ID, "cc.aidadsp.rt-neural-loader");
    }
}
