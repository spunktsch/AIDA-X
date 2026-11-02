/*
 * AIDA-X LV2 Plugin Metadata Header
 * Copyright (C) 2022-2023 Massimo Pennazio <maxipenna@libero.it>
 * Copyright (C) 2023-2024 Filipe Coelho <falktx@falktx.com>
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * This header provides LV2 plugin metadata for embedded systems
 * (MOD Dwarf, Portenta-X8, etc.) that need to reference AIDA-X
 * plugin ports and parameters without the full DPF framework.
 */

#ifndef AIDA_X_LV2_PLUGIN_H
#define AIDA_X_LV2_PLUGIN_H

#include <stdint.h>

/* Plugin identity */
#define AIDA_X_LV2_URI "http://aidadsp.cc/plugins/aidadsp-bundle/rt-neural-loader"
#define AIDA_X_LV2_SYMBOL "rt-neural-loader"
#define AIDA_X_LV2_BRAND "AIDA DSP"
#define AIDA_X_LV2_NAME "AIDA-X"
#define AIDA_X_LV2_VERSION_MAJOR 1
#define AIDA_X_LV2_VERSION_MINOR 1
#define AIDA_X_LV2_VERSION_PATCH 0

/* LV2 category */
#define AIDA_X_LV2_CATEGORY "lv2:SimulatorPlugin"

/* Audio ports */
#define AIDA_X_LV2_AUDIO_INPUTS 1
#define AIDA_X_LV2_AUDIO_OUTPUTS 1

/* Port indices */
#define AIDA_X_LV2_PORT_AUDIO_IN_0     0
#define AIDA_X_LV2_PORT_AUDIO_OUT_0    1

/* Event ports (atom:AtomPort with midi:MidiEvent support) */
#define AIDA_X_LV2_PORT_EVENTS_IN      2
#define AIDA_X_LV2_PORT_EVENTS_OUT     3

/* Control parameter ports */
#define AIDA_X_LV2_PORT_ANTIALIASING       4
#define AIDA_X_LV2_PORT_INPUT_GAIN         5
#define AIDA_X_LV2_PORT_NETBYPASS          6
#define AIDA_X_LV2_PORT_EQBYPASS           7
#define AIDA_X_LV2_PORT_EQPOS              8
#define AIDA_X_LV2_PORT_BASSGAIN           9
#define AIDA_X_LV2_PORT_BASSFREQ           10
#define AIDA_X_LV2_PORT_MIDGAIN            11
#define AIDA_X_LV2_PORT_MIDFREQ            12
#define AIDA_X_LV2_PORT_MIDQ               13
#define AIDA_X_LV2_PORT_MTYPE              14
#define AIDA_X_LV2_PORT_TREBLEGAIN         15
#define AIDA_X_LV2_PORT_TREBLEFREQ         16
#define AIDA_X_LV2_PORT_DEPTH              17
#define AIDA_X_LV2_PORT_PRESENCE           18
#define AIDA_X_LV2_PORT_OUTPUT_GAIN        19
#define AIDA_X_LV2_PORT_CABSIMBYPASS       20
#define AIDA_X_LV2_PORT_GLOBALBYPASS       21
#define AIDA_X_LV2_PORT_PARAM1             22
#define AIDA_X_LV2_PORT_PARAM2             23
#define AIDA_X_LV2_PORT_DCBLOCKER          24

/* Output-only parameter ports */
#define AIDA_X_LV2_PORT_MODEL_INPUT_SIZE   25
#define AIDA_X_LV2_PORT_METER_IN           26
#define AIDA_X_LV2_PORT_METER_OUT          27

#define AIDA_X_LV2_TOTAL_PORTS 28

/* Parameter count (excluding audio and event ports) */
#define AIDA_X_LV2_NUM_PARAMS 20
#define AIDA_X_LV2_NUM_OUTPUTS 3

/* Control port default values and ranges */
typedef struct {
    const char*  symbol;
    const char*  name;
    const char*  short_name;
    const char*  unit;
    float        default_val;
    float        minimum;
    float        maximum;
    uint8_t      is_integer;
    uint8_t      is_boolean;
    uint8_t      is_logarithmic;
    uint8_t      is_output;
    uint8_t      enumeration_count;
    const char** enumeration_labels;
} aida_lv2_port_info_t;

/* Audio port info */
typedef struct {
    const char* symbol;
    const char* name;
    uint8_t     is_input;
    uint8_t     is_sidechain;
} aida_lv2_audio_port_info_t;

/* Event port info */
typedef struct {
    const char* symbol;
    const char* name;
    uint8_t     is_input;
    uint8_t     supports_midi;
    uint8_t     supports_string;
} aida_lv2_event_port_info_t;

static const char* const kEQPOS_labels[] = { "POST", "PRE" };
static const char* const kMTYPE_labels[] = { "PEAK", "BANDPASS" };
static const char* const kBYPASS_labels[] = { "ON", "OFF" };
static const char* const kModelInSize_labels[] = { "ERROR", "SNAPSHOT", "WITH 1 PARAM", "WITH 2 PARAMS" };

static const aida_lv2_audio_port_info_t aida_lv2_audio_ports[] = {
    { "lv2_audioIn",  "Audio In",   1, 0 },
    { "lv2_audioOut", "Audio Out",  0, 0 },
};

static const aida_lv2_event_port_info_t aida_lv2_event_ports[] = {
    { "lv2_events_in",  "Events Input",  1, 1, 0 },
    { "lv2_events_out", "Events Output", 0, 1, 0 },
};

static const aida_lv2_port_info_t aida_lv2_control_ports[] = {
    { "inlpf",  "ANTIALIASING",  "ANTIALIASING", "%",     66.216f, 0.f,     100.f,    0, 0, 0, 0, 0, NULL },
    { "inlevel","INPUT",         "PREGAIN",      "dB",     0.f,      -12.f,     12.f,     0, 0, 1, 0, 0, NULL },
    { "netbypass", "NETBYPASS", "NETBYPASS",    "",       0.f,      0.f,       1.f,      0, 1, 0, 0, 2, kBYPASS_labels },
    { "eqbypass", "EQBYPASS",   "EQBYPASS",     "",       0.f,      0.f,       1.f,      0, 1, 0, 0, 2, kBYPASS_labels },
    { "eqpos",  "EQPOS",        "EQPOS",        "",       0.f,      0.f,       1.f,      1, 0, 0, 0, 2, kEQPOS_labels },
    { "bassgain", "BASS",       "BASS",         "dB",     0.f,      -8.f,      8.f,      0, 0, 1, 0, 0, NULL },
    { "bassfreq", "BFREQ",      "BFREQ",        "Hz",     305.f,    75.f,      600.f,    0, 0, 1, 0, 0, NULL },
    { "midgain", "MID",         "MID",          "dB",     0.f,      -8.f,      8.f,      0, 0, 1, 0, 0, NULL },
    { "midfreq", "MFREQ",       "MFREQ",        "Hz",     750.f,    150.f,     5000.f,   0, 0, 1, 0, 0, NULL },
    { "midq",   "MIDQ",         "MIDQ",         "",       0.707f,   0.2f,      5.f,      0, 0, 1, 0, 0, NULL },
    { "mtype",  "MTYPE",        "MTYPE",        "",       0.f,      0.f,       1.f,      1, 0, 0, 0, 2, kMTYPE_labels },
    { "treblegain", "TREBLE",   "TREBLE",       "dB",     0.f,      -8.f,      8.f,      0, 0, 1, 0, 0, NULL },
    { "treblefreq", "TFREQ",    "TFREQ",        "Hz",     2000.f,   1000.f,    4000.f,   0, 0, 1, 0, 0, NULL },
    { "depth",  "DEPTH",        "DEPTH",        "dB",     0.f,      -8.f,      8.f,      0, 0, 1, 0, 0, NULL },
    { "presence", "PRESENCE",   "PRESENCE",     "dB",     0.f,      -8.f,      8.f,      0, 0, 1, 0, 0, NULL },
    { "outlevel", "OUTPUT",     "MASTER",       "dB",     0.f,      -15.f,     15.f,     0, 0, 1, 0, 0, NULL },
    { "cabsimbypass", "CABSIMBYPASS", "CABSIMBYPASS", "",  0.f,      0.f,       1.f,      0, 1, 0, 0, 2, kBYPASS_labels },
    { "globalbypass", "Bypass", "dpf_bypass",   "",       0.f,      0.f,       1.f,      0, 1, 0, 0, 2, kBYPASS_labels },
    { "param1", "PARAM1",       "PARAM1",       "",       0.f,      0.f,       1.f,      0, 0, 0, 0, 0, NULL },
    { "param2", "PARAM2",       "PARAM2",       "",       0.f,      0.f,       1.f,      0, 0, 0, 0, 0, NULL },
    { "dcblocker", "DCBLOCKER", "DCBLOCKER",   "",       1.f,      0.f,       1.f,      0, 1, 0, 0, 2, kBYPASS_labels },
};

static const aida_lv2_port_info_t aida_lv2_output_ports[] = {
    { "modelinputsize", "Model Input Size", "ModelInSize", "", 0.f, 0.f, 3.f, 1, 0, 0, 1, 4, kModelInSize_labels },
    { "meterin", "Meter In", "MeterIn", "dB", 0.f, 0.f, 2.f, 0, 0, 0, 1, 0, NULL },
    { "meterout", "Meter Out", "MeterOut", "dB", 0.f, 0.f, 2.f, 0, 0, 0, 1, 0, NULL },
};

/* State keys for host-readable/writable state */
#define AIDA_X_LV2_STATE_MODEL_FILE    "json"
#define AIDA_X_LV2_STATE_IMPULSE_FILE "cabinet"

#define AIDA_X_LV2_NUM_STATE_KEYS 2

/* Brand identifier for VST3 compatibility */
#define AIDA_X_LV2_BRAND_ID "Aida"
#define AIDA_X_LV2_UNIQUE_ID "aida"

/* CLAP ID (also used for AU CFBundleIdentifier) */
#define AIDA_X_LV2_CLAP_ID "cc.aidadsp.rt-neural-loader"

#endif /* AIDA_X_LV2_PLUGIN_H */
