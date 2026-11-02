/// Parameter definitions matching DistrhoPluginCommon.hpp exactly.
///
/// 24 parameters total (kParameterCount = 24), matching the C++ enum:
///   kParameterINLPF=0, kParameterINLEVEL=1, kParameterNETBYPASS=2,
///   kParameterEQBYPASS=3, kParameterEQPOS=4, kParameterBASSGAIN=5,
///   kParameterBASSFREQ=6, kParameterMIDGAIN=7, kParameterMIDFREQ=8,
///   kParameterMIDQ=9, kParameterMTYPE=10, kParameterTREBLEGAIN=11,
///   kParameterTREBLEFREQ=12, kParameterDEPTH=13, kParameterPRESENCE=14,
///   kParameterOUTLEVEL=15, kParameterCABSIMBYPASS=16,
///   kParameterGLOBALBYPASS=17, kParameterPARAM1=18, kParameterPARAM2=19,
///   kParameterDCBLOCKER=20, kParameterModelInputSize=21,
///   kParameterMeterIn=22, kParameterMeterOut=23

use aidadx_core::biquad::{EqPos, MidEqType};

/// Parameter index matching the C++ enum order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ParameterIndex {
    Antialiasing = 0,       // INLPF
    InputLevel,             // INLEVEL
    NetBypass,              // NETBYPASS
    EqBypass,               // EQBYPASS
    EqPos,                  // EQPOS
    BassGain,               // BASSGAIN
    BassFreq,               // BASSFREQ
    MidGain,                // MIDGAIN
    MidFreq,                // MIDFREQ
    MidQ,                   // MIDQ
    MidType,                // MTYPE
    TrebleGain,             // TREBLEGAIN
    TrebleFreq,             // TREBLEFREQ
    Depth,                  // DEPTH
    Presence,               // PRESENCE
    OutputLevel,            // OUTLEVEL
    CabSimBypass,           // CABSIMBYPASS
    GlobalBypass,           // GLOBALBYPASS
    Param1,                 // PARAM1
    Param2,                 // PARAM2
    DcBlocker,              // DCBLOCKER
    ModelInputSize,         // Model Input Size (output)
    MeterIn,                // Meter In (output)
    MeterOut,               // Meter Out (output)
}

impl ParameterIndex {
    pub const COUNT: u32 = 24;

    pub fn from_u32(index: u32) -> Option<ParameterIndex> {
        match index {
            0 => Some(ParameterIndex::Antialiasing),
            1 => Some(ParameterIndex::InputLevel),
            2 => Some(ParameterIndex::NetBypass),
            3 => Some(ParameterIndex::EqBypass),
            4 => Some(ParameterIndex::EqPos),
            5 => Some(ParameterIndex::BassGain),
            6 => Some(ParameterIndex::BassFreq),
            7 => Some(ParameterIndex::MidGain),
            8 => Some(ParameterIndex::MidFreq),
            9 => Some(ParameterIndex::MidQ),
            10 => Some(ParameterIndex::MidType),
            11 => Some(ParameterIndex::TrebleGain),
            12 => Some(ParameterIndex::TrebleFreq),
            13 => Some(ParameterIndex::Depth),
            14 => Some(ParameterIndex::Presence),
            15 => Some(ParameterIndex::OutputLevel),
            16 => Some(ParameterIndex::CabSimBypass),
            17 => Some(ParameterIndex::GlobalBypass),
            18 => Some(ParameterIndex::Param1),
            19 => Some(ParameterIndex::Param2),
            20 => Some(ParameterIndex::DcBlocker),
            21 => Some(ParameterIndex::ModelInputSize),
            22 => Some(ParameterIndex::MeterIn),
            23 => Some(ParameterIndex::MeterOut),
            _ => None,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Parameter definition matching DPF Parameter struct exactly
#[derive(Debug, Clone)]
pub struct ParameterDef {
    pub index: ParameterIndex,
    pub clap_id: &'static str,
    pub name: &'static str,
    pub unit: &'static str,
    pub default_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub is_output: bool,
    pub is_boolean: bool,
    pub is_integer: bool,
    pub enums: &'static [ParameterEnumeration],
}

/// Enumeration value for integer/discrete parameters
#[derive(Debug, Clone)]
pub struct ParameterEnumeration {
    pub value: f64,
    pub label: &'static str,
}

/// All 24 parameter definitions matching DPF kParameters[] exactly
pub fn all_parameters() -> [ParameterDef; 24] {
    [
        // 0: ANTIALIASING (INLPF) - input lowpass filter knob
        ParameterDef {
            index: ParameterIndex::Antialiasing,
            clap_id: "antialiasing",
            name: "ANTIALIASING",
            unit: "%",
            default_value: 66.216,
            min_value: 0.0,
            max_value: 100.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 1: INPUT / PREGAIN
        ParameterDef {
            index: ParameterIndex::InputLevel,
            clap_id: "input_level",
            name: "INPUT",
            unit: "dB",
            default_value: 0.0,
            min_value: -12.0,
            max_value: 12.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 2: NETBYPASS
        ParameterDef {
            index: ParameterIndex::NetBypass,
            clap_id: "net_bypass",
            name: "NETBYPASS",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: true,
            is_integer: true,
            enums: &BYPASS_ENUM,
        },
        // 3: EQBYPASS
        ParameterDef {
            index: ParameterIndex::EqBypass,
            clap_id: "eq_bypass",
            name: "EQBYPASS",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: true,
            is_integer: true,
            enums: &BYPASS_ENUM,
        },
        // 4: EQPOS - pre/post EQ position
        ParameterDef {
            index: ParameterIndex::EqPos,
            clap_id: "eq_pos",
            name: "EQPOS",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: false,
            is_integer: true,
            enums: &EQPOS_ENUM,
        },
        // 5: BASS
        ParameterDef {
            index: ParameterIndex::BassGain,
            clap_id: "bass_gain",
            name: "BASS",
            unit: "dB",
            default_value: 0.0,
            min_value: -8.0,
            max_value: 8.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 6: BFREQ
        ParameterDef {
            index: ParameterIndex::BassFreq,
            clap_id: "bass_freq",
            name: "BFREQ",
            unit: "Hz",
            default_value: 305.0,
            min_value: 75.0,
            max_value: 600.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 7: MID
        ParameterDef {
            index: ParameterIndex::MidGain,
            clap_id: "mid_gain",
            name: "MID",
            unit: "dB",
            default_value: 0.0,
            min_value: -8.0,
            max_value: 8.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 8: MFREQ
        ParameterDef {
            index: ParameterIndex::MidFreq,
            clap_id: "mid_freq",
            name: "MFREQ",
            unit: "Hz",
            default_value: 750.0,
            min_value: 150.0,
            max_value: 5000.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 9: MIDQ
        ParameterDef {
            index: ParameterIndex::MidQ,
            clap_id: "mid_q",
            name: "MIDQ",
            unit: "",
            default_value: 0.707,
            min_value: 0.2,
            max_value: 5.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 10: MTYPE
        ParameterDef {
            index: ParameterIndex::MidType,
            clap_id: "mid_type",
            name: "MTYPE",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: false,
            is_integer: true,
            enums: &MTYPE_ENUM,
        },
        // 11: TREBLE
        ParameterDef {
            index: ParameterIndex::TrebleGain,
            clap_id: "treble_gain",
            name: "TREBLE",
            unit: "dB",
            default_value: 0.0,
            min_value: -8.0,
            max_value: 8.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 12: TFREQ
        ParameterDef {
            index: ParameterIndex::TrebleFreq,
            clap_id: "treble_freq",
            name: "TFREQ",
            unit: "Hz",
            default_value: 2000.0,
            min_value: 1000.0,
            max_value: 4000.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 13: DEPTH
        ParameterDef {
            index: ParameterIndex::Depth,
            clap_id: "depth",
            name: "DEPTH",
            unit: "dB",
            default_value: 0.0,
            min_value: -8.0,
            max_value: 8.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 14: PRESENCE
        ParameterDef {
            index: ParameterIndex::Presence,
            clap_id: "presence",
            name: "PRESENCE",
            unit: "dB",
            default_value: 0.0,
            min_value: -8.0,
            max_value: 8.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 15: OUTPUT / MASTER
        ParameterDef {
            index: ParameterIndex::OutputLevel,
            clap_id: "output_level",
            name: "OUTPUT",
            unit: "dB",
            default_value: 0.0,
            min_value: -15.0,
            max_value: 15.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 16: CABSIMBYPASS
        ParameterDef {
            index: ParameterIndex::CabSimBypass,
            clap_id: "cab_sim_bypass",
            name: "CABSIMBYPASS",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: true,
            is_integer: true,
            enums: &BYPASS_ENUM,
        },
        // 17: GLOBAL Bypass (dpf_bypass)
        ParameterDef {
            index: ParameterIndex::GlobalBypass,
            clap_id: "bypass",
            name: "Bypass",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: true,
            is_integer: true,
            enums: &GLOBAL_BYPASS_ENUM,
        },
        // 18: PARAM1
        ParameterDef {
            index: ParameterIndex::Param1,
            clap_id: "param1",
            name: "PARAM1",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 19: PARAM2
        ParameterDef {
            index: ParameterIndex::Param2,
            clap_id: "param2",
            name: "PARAM2",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 20: DCBLOCKER
        ParameterDef {
            index: ParameterIndex::DcBlocker,
            clap_id: "dc_blocker",
            name: "DCBLOCKER",
            unit: "",
            default_value: 1.0,
            min_value: 0.0,
            max_value: 1.0,
            is_output: false,
            is_boolean: true,
            is_integer: true,
            enums: &DC_BLOCKER_ENUM,
        },
        // 21: Model Input Size (output only)
        ParameterDef {
            index: ParameterIndex::ModelInputSize,
            clap_id: "model_input_size",
            name: "Model Input Size",
            unit: "",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 3.0,
            is_output: true,
            is_boolean: false,
            is_integer: true,
            enums: &MODEL_IN_SIZE_ENUM,
        },
        // 22: Meter In (output only)
        ParameterDef {
            index: ParameterIndex::MeterIn,
            clap_id: "meter_in",
            name: "Meter In",
            unit: "dB",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 2.0,
            is_output: true,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
        // 23: Meter Out (output only)
        ParameterDef {
            index: ParameterIndex::MeterOut,
            clap_id: "meter_out",
            name: "Meter Out",
            unit: "dB",
            default_value: 0.0,
            min_value: 0.0,
            max_value: 2.0,
            is_output: true,
            is_boolean: false,
            is_integer: false,
            enums: &[],
        },
    ]
}

// Enumeration values matching DPF exactly

static BYPASS_ENUM: &[ParameterEnumeration] = &[
    ParameterEnumeration { value: 0.0, label: "ON" },
    ParameterEnumeration { value: 1.0, label: "OFF" },
];

static GLOBAL_BYPASS_ENUM: &[ParameterEnumeration] = &[
    ParameterEnumeration { value: 0.0, label: "PROCESSING" },
    ParameterEnumeration { value: 1.0, label: "BYPASSED" },
];

static EQPOS_ENUM: &[ParameterEnumeration] = &[
    ParameterEnumeration { value: 0.0, label: "POST" },
    ParameterEnumeration { value: 1.0, label: "PRE" },
];

static MTYPE_ENUM: &[ParameterEnumeration] = &[
    ParameterEnumeration { value: 0.0, label: "PEAK" },
    ParameterEnumeration { value: 1.0, label: "BANDPASS" },
];

static DC_BLOCKER_ENUM: &[ParameterEnumeration] = &[
    ParameterEnumeration { value: 0.0, label: "OFF" },
    ParameterEnumeration { value: 1.0, label: "ON" },
];

static MODEL_IN_SIZE_ENUM: &[ParameterEnumeration] = &[
    ParameterEnumeration { value: 0.0, label: "ERROR" },
    ParameterEnumeration { value: 1.0, label: "SNAPSHOT" },
    ParameterEnumeration { value: 2.0, label: "WITH 1 PARAM" },
    ParameterEnumeration { value: 3.0, label: "WITH 2 PARAMS" },
];

impl ParameterDef {
    /// Check if this parameter is automatable (not output-only)
    pub fn is_automatable(&self) -> bool {
        !self.is_output
    }

    /// Get the CLAP parameter flags for this parameter
    pub fn clap_flags(&self) -> u32 {
        let mut flags = 0u32;
        // clap_param_flags::CLAP_PARAM_IS_AUTOMATABLE = 1
        if self.is_automatable() {
            flags |= 1;
        }
        // clap_param_flags::CLAP_PARAM_IS_BYPASS = 8192 (for global bypass)
        if self.index == ParameterIndex::GlobalBypass {
            flags |= 8192;
        }
        // clap_param_flags::CLAP_PARAM_IS_HIDDEN = 16384 (for output-only meters)
        if self.is_output {
            flags |= 16384;
        }
        flags
    }

    /// Resolve an enum value by its index
    pub fn get_enum(&self, index: u32) -> Option<&ParameterEnumeration> {
        if self.enums.is_empty() {
            return None;
        }
        self.enums.get(index as usize)
    }

    /// Get the total number of enum values
    pub fn enum_count(&self) -> u32 {
        self.enums.len() as u32
    }

    /// Convert value to human-readable string
    pub fn value_to_string(&self, value: f64) -> String {
        if !self.enums.is_empty() {
            // Find the closest enum match
            for (i, e) in self.enums.iter().enumerate() {
                if (value - e.value).abs() < 0.01 {
                    return e.label.to_string();
                }
            }
        }
        format!("{:.3}", value)
    }

    /// Convert EqPos parameter value to enum
    pub fn eq_pos_from_value(value: f64) -> EqPos {
        if value > 0.5 {
            EqPos::Pre
        } else {
            EqPos::Post
        }
    }

    /// Convert MidType parameter value to enum
    pub fn mid_type_from_value(value: f64) -> MidEqType {
        if value > 0.5 {
            MidEqType::Bandpass
        } else {
            MidEqType::Peak
        }
    }
}

/// Resolve parameter definition by index
pub fn parameter_by_index(index: u32) -> Option<&'static ParameterDef> {
    all_parameters().get(index as usize).map(|p| p)
}

/// Resolve parameter definition by CLAP ID
pub fn parameter_by_clap_id(id: &str) -> Option<&'static ParameterDef> {
    all_parameters().iter().find(|p| p.clap_id == id)
}
