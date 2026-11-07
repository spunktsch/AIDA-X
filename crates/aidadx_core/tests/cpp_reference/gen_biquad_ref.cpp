// Golden-reference generator for the Rust biquad / tone-control port (TICKET-022).
//
// Runs the ORIGINAL C++ DSP code on deterministic signals and writes the
// outputs to crates/aidadx_core/tests/fixtures/biquad_golden.{bin,idx}:
//
//   - src/Biquad.cpp / src/Biquad.h are compiled unchanged (included below).
//   - DPF's ExponentialValueSmoother / LinearValueSmoother are used unchanged
//     from modules/dpf/distrho/extra/ValueSmoother.hpp.
//   - The tone-control code in src/aidadsp-plugin.cpp is entangled with the
//     DPF Plugin class, model and convolver, so the pieces needed are copied
//     VERBATIM below with a source:line citation (commit cc32486).
//
// Fixture format
//   biquad_golden.bin : concatenated little-endian f32 records
//   biquad_golden.idx : text, one record per line: "<name> <offset> <len>"
//                       (offset/len in f32 elements). Lines starting with '#'
//                       are comments. Records named "input/<signal>" hold the
//                       input signals; Rust reads those instead of regenerating
//                       them so no libm differences (sin/exp) can creep in.
//
// Build: see build_and_run.bat / build_and_run.sh next to this file.

#define _USE_MATH_DEFINES
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include "Biquad.cpp" // src/Biquad.cpp, unchanged (also pulls in src/Biquad.h)
#include "extra/ValueSmoother.hpp" // modules/dpf/distrho/extra/ValueSmoother.hpp, unchanged

USE_NAMESPACE_DISTRHO

// ---------------------------------------------------------------------------------------------------------------------
// Minimal stand-ins for src/DistrhoPluginCommon.hpp (enum values copied from lines 70-115)

enum Parameters {
    kParameterINLPF,
    kParameterINLEVEL,
    kParameterNETBYPASS,
    kParameterEQBYPASS,
    kParameterEQPOS,
    kParameterBASSGAIN,
    kParameterBASSFREQ,
    kParameterMIDGAIN,
    kParameterMIDFREQ,
    kParameterMIDQ,
    kParameterMTYPE,
    kParameterTREBLEGAIN,
    kParameterTREBLEFREQ,
    kParameterDEPTH,
    kParameterPRESENCE,
    kParameterOUTLEVEL,
    kParameterCABSIMBYPASS,
    kParameterGLOBALBYPASS,
    kParameterPARAM1,
    kParameterPARAM2,
    kParameterDCBLOCKER,
    kParameterModelInputSize,
    kParameterMeterIn,
    kParameterMeterOut,
    kParameterCount
};
static constexpr const uint32_t kNumParameters = kParameterCount;

enum EqPos { kEqPost, kEqPre };
enum MidEqType { kMidEqPeak, kMidEqBandpass };

// Defaults: src/DistrhoPluginCommon.hpp:140-163 (kParameters[i].ranges.def)
static const float kParameterDefaults[kNumParameters] = {
    66.216f, 0.f, 0.f, 0.f, 0.f, 0.f, 305.f, 0.f, 750.f, 0.707f, 0.f, 0.f,
    2000.f, 0.f, 0.f, 0.f, 0.f, 0.f, 0.f, 0.f, 1.f, 0.f, 0.f, 0.f,
};

// ---------------------------------------------------------------------------------------------------------------------
// BEGIN VERBATIM: src/aidadsp-plugin.cpp:33-49

/* Define a constexpr for converting a gain in dB to a coefficient */
static constexpr float DB_CO(const float g) { return g > -90.f ? std::pow(10.f, g * 0.05f) : 0.f; }

/* Define a macro to re-maps a number from one range to another  */
static constexpr float MAP(const float x, const float in_min, const float in_max, const float out_min, const float out_max)
{
    return ((x - in_min) * (out_max - out_min) / (in_max - in_min)) + out_min;
}

/* Defines for tone controls */
static constexpr const float COMMON_Q = 0.707f;
static constexpr const float DEPTH_FREQ = 75.f;
static constexpr const float PRESENCE_FREQ = 900.f;

/* Defines for antialiasing filter */
static constexpr const float INLPF_MAX_CO = 0.99f * 0.5f; /* coeff * ((samplerate / 2) / samplerate) */
static constexpr const float INLPF_MIN_CO = 0.25f * 0.5f; /* coeff * ((samplerate / 2) / samplerate) */

// END VERBATIM
// ---------------------------------------------------------------------------------------------------------------------
// BEGIN VERBATIM: src/aidadsp-plugin.cpp:56-106

struct AidaToneControl {
    Biquad dc_blocker { bq_type_highpass, 0.5f, COMMON_Q, 0.0f };
    Biquad in_lpf { bq_type_lowpass, 0.5f, COMMON_Q, 0.0f };
    Biquad bass { bq_type_lowshelf, 0.5f, COMMON_Q, 0.0f };
    Biquad mid { bq_type_peak, 0.5f, COMMON_Q, 0.0f };
    Biquad treble { bq_type_highshelf, 0.5f, COMMON_Q, 0.0f };
    Biquad depth { bq_type_peak, 0.5f, COMMON_Q, 0.0f };
    Biquad presence { bq_type_highshelf, 0.5f, COMMON_Q, 0.0f };
    ExponentialValueSmoother inlevel;
    ExponentialValueSmoother outlevel;
    bool net_bypass = false;
    bool eq_bypass = false;
    EqPos eq_pos = kEqPost;
    MidEqType mid_type = kMidEqPeak;

    AidaToneControl()
    {
        inlevel.setTimeConstant(1);
        outlevel.setTimeConstant(1);
    }

    void setSampleRate(const float parameters[kNumParameters], const double sampleRate)
    {
        dc_blocker.setFc(35.0f / sampleRate);

        in_lpf.setFc(MAP(parameters[kParameterINLPF], 0.0f, 100.0f, INLPF_MAX_CO, INLPF_MIN_CO));

        bass.setBiquad(bq_type_lowshelf,
                       parameters[kParameterBASSFREQ] / sampleRate, COMMON_Q, parameters[kParameterBASSGAIN]);

        mid.setBiquad(mid_type == kMidEqBandpass ? bq_type_bandpass : bq_type_peak,
                      parameters[kParameterMIDFREQ] / sampleRate,
                      parameters[kParameterMIDQ],
                      parameters[kParameterMIDGAIN]);

        treble.setBiquad(bq_type_highshelf,
                         parameters[kParameterTREBLEFREQ] / sampleRate, COMMON_Q, parameters[kParameterTREBLEGAIN]);

        depth.setBiquad(bq_type_peak,
                        DEPTH_FREQ / sampleRate, COMMON_Q, parameters[kParameterDEPTH]);

        presence.setBiquad(bq_type_highshelf,
                           PRESENCE_FREQ / sampleRate, COMMON_Q, parameters[kParameterPRESENCE]);

        inlevel.setSampleRate(sampleRate);
        inlevel.setTargetValue(DB_CO(parameters[kParameterINLEVEL]));

        outlevel.setSampleRate(sampleRate);
        outlevel.setTargetValue(DB_CO(parameters[kParameterOUTLEVEL]));
    }
};

// END VERBATIM
// ---------------------------------------------------------------------------------------------------------------------
// BEGIN VERBATIM: src/aidadsp-plugin.cpp:141-179

static void applyGainRamp(ExponentialValueSmoother& smoother, float* const out, const uint32_t numSamples)
{
    for (uint32_t i=0; i<numSamples; ++i)
        out[i] *= smoother.next();
}

// --------------------------------------------------------------------------------------------------------------------
// Apply filter

static void applyBiquadFilter(Biquad& filter, float* const out, const float* const in, const uint32_t numSamples)
{
    for (uint32_t i=0; i<numSamples; ++i)
        out[i] = filter.process(in[i]);
}

static void applyBiquadFilter(Biquad& filter, float* const out, const uint32_t numSamples)
{
    for (uint32_t i=0; i<numSamples; ++i)
        out[i] = filter.process(out[i]);
}

// --------------------------------------------------------------------------------------------------------------------
// Apply biquad cascade filters

static void applyToneControls(AidaToneControl& aida, float* const out, uint32_t numSamples)
{
    if (aida.mid_type == kMidEqBandpass)
    {
        applyBiquadFilter(aida.mid, out, numSamples);
    }
    else
    {
        applyBiquadFilter(aida.depth, out, numSamples);
        applyBiquadFilter(aida.bass, out, numSamples);
        applyBiquadFilter(aida.mid, out, numSamples);
        applyBiquadFilter(aida.treble, out, numSamples);
        applyBiquadFilter(aida.presence, out, numSamples);
    }
}

// END VERBATIM
// ---------------------------------------------------------------------------------------------------------------------
// The tone-control part of AidaDSPLoaderPlugin (no model, no cabinet, no global bypass).

struct TonePlugin {
    AidaToneControl aida;
    float parameters[kNumParameters];
    double sampleRate;
    bool enabledLPF = true; // src/aidadsp-plugin.cpp:291
    bool enabledDC = true;  // src/aidadsp-plugin.cpp:292

    explicit TonePlugin(double sr) : sampleRate(sr)
    {
        for (uint32_t i=0; i<kNumParameters; ++i)
            parameters[i] = kParameterDefaults[i];
    }

    // src/aidadsp-plugin.cpp:1102-1104 (sampleRateChanged, tone-control part)
    void sampleRateChanged(const double newSampleRate)
    {
        sampleRate = newSampleRate;
        aida.setSampleRate(parameters, newSampleRate);
    }

    // src/aidadsp-plugin.cpp:888-891 (activate, tone-control part)
    void activate()
    {
        aida.inlevel.clearToTargetValue();
        aida.outlevel.clearToTargetValue();
    }

    // BEGIN VERBATIM (tone-control cases): src/aidadsp-plugin.cpp:504-559, 573-575
    void setParameterValue(const uint32_t index, const float value)
    {
        parameters[index] = value;

        switch (static_cast<Parameters>(index))
        {
        case kParameterINLPF:
            aida.in_lpf.setFc(MAP(value, 0.0f, 100.0f, INLPF_MAX_CO, INLPF_MIN_CO));
            enabledLPF = d_isNotZero(value);
            break;
        case kParameterINLEVEL:
            aida.inlevel.setTargetValue(DB_CO(value));
            break;
        case kParameterNETBYPASS:
            aida.net_bypass = value > 0.5f;
            break;
        case kParameterEQBYPASS:
            aida.eq_bypass = value > 0.5f;
            break;
        case kParameterEQPOS:
            aida.eq_pos = value > 0.5f ? kEqPre : kEqPost;
            break;
        case kParameterBASSGAIN:
            aida.bass.setPeakGain(value);
            break;
        case kParameterBASSFREQ:
            aida.bass.setFc(value / sampleRate);
            break;
        case kParameterMIDGAIN:
            aida.mid.setPeakGain(value);
            break;
        case kParameterMIDFREQ:
            aida.mid.setFc(value / sampleRate);
            break;
        case kParameterMIDQ:
            aida.mid.setQ(value);
            break;
        case kParameterMTYPE:
            aida.mid_type = value > 0.5f ? kMidEqBandpass : kMidEqPeak;
            break;
        case kParameterTREBLEGAIN:
            aida.treble.setPeakGain(value);
            break;
        case kParameterTREBLEFREQ:
            aida.treble.setFc(value / sampleRate);
            break;
        case kParameterDEPTH:
            aida.depth.setPeakGain(value);
            break;
        case kParameterPRESENCE:
            aida.presence.setPeakGain(value);
            break;
        case kParameterOUTLEVEL:
            aida.outlevel.setTargetValue(DB_CO(value));
            break;
        case kParameterDCBLOCKER:
            enabledDC = value > 0.5f;
            break;
        default:
            break;
        }
    }
    // END VERBATIM

    // src/aidadsp-plugin.cpp:1003-1057 (run(), minus model, cabinet, meters and bypass)
    void run(const float* const in, float* const out, const uint32_t numSamples)
    {
        // High frequencies roll-off (lowpass)
        if (enabledLPF)
            applyBiquadFilter(aida.in_lpf, out, in, numSamples);
        else
            std::memcpy(out, in, sizeof(float)*numSamples);

        // Pre-gain
        applyGainRamp(aida.inlevel, out, numSamples);

        // Equalizer section
        if (!aida.eq_bypass && aida.eq_pos == kEqPre)
            applyToneControls(aida, out, numSamples);

        // (neural model skipped)

        // DC blocker filter (highpass)
        if (enabledDC)
            applyBiquadFilter(aida.dc_blocker, out, numSamples);

        // (cabinet convolution skipped)

        // Equalizer section
        if (!aida.eq_bypass && aida.eq_pos == kEqPost)
            applyToneControls(aida, out, numSamples);

        // Output volume
        applyGainRamp(aida.outlevel, out, numSamples);
    }
};

// ---------------------------------------------------------------------------------------------------------------------
// Fixture writer

struct Fixture {
    std::vector<float> data;
    std::string index;

    void add(const std::string& name, const std::vector<float>& v)
    {
        char line[256];
        std::snprintf(line, sizeof(line), "%s %zu %zu\n", name.c_str(), data.size(), v.size());
        index += line;
        data.insert(data.end(), v.begin(), v.end());
    }

    bool write(const std::string& dir) const
    {
        const std::string bin = dir + "/biquad_golden.bin";
        const std::string idx = dir + "/biquad_golden.idx";
        FILE* f = std::fopen(bin.c_str(), "wb");
        if (f == nullptr) return false;
        // x86/x64/ARM are little-endian: raw f32 dump is LE.
        std::fwrite(data.data(), sizeof(float), data.size(), f);
        std::fclose(f);
        f = std::fopen(idx.c_str(), "wb");
        if (f == nullptr) return false;
        std::fputs("# TICKET-022 golden reference, generated by tests/cpp_reference/gen_biquad_ref.cpp\n"
                   "# <name> <offset_f32> <len_f32>\n", f);
        std::fputs(index.c_str(), f);
        std::fclose(f);
        return true;
    }
};

// ---------------------------------------------------------------------------------------------------------------------
// Signals (stored in the fixture, Rust reads them back)

static constexpr uint32_t kFilterLen = 1024;
static constexpr uint32_t kChainLen = 4096;
static constexpr uint32_t kBlock = 256; // host block size for chain scenarios

static std::vector<float> makeImpulse(uint32_t n)
{
    std::vector<float> v(n, 0.f);
    v[0] = 1.f;
    return v;
}

static std::vector<float> makeStep(uint32_t n)
{
    return std::vector<float>(n, 0.5f);
}

// Exponential sine sweep 20 Hz .. 20 kHz at 48 kHz, amplitude 0.5.
static std::vector<float> makeSweep(uint32_t n)
{
    std::vector<float> v(n);
    const double f0 = 20.0, f1 = 20000.0, sr = 48000.0;
    const double T = n / sr;
    const double k = std::log(f1 / f0);
    for (uint32_t i=0; i<n; ++i)
    {
        const double t = i / sr;
        v[i] = (float)(0.5 * std::sin(2.0 * M_PI * f0 * T / k * (std::exp(t / T * k) - 1.0)));
    }
    return v;
}

// White noise in [-0.5, 0.5) from a 32-bit LCG (Numerical Recipes constants), seed 0x12345678.
static std::vector<float> makeNoise(uint32_t n)
{
    std::vector<float> v(n);
    uint32_t s = 0x12345678u;
    for (uint32_t i=0; i<n; ++i)
    {
        s = s * 1664525u + 1013904223u;
        v[i] = (float)((double)(s >> 8) / 16777216.0 - 0.5);
    }
    return v;
}

struct Signal { const char* name; std::vector<float> data; };

// ---------------------------------------------------------------------------------------------------------------------
// 1. Single filters

struct FilterCase {
    const char* name;
    int type;
    double fc, q, gain;
    // optional mid-stream change at sample kFilterLen/2 (setter: 0 none, 1 setPeakGain, 2 setFc, 3 setQ)
    int change;
    double changeValue;
};

static const FilterCase kFilterCases[] = {
    { "lowpass_1k_q0707",        bq_type_lowpass,   1000.0/48000.0, 0.707, 0.0, 0, 0 },
    { "lowpass_0.3_q2",          bq_type_lowpass,   0.3,            2.0,   0.0, 0, 0 },
    { "lowpass_inlpf_max",       bq_type_lowpass,   0.495,          0.707, 0.0, 0, 0 },
    { "highpass_35hz",           bq_type_highpass,  35.0/48000.0,   0.707, 0.0, 0, 0 },
    { "highpass_0.1_q0.5",       bq_type_highpass,  0.1,            0.5,   0.0, 0, 0 },
    { "bandpass_750_q0707",      bq_type_bandpass,  750.0/48000.0,  0.707, 0.0, 0, 0 },
    { "bandpass_0.05_q5",        bq_type_bandpass,  0.05,           5.0,   0.0, 0, 0 },
    { "notch_0.02_q1",           bq_type_notch,     0.02,           1.0,   0.0, 0, 0 },
    { "notch_0.2_q0.3",          bq_type_notch,     0.2,            0.3,   0.0, 0, 0 },
    { "peak_boost_75_q0707_+8",  bq_type_peak,      75.0/48000.0,   0.707, 8.0, 0, 0 },
    { "peak_cut_0.1_q5_-8",      bq_type_peak,      0.1,            5.0,  -8.0, 0, 0 },
    { "peak_flat_0.05_q1_0",     bq_type_peak,      0.05,           1.0,   0.0, 0, 0 },
    { "lowshelf_boost_305_+8",   bq_type_lowshelf,  305.0/48000.0,  0.707, 8.0, 0, 0 },
    { "lowshelf_cut_75_-8",      bq_type_lowshelf,  75.0/48000.0,   0.707,-8.0, 0, 0 },
    { "lowshelf_flat_0.01_0",    bq_type_lowshelf,  0.01,           0.707, 0.0, 0, 0 },
    { "highshelf_boost_2k_+8",   bq_type_highshelf, 2000.0/48000.0, 0.707, 8.0, 0, 0 },
    { "highshelf_cut_900_-8",    bq_type_highshelf, 900.0/48000.0,  0.707,-8.0, 0, 0 },
    { "highshelf_boost_0.3_+3",  bq_type_highshelf, 0.3,            0.707, 3.0, 0, 0 },
    // coefficient changes while running (state z1/z2 must carry over)
    { "peak_gain_change",        bq_type_peak,      0.02,           1.5,   6.0, 1, -6.0 },
    { "lowshelf_fc_change",      bq_type_lowshelf,  0.005,          0.707, 5.0, 2, 0.02 },
    { "bandpass_q_change",       bq_type_bandpass,  0.03,           0.5,   0.0, 3, 4.0 },
};

static void runFilterCases(Fixture& fx, const std::vector<Signal>& signals)
{
    for (const FilterCase& c : kFilterCases)
    {
        for (const Signal& s : signals)
        {
            Biquad bq(c.type, c.fc, c.q, c.gain);
            std::vector<float> out(kFilterLen);
            const uint32_t half = kFilterLen / 2;
            for (uint32_t i=0; i<kFilterLen; ++i)
            {
                if (i == half)
                {
                    switch (c.change)
                    {
                    case 1: bq.setPeakGain(c.changeValue); break;
                    case 2: bq.setFc(c.changeValue); break;
                    case 3: bq.setQ(c.changeValue); break;
                    default: break;
                    }
                }
                out[i] = bq.process(s.data[i]);
            }
            fx.add(std::string("filter/") + c.name + "/" + s.name, out);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// 2. Tone-control chain scenarios

static constexpr uint32_t kEvSampleRate = 1000; // pseudo-parameter: call sampleRateChanged(sampleRate)

struct Event { uint32_t at; uint32_t param; float value; };

struct ChainCase {
    const char* name;
    double sampleRate;
    bool activate;                 // call activate() (snaps gain ramps) before processing
    std::vector<Event> init;       // applied to parameters[] BEFORE sampleRateChanged (constructor state)
    std::vector<Event> events;     // applied via setParameterValue at block boundaries (at % kBlock == 0)
};

static std::vector<ChainCase> chainCases()
{
    std::vector<ChainCase> cases;

    // plugin defaults, post-EQ, peak mid, DC blocker on, LPF at 66.216 %
    cases.push_back({ "defaults", 48000.0, true, {}, {} });

    // all bands active, EQ post
    cases.push_back({ "eq_post_all_bands", 48000.0, true,
        { {0, kParameterBASSGAIN, 6.f}, {0, kParameterBASSFREQ, 150.f},
          {0, kParameterMIDGAIN, -4.f}, {0, kParameterMIDFREQ, 1200.f}, {0, kParameterMIDQ, 2.f},
          {0, kParameterTREBLEGAIN, 3.f}, {0, kParameterTREBLEFREQ, 3500.f},
          {0, kParameterDEPTH, 5.f}, {0, kParameterPRESENCE, -6.f} }, {} });

    // same, EQ pre
    cases.push_back({ "eq_pre_all_bands", 48000.0, true,
        { {0, kParameterEQPOS, 1.f},
          {0, kParameterBASSGAIN, -7.f}, {0, kParameterBASSFREQ, 500.f},
          {0, kParameterMIDGAIN, 8.f}, {0, kParameterMIDFREQ, 400.f}, {0, kParameterMIDQ, 0.5f},
          {0, kParameterTREBLEGAIN, -8.f}, {0, kParameterTREBLEFREQ, 1000.f},
          {0, kParameterDEPTH, -3.f}, {0, kParameterPRESENCE, 8.f} }, {} });

    // bandpass mid. AidaToneControl::setSampleRate picks the mid biquad type from the struct's
    // mid_type (set only by setParameterValue), so a real bandpass needs MTYPE first and then a
    // sampleRateChanged, as when a host restores state and then (re)activates the plugin.
    cases.push_back({ "mid_bandpass", 48000.0, true,
        { {0, kParameterMIDFREQ, 1000.f}, {0, kParameterMIDQ, 3.f}, {0, kParameterBASSGAIN, 8.f} },
        { {0, kParameterMTYPE, 1.f}, {0, kEvSampleRate, 0.f} } });

    // C++ quirk: changing MTYPE via setParameterValue only switches the cascade routing
    // (mid alone), the mid biquad keeps its PEAK type until the next sampleRateChanged.
    cases.push_back({ "mid_type_switch_quirk", 48000.0, true,
        { {0, kParameterMIDGAIN, 6.f}, {0, kParameterMIDFREQ, 900.f} },
        { {1024, kParameterMTYPE, 1.f}, {2048, kEvSampleRate, 0.f}, {3072, kParameterMTYPE, 0.f} } });

    // DC blocker off
    cases.push_back({ "dc_blocker_off", 48000.0, true,
        { {0, kParameterDCBLOCKER, 0.f}, {0, kParameterBASSGAIN, 4.f} }, {} });

    // anti-aliasing LPF off (setParameterValue(INLPF, 0) disables it) and at maximum
    cases.push_back({ "lpf_off", 48000.0, true, {}, { {0, kParameterINLPF, 0.f} } });
    cases.push_back({ "lpf_max", 48000.0, true, { {0, kParameterINLPF, 100.f} }, {} });
    cases.push_back({ "lpf_sweep_param", 48000.0, true, {},
        { {1024, kParameterINLPF, 10.f}, {2048, kParameterINLPF, 90.f}, {3072, kParameterINLPF, 50.f} } });

    // EQ bypass
    cases.push_back({ "eq_bypass", 48000.0, true,
        { {0, kParameterEQBYPASS, 1.f}, {0, kParameterBASSGAIN, 8.f} }, {} });

    // gain ramps from cold start (no activate(): smoothers start at 0 and ramp to the target)
    cases.push_back({ "gain_ramp_cold_start", 48000.0, false,
        { {0, kParameterINLEVEL, 6.f}, {0, kParameterOUTLEVEL, -3.f} }, {} });

    // gain + EQ parameter automation at block boundaries
    cases.push_back({ "automation", 48000.0, true,
        { {0, kParameterINLEVEL, -6.f} },
        { {512, kParameterINLEVEL, 12.f}, {768, kParameterOUTLEVEL, -15.f},
          {1024, kParameterBASSGAIN, 8.f}, {1280, kParameterTREBLEFREQ, 4000.f},
          {1536, kParameterMIDQ, 5.f}, {1792, kParameterMIDGAIN, -8.f}, {2048, kParameterEQPOS, 1.f},
          {2304, kParameterBASSFREQ, 75.f}, {2560, kParameterDEPTH, 8.f}, {2816, kParameterPRESENCE, -8.f},
          {3072, kParameterOUTLEVEL, 15.f}, {3328, kParameterINLEVEL, -12.f}, {3584, kParameterDCBLOCKER, 0.f},
          {3840, kParameterMIDFREQ, 5000.f} } });

    // 44.1 kHz
    cases.push_back({ "sr44100_all_bands", 44100.0, true,
        { {0, kParameterBASSGAIN, 3.f}, {0, kParameterMIDGAIN, 5.f}, {0, kParameterTREBLEGAIN, -5.f},
          {0, kParameterDEPTH, 8.f}, {0, kParameterPRESENCE, 2.f}, {0, kParameterOUTLEVEL, 4.f} }, {} });

    return cases;
}

static void runChainCases(Fixture& fx, const std::vector<Signal>& signals)
{
    for (const ChainCase& c : chainCases())
    {
        for (const Signal& s : signals)
        {
            TonePlugin p(c.sampleRate);
            for (const Event& e : c.init)
                p.parameters[e.param] = e.value;
            p.sampleRateChanged(c.sampleRate);
            if (c.activate)
                p.activate();

            std::vector<float> out(kChainLen);
            for (uint32_t pos=0; pos<kChainLen; pos += kBlock)
            {
                for (const Event& e : c.events)
                {
                    if (e.at != pos) continue;
                    if (e.param == kEvSampleRate)
                        p.sampleRateChanged(p.sampleRate);
                    else
                        p.setParameterValue(e.param, e.value);
                }
                p.run(s.data.data() + pos, out.data() + pos, kBlock);
            }
            fx.add(std::string("chain/") + c.name + "/" + s.name, out);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// 3. Smoothers (value sequences)

static constexpr uint32_t kSmootherLen = 12000;

static void runSmootherCases(Fixture& fx)
{
    // Exponential: T60 = 1 s (in/out level), 0.25 s (bypass), 0.1 s (cabsim) at 48 kHz,
    // target 0 -> DB_CO(+6) then -> DB_CO(-15) at 1/3, -> 0 at 2/3.
    const float t60s[] = { 1.f, 0.25f, 0.1f };
    const char* names[] = { "exp_t60_1", "exp_t60_0.25", "exp_t60_0.1" };
    for (int k=0; k<3; ++k)
    {
        ExponentialValueSmoother sm;
        sm.setTimeConstant(t60s[k]);
        sm.setSampleRate(48000.f);
        sm.setTargetValue(DB_CO(6.f));
        std::vector<float> out(kSmootherLen);
        for (uint32_t i=0; i<kSmootherLen; ++i)
        {
            if (i == kSmootherLen/3) sm.setTargetValue(DB_CO(-15.f));
            if (i == 2*kSmootherLen/3) sm.setTargetValue(0.f);
            out[i] = sm.next();
        }
        fx.add(std::string("smoother/") + names[k], out);
    }

    // Exponential at 44.1 kHz, cleared to target then retargeted
    {
        ExponentialValueSmoother sm;
        sm.setSampleRate(44100.f);
        sm.setTimeConstant(1.f);
        sm.setTargetValue(DB_CO(-6.f));
        sm.clearToTargetValue();
        sm.setTargetValue(DB_CO(12.f));
        std::vector<float> out(kSmootherLen);
        for (uint32_t i=0; i<kSmootherLen; ++i)
            out[i] = sm.next();
        fx.add("smoother/exp_t60_1_sr44100_cleared", out);
    }

    // Linear: tau 0.1 s (param1/param2) at 48 kHz, several target changes incl. mid-segment
    {
        LinearValueSmoother sm;
        sm.setTimeConstant(0.1f);
        sm.setSampleRate(48000.f);
        sm.setTargetValue(0.25f);
        sm.clearToTargetValue();
        std::vector<float> out(kSmootherLen);
        for (uint32_t i=0; i<kSmootherLen; ++i)
        {
            if (i == 100) sm.setTargetValue(1.f);
            if (i == 3000) sm.setTargetValue(0.f);   // mid-segment retarget
            if (i == 7000) sm.setTargetValue(0.7f);
            if (i == 10000) sm.setTargetValue(0.7f); // unchanged target: no new segment
            out[i] = sm.next();
        }
        fx.add("smoother/linear_tau_0.1", out);
    }
}

// ---------------------------------------------------------------------------------------------------------------------

int main(int argc, char** argv)
{
    const std::string outDir = argc > 1 ? argv[1] : ".";

    Fixture fx;

    std::vector<Signal> filterSignals = {
        { "impulse", makeImpulse(kFilterLen) },
        { "sweep", makeSweep(kFilterLen) },
        { "noise", makeNoise(kFilterLen) },
    };
    std::vector<Signal> chainSignals = {
        { "impulse", makeImpulse(kChainLen) },
        { "step", makeStep(kChainLen) },
        { "sweep", makeSweep(kChainLen) },
        { "noise", makeNoise(kChainLen) },
    };

    for (const Signal& s : filterSignals)
        fx.add(std::string("input/filter/") + s.name, s.data);
    for (const Signal& s : chainSignals)
        fx.add(std::string("input/chain/") + s.name, s.data);

    runFilterCases(fx, filterSignals);
    runChainCases(fx, chainSignals);
    runSmootherCases(fx);

    if (!fx.write(outDir))
    {
        std::fprintf(stderr, "failed to write fixtures to %s\n", outDir.c_str());
        return 1;
    }
    std::printf("wrote %zu f32 values to %s/biquad_golden.bin\n", fx.data.size(), outDir.c_str());
    return 0;
}
