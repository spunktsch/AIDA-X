// AIDA-X WGPU+egui Backend - Application Implementation
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#include "WgpuApp.hpp"
#include "Widgets.hpp"
#include "Gui.hpp"
#include "DistrhoPluginCommon.hpp"

#include <cstdio>
#include <cmath>
#include <algorithm>
#include <memory>

#ifdef _WIN32
#include <windows.h>
#include <GLFW/glfw3.h>
#elif defined(__linux__)
#include <GLFW/glfw3.h>
#elif defined(__APPLE__)
#include <GLFW/glfw3.h>
#endif

// Parameter indices from DistrhoPluginCommon.hpp (enum Parameters)
enum {
    kParamBypassModel    = kParameterNETBYPASS,    // 2
    kParamINLevel        = kParameterINLEVEL,       // 1
    kParamGlobalBypass   = kParameterGLOBALBYPASS,  // 17
    kParamEQPos          = kParameterEQPOS,         // 4
    kParamBassGain       = kParameterBASSGAIN,      // 5
    kParamMidGain        = kParameterMIDGAIN,       // 7
    kParamMidType        = kParameterMTYPE,         // 10
    kParamTrebleGain     = kParameterTREBLEGAIN,    // 11
    kParamDepth          = kParameterDEPTH,         // 13
    kParamPresence       = kParameterPRESENCE,      // 14
    kParamCabBypass      = kParameterCABSIMBYPASS,  // 16
    kParamOutLevel       = kParameterOUTLEVEL,      // 15
    kParamMeterIn        = kParameterMeterIn,       // 22
    kParamMeterOut       = kParameterMeterOut,      // 23
};

// ============================================================================
// WgpuApp implementation
// ============================================================================

WgpuApp::~WgpuApp() {
    shutdown();
}

bool WgpuApp::init(const Config& config) {
    // Initialize GLFW
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        return false;
    }

    glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API);
    glfwWindowHint(GLFW_RESIZABLE, config.resizable ? GLFW_TRUE : GLFW_FALSE);

    // Create window
    GLFWwindow* window = glfwCreateWindow(
        config.width, config.height, config.title, nullptr, nullptr);
    if (!window) {
        fprintf(stderr, "Failed to create window\n");
        glfwTerminate();
        return false;
    }

    m_window = window;
    m_window_width = config.width;
    m_window_height = config.height;
    m_scale = config.dpr;

    // Set up GLFW callbacks
    glfwSetWindowUserPointer(window, this);
    glfwSetFramebufferSizeCallback(window, [](GLFWwindow* w, int width, int height) {
        auto* app = static_cast<WgpuApp*>(glfwGetWindowUserPointer(w));
        if (app) {
            app->m_window_width = width;
            app->m_window_height = height;
        }
    });

    glfwSetMouseButtonCallback(window, [](GLFWwindow* w, int button, int action, int mods) {
        auto* app = static_cast<WgpuApp*>(glfwGetWindowUserPointer(w));
        if (app && button == GLFW_MOUSE_BUTTON_LEFT) {
            app->m_input.mouse_down[0] = (action == GLFW_PRESS);
        }
        if (app && button == GLFW_MOUSE_BUTTON_RIGHT) {
            app->m_input.mouse_down[2] = (action == GLFW_PRESS);
        }
    });

    glfwSetCursorPosCallback(window, [](GLFWwindow* w, double x, double y) {
        auto* app = static_cast<WgpuApp*>(glfwGetWindowUserPointer(w));
        if (app) {
            app->m_input.mouse_pos = {static_cast<float>(x), static_cast<float>(y)};
        }
    });

    glfwSetScrollCallback(window, [](GLFWwindow* w, double xoffset, double yoffset) {
        auto* app = static_cast<WgpuApp*>(glfwGetWindowUserPointer(w));
        if (app) {
            app->m_input.scroll += static_cast<float>(yoffset);
        }
    });

    glfwSetKeyCallback(window, [](GLFWwindow* w, int key, int scancode, int action, int mods) {
        auto* app = static_cast<WgpuApp*>(glfwGetWindowUserPointer(w));
        if (app && action != GLFW_RELEASE) {
            app->m_input.ctrl = (mods & GLFW_MOD_CONTROL) != 0;
            app->m_input.shift = (mods & GLFW_MOD_SHIFT) != 0;
        }
    });

    // Initialize WGPU backend
    WgpuConfig wgpu_config;
    wgpu_config.width = config.width;
    wgpu_config.height = config.height;
    wgpu_config.title = config.title;

    if (!m_backend.init(wgpu_config)) {
        fprintf(stderr, "Failed to initialize WGPU backend\n");
        glfwDestroyWindow(window);
        glfwTerminate();
        return false;
    }

    m_running = true;
    return true;
}

void WgpuApp::run() {
    if (!m_running) return;

    while (!m_running && !glfwWindowShouldClose(static_cast<GLFWwindow*>(m_window))) {
        process_events();
        render_frame();
    }

    shutdown();
}

void WgpuApp::shutdown() {
    if (m_window) {
        GLFWwindow* window = static_cast<GLFWwindow*>(m_window);
        glfwDestroyWindow(window);
        m_window = nullptr;
    }
    m_running = false;
    glfwTerminate();
}

void WgpuApp::process_events() {
    if (m_window) {
        glfwPollEvents();
    }
}

void WgpuApp::render_frame() {
    if (!m_window) return;

    // Check window size
    int w, h;
    glfwGetFramebufferSize(static_cast<GLFWwindow*>(m_window), &w, &h);
    if (w != static_cast<int>(m_window_width) || h != static_cast<int>(m_window_height)) {
        m_backend.set_size(static_cast<uint32_t>(w), static_cast<uint32_t>(h));
        m_window_width = w;
        m_window_height = h;
    }

    // Initialize GUI context
    m_gui.begin(static_cast<float>(w), static_cast<float>(h));

    // Scale factors
    float s = m_scale;
    float pedal_w = 772.0f * s;
    float pedal_h = 442.0f * s;
    float margin_h = 20.0f * s;
    float margin_v = 20.0f * s;
    float head_h = 177.0f * s;
    float knob_size = AidaGui::AidaKnob::kScaleSize * s;
    float switch_w = AidaGui::AidaSwitch::kFullWidth * s;
    float splitter_w = AidaGui::AidaSplitter::kLineWidth * s;
    float font_size = 14.0f * s;

    // Layout constants matching original
    float head_x = margin_h;
    float head_y = margin_v;
    float pedal_x = 0.0f;
    float pedal_y = margin_v + (h - pedal_h) / 2.0f;

    // === Draw pedal background ===
    // Outer gradient background
    Color outer_bg = Color::from_rgba(0xcd, 0xff, 0x05).r = 0.08f,
                              outer_bg.g = 0.08f, outer_bg.b = 0.08f;
    rect({0, 0, static_cast<float>(w), static_cast<float>(h)}, outer_bg);

    // Pedal body gradient (left half - darker)
    Color pedal_left = Color::from_rgba(0x1c, 0x17, 0x0c);
    Color pedal_right = Color::from_rgba(0x2a, 0x22, 0x0f);
    rect({pedal_x, pedal_y, pedal_w / 2.0f, pedal_h}, pedal_left);
    rect({pedal_x + pedal_w / 2.0f, pedal_y, pedal_w / 2.0f, pedal_h}, pedal_right);

    // Pedal border
    Color border = Color::from_rgba(150, 150, 150, 0.25f);
    line({pedal_x, pedal_y}, {pedal_x + pedal_w, pedal_y}, border, 1.0f);
    line({pedal_x, pedal_y + pedal_h}, {pedal_x + pedal_w, pedal_y + pedal_h}, border, 1.0f);
    line({pedal_x, pedal_y}, {pedal_x, pedal_y + pedal_h}, border, 1.0f);
    line({pedal_x + pedal_w, pedal_y}, {pedal_x + pedal_w, pedal_y + pedal_h}, border, 1.0f);

    // Head section (green area)
    float head_bg = Color::from_rgba(0x8b, 0xf7, 0x00);
    head_bg.g = 0.6f; // Darker green for pedal head
    rect({head_x, head_y, pedal_w - margin_h * 2.0f, head_h}, head_bg);

    // Brand text: "AIDA-X"
    text({pedal_x + pedal_w / 2.0f - 60.0f * s, head_y + 10.0f * s, 120.0f * s, 28.0f * s},
         "AIDA-X", 28.0f * s, Color::from_rgba(0xe0, 0xe0, 0xe0));

    // Tagline
    text({pedal_x + pedal_w / 2.0f - 50.0f * s, head_y + 35.0f * s, 100.0f * s, 14.0f * s},
         "AI CRAFTED TONE", 14.0f * s, Color::from_rgba(0x0c, 0x2f, 0x03, 0.7f));

    // === Control row ===
    float control_y = head_y + head_h + 15.0f * s;
    float control_h = 75.0f * s;
    float unused_space = pedal_w
        - (knob_size * 7.0f)
        - (switch_w * 3.0f)
        - (splitter_w * 3.0f);
    float padding = unused_space / 14.0f;

    // === Create widgets ===
    // Bypass switch
    std::unique_ptr<AidaGui::AidaSwitch> bypass_sw(new AidaGui::AidaSwitch(
        pedal_x + margin_h + 5.0f * s, control_y, s,
        kParamGlobalBypass, "OFF", "ON",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 1.0f : 0.0f;
            m_bypass_global = !on;
            if (m_bypass_cb) m_bypass_cb(param, on);
        }
    ));
    bypass_sw->set_checked(!m_bypass_global);

    // IN Level knob
    std::unique_ptr<AidaGui::AidaKnob> inlevel(new AidaGui::AidaKnob(
        pedal_x + margin_h + 5.0f * s + switch_w + padding, control_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        }
    ));
    inlevel->set_range(0.0f, 1.0f);
    inlevel->set_value(m_parameters[kParamINLevel]);
    inlevel->set_label("INPUT");

    // EQ Position switch
    std::unique_ptr<AidaGui::AidaSwitch> eqpos_sw(new AidaGui::AidaSwitch(
        pedal_x + margin_h + 5.0f * s + switch_w + knob_size + padding * 2, control_y, s,
        kParamEQPos, "PRE", "POST",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 1.0f : 0.0f;
            m_eq_post = on;
        }
    ));
    eqpos_sw->set_checked(m_eq_post);

    // MID Type switch
    std::unique_ptr<AidaGui::AidaSwitch> midtype_sw(new AidaGui::AidaSwitch(
        pedal_x + margin_h + 5.0f * s + switch_w * 2 + knob_size + padding * 3, control_y, s,
        kParamMidType, "PEAK", "BAND",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 1.0f : 0.0f;
            m_mid_bandpass = on;
        }
    ));
    midtype_sw->set_checked(m_mid_bandpass);

    // Knobs row
    float knobs_start = pedal_x + margin_h + 5.0f * s + switch_w * 3 + splitter_w * 3 + padding * 2;
    float knobs_y = control_y;

    std::unique_ptr<AidaGui::AidaKnob> bass(new AidaGui::AidaKnob(
        knobs_start, knobs_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        }
    ));
    bass->set_range(0.0f, 1.0f);
    bass->set_value(m_parameters[kParamBassGain]);
    bass->set_label("BASS");

    std::unique_ptr<AidaGui::AidaKnob> middle(new AidaGui::AidaKnob(
        knobs_start + knob_size + padding, knobs_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        }
    ));
    middle->set_range(0.0f, 1.0f);
    middle->set_value(m_parameters[kParamMidGain]);
    middle->set_label("MID");

    std::unique_ptr<AidaGui::AidaKnob> treble(new AidaGui::AidaKnob(
        knobs_start + knob_size * 2 + padding * 2, knobs_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        }
    ));
    treble->set_range(0.0f, 1.0f);
    treble->set_value(m_parameters[kParamTrebleGain]);
    treble->set_label("TREBLE");

    // Splitter
    std::unique_ptr<AidaGui::AidaSplitter> splitter1(new AidaGui::AidaSplitter(
        knobs_start + knob_size * 3 + padding * 3, knobs_y));

    std::unique_ptr<AidaGui::AidaKnob> depth(new AidaGui::AidaKnob(
        knobs_start + knob_size * 3 + splitter_w + padding * 4, knobs_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        }
    ));
    depth->set_range(0.0f, 1.0f);
    depth->set_value(m_parameters[kParamDepth]);
    depth->set_label("DEPTH");

    std::unique_ptr<AidaGui::AidaKnob> presence(new AidaGui::AidaKnob(
        knobs_start + knob_size * 3 + splitter_w + knob_size + padding * 5, knobs_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        }
    ));
    presence->set_range(0.0f, 1.0f);
    presence->set_value(m_parameters[kParamPresence]);
    presence->set_label("PRESENCE");

    std::unique_ptr<AidaGui::AidaSplitter> splitter2(new AidaGui::AidaSplitter(
        knobs_start + knob_size * 3 + splitter_w * 2 + padding * 6, knobs_y));

    std::unique_ptr<AidaGui::AidaKnob> outlevel(new AidaGui::AidaKnob(
        knobs_start + knob_size * 3 + splitter_w * 2 + knob_size + padding * 7, knobs_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        }
    ));
    outlevel->set_range(0.0f, 1.0f);
    outlevel->set_value(m_parameters[kParamOutLevel]);
    outlevel->set_label("OUTPUT");

    // === Meter section (left side) ===
    float meter_x = pedal_x + 5.0f * s;
    float meter_y = margin_v - 10.0f * s;

    std::unique_ptr<AidaGui::AidaMeter> in_meter(new AidaGui::AidaMeter(meter_x, meter_y, "INPUT"));
    std::unique_ptr<AidaGui::AidaMeter> out_meter(new AidaGui::AidaMeter(
        meter_x + AidaGui::AidaMeter::kMeterWidth + 10.0f * s, meter_y, "OUTPUT"));

    // Update meters from parameters
    in_meter->set_db(m_parameters[kParamMeterIn] * 100.0f - 70.0f);
    out_meter->set_db(m_parameters[kParamMeterOut] * 100.0f - 70.0f);

    // === Filename buttons (bottom right) ===
    float loader_x = pedal_x + pedal_w * 2.0f / 3.0f;
    float loader_y = pedal_y + pedal_h - head_h - 10.0f * s;

    std::unique_ptr<AidaGui::AidaFilenameButton> model_btn(new AidaGui::AidaFilenameButton(
        loader_x, loader_y - 20.0f * s, s,
        kParamBypassModel, "Amp Model",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 0.0f : 1.0f;
            m_bypass_model = !on;
            if (m_bypass_cb) m_bypass_cb(param, on);
        },
        [this](const char* path, bool is_model) {
            if (m_file_loaded) m_file_loaded(path, is_model);
            if (is_model) {
                m_model_path = path;
            }
        }
    ));
    model_btn->set_filename(m_model_path);

    std::unique_ptr<AidaGui::AidaFilenameButton> cab_btn(new AidaGui::AidaFilenameButton(
        loader_x, loader_y, s,
        kParamCabBypass, "Cabinet IR",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 0.0f : 1.0f;
            m_bypass_cab = !on;
            if (m_bypass_cb) m_bypass_cb(param, on);
        },
        [this](const char* path, bool is_model) {
            if (m_file_loaded) m_file_loaded(path, is_model);
        }
    ));
    cab_btn->set_filename(m_cab_path);

    // === Render widgets (process input first, then draw) ===
    float px = m_input.mouse_pos.x;
    float py = m_input.mouse_pos.y;

    bypass_sw->handle_input(m_input, px, py);
    inlevel->handle_input(m_input, px, py);
    eqpos_sw->handle_input(m_input, px, py);
    midtype_sw->handle_input(m_input, px, py);
    bass->handle_input(m_input, px, py);
    middle->handle_input(m_input, px, py);
    treble->handle_input(m_input, px, py);
    depth->handle_input(m_input, px, py);
    presence->handle_input(m_input, px, py);
    outlevel->handle_input(m_input, px, py);
    model_btn->handle_input(m_input, px, py);
    cab_btn->handle_input(m_input, px, py);
    in_meter->handle_input(m_input, px, py);
    out_meter->handle_input(m_input, px, py);

    // Reset input for next frame
    m_input.mouse_down[0] = false;
    m_input.mouse_down[2] = false;
    m_input.scroll = 0.0f;

    // Now render everything
    bypass_sw->render(m_gui);
    inlevel->render(m_gui);
    eqpos_sw->render(m_gui);
    midtype_sw->render(m_gui);
    bass->render(m_gui);
    middle->render(m_gui);
    treble->render(m_gui);
    splitter1->render(m_gui);
    depth->render(m_gui);
    presence->render(m_gui);
    splitter2->render(m_gui);
    outlevel->render(m_gui);
    in_meter->render(m_gui);
    out_meter->render(m_gui);
    model_btn->render(m_gui);
    cab_btn->render(m_gui);

    // Update WGPU backend
    m_backend.poll_events();
    if (m_backend.begin_frame()) {
        // TODO: Create mesh from GUI shapes and render
        m_backend.end_frame(m_gui, AidaGui::Mesh{});
    }

    // Swap buffers
    glfwSwapBuffers(static_cast<GLFWwindow*>(m_window));
}
