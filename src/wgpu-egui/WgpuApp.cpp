// AIDA-X WGPU+egui Backend - Application Implementation
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#include "WgpuApp.hpp"
#include "WgpuBackend.hpp"
#include "Widgets.hpp"

#include <cstdio>
#include <cmath>
#include <algorithm>
#include <memory>
#include <chrono>

#ifdef _WIN32
#include <windows.h>
#include <GLFW/glfw3.h>
#elif defined(__linux__)
#include <GLFW/glfw3.h>
#elif defined(__APPLE__)
#include <GLFW/glfw3.h>
#endif

enum {
    kParamINLevel    = 1,
    kParamEQPos      = 4,
    kParamBassGain   = 5,
    kParamMidGain    = 7,
    kParamTrebleGain = 11,
    kParamMTYPE      = 10,
    kParamDEPTH      = 13,
    kParamPRESENCE   = 14,
    kParamOUTLEVEL   = 15,
    kParamCABSIMBYPASS = 16,
    kParamGLOBALBYPASS = 17,
    kParamNETBYPASS  = 2,
    kParamMeterIn    = 22,
    kParamMeterOut   = 23,
};

WgpuApp::~WgpuApp() {
    shutdown();
}

bool WgpuApp::init(const Config& config) {
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        return false;
    }

    glfwWindowHint(GLFW_CLIENT_API, GLFW_NO_API);
    glfwWindowHint(GLFW_RESIZABLE, config.resizable ? GLFW_TRUE : GLFW_FALSE);

    GLFWwindow* window = glfwCreateWindow(config.width, config.height, config.title, nullptr, nullptr);
    if (!window) {
        fprintf(stderr, "Failed to create window\n");
        glfwTerminate();
        return false;
    }

    m_window = window;
    m_window_width = config.width;
    m_window_height = config.height;
    m_scale = config.dpr;

    glfwSetWindowUserPointer(window, this);

    glfwSetFramebufferSizeCallback(window, [](GLFWwindow* w, int width, int height) {
        auto* app = static_cast<WgpuApp*>(glfwGetWindowUserPointer(w));
        if (app) {
            int fbw, fbh;
            glfwGetFramebufferSize(w, &fbw, &fbh);
            app->m_window_width = fbw;
            app->m_window_height = fbh;
            double wpw, wpd;
            glfwGetWindowSize(w, &wpw, &wpd);
            if (wpw > 0 && wpd > 0) {
                app->m_scale = static_cast<float>(fbw / wpw);
            }
        }
    });

    glfwSetMouseButtonCallback(window, [](GLFWwindow* w, int button, int action, int mods) {
        auto* app = static_cast<WgpuApp*>(glfwGetWindowUserPointer(w));
        if (!app) return;
        auto now = app->m_time;
        if (button == GLFW_MOUSE_BUTTON_LEFT) {
            bool pressed = (action == GLFW_PRESS);
            if (pressed) {
                float dx = app->m_input.mouse_pos.x - app->m_input.last_click_pos.x;
                float dy = app->m_input.mouse_pos.y - app->m_input.last_click_pos.y;
                float dist = std::sqrt(dx * dx + dy * dy);
                app->m_input.shift = ((mods & GLFW_MOD_SHIFT) != 0);
                app->m_input.ctrl = ((mods & GLFW_MOD_CONTROL) != 0);

                if (dist < 5.0f && (now - app->m_input.last_click_time) < app->m_double_click_threshold) {
                    app->m_input.mouse_down[1] = true; // double click
                } else {
                    app->m_input.mouse_down[0] = true;
                }
                app->m_input.last_click_time = now;
                app->m_input.last_click_pos = app->m_input.mouse_pos;
            } else {
                app->m_input.mouse_down[0] = false;
                app->m_input.mouse_down[1] = false;
            }
        }
        if (button == GLFW_MOUSE_BUTTON_RIGHT && action != GLFW_RELEASE) {
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
            app->m_last_scroll_delta += static_cast<float>(yoffset);
        }
    });

    glfwSetKeyCallback(window, [](GLFWwindow* w, int key, int scancode, int action, int mods) {
        auto* app = static_cast<WgpuApp*>(glfwGetWindowUserPointer(w));
        if (app && action != GLFW_RELEASE) {
            app->m_input.ctrl = ((mods & GLFW_MOD_CONTROL) != 0);
            app->m_input.shift = ((mods & GLFW_MOD_SHIFT) != 0);
        }
    });

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
    while (m_running && !glfwWindowShouldClose(static_cast<GLFWwindow*>(m_window))) {
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

    m_time += 1.0 / 60.0;

    int w, h;
    glfwGetFramebufferSize(static_cast<GLFWwindow*>(m_window), &w, &h);
    if (w != static_cast<int>(m_window_width) || h != static_cast<int>(m_window_height)) {
        m_backend.set_size(static_cast<uint32_t>(w), static_cast<uint32_t>(h));
        m_window_width = w;
        m_window_height = h;
    }

    m_gui.begin(static_cast<float>(w), static_cast<float>(h));

    float s = m_scale;
    float pw = 772.0f * s;  // pedal width
    float ph = 442.0f * s;  // pedal height
    float mh = 20.0f * s;   // margin horizontal
    float mv = 20.0f * s;   // margin vertical
    float hh = 177.0f * s;  // head height
    float px = 0.0f;
    float py = mv + (static_cast<float>(h) - ph) / 2.0f;

    // === Pedal background ===
    Color pedal_dark = Color::from_rgba(0x1c, 0x17, 0x0c);
    Color pedal_light = Color::from_rgba(0x2a, 0x22, 0x0f);
    m_gui.rects.clear();
    m_gui.lines.clear();
    m_gui.texts.clear();

    rect(m_gui, {px, py, pw, ph}, pedal_dark);
    rect(m_gui, {px + pw / 2.0f, py, pw / 2.0f, ph}, pedal_light);

    Color border = Color::from_rgba(150, 150, 150, 0.25f);
    line(m_gui, {px, py}, {px + pw, py}, border, 1.0f);
    line(m_gui, {px, py + ph}, {px + pw, py + ph}, border, 1.0f);
    line(m_gui, {px, py}, {px, py + ph}, border, 1.0f);
    line(m_gui, {px + pw, py}, {px + pw, py + ph}, border, 1.0f);

    // Head section
    Color head_green = Color::from_rgba(0x8b, 0xf7, 0x00);
    head_green.g = 0.45f;
    head_green.b = 0.0f;
    rect(m_gui, {px + mh, py + mh, pw - mh * 2.0f, hh}, head_green);

    // Brand text
    text_centered(m_gui, px + pw / 2.0f, py + hh * 0.35f, "AIDA-X", 28.0f * s, Color::from_rgba(0xe0, 0xe0, 0xe0));
    text_centered(m_gui, px + pw / 2.0f, py + hh * 0.55f, "AI CRAFTED TONE", 14.0f * s, Color::from_hsl(0.3f, 0.9f, 0.15f).with_alpha(0.7f));

    // === Control row ===
    float control_y = py + hh + 15.0f * s;
    float knob_size = AidaKnob::kScaleSize * s;
    float switch_w = AidaSwitch::kFullWidth * s;
    float splitter_w = AidaSplitter::kLineWidth * s;

    float unused = pw - (knob_size * 7.0f) - (switch_w * 3.0f) - (splitter_w * 3.0f) - mh * 2.0f;
    float padding = unused / 14.0f;
    if (padding < 4.0f * s) padding = 4.0f * s;

    float control_start = px + mh + 5.0f * s;

    // Bypass switch
    AidaSwitch bypass_sw(control_start, control_y, s,
        static_cast<uint32_t>(kParamGLOBALBYPASS),
        "OFF", "ON",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 1.0f : 0.0f;
            m_bypass_global = !on;
            if (m_bypass_cb) m_bypass_cb(param, on);
        });
    bypass_sw.set_checked(!m_bypass_global);

    // IN Level knob
    AidaKnob inlevel(control_start + switch_w + padding, control_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        });
    inlevel.set_range(0.0f, 1.0f);
    inlevel.set_value(m_parameters[kParamINLevel]);
    inlevel.set_label("INPUT");

    // EQ Position switch
    AidaSwitch eqpos_sw(control_start + switch_w + knob_size + padding * 2, control_y, s,
        static_cast<uint32_t>(kParamEQPos),
        "PRE", "POST",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 1.0f : 0.0f;
            m_eq_post = on;
        });
    eqpos_sw.set_checked(m_eq_post);

    // MID Type switch
    AidaSwitch midtype_sw(control_start + switch_w * 2 + knob_size + padding * 3, control_y, s,
        static_cast<uint32_t>(kParamMTYPE),
        "PEAK", "BAND",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 1.0f : 0.0f;
            m_mid_bandpass = on;
        });
    midtype_sw.set_checked(m_mid_bandpass);

    // First group of knobs (Bass, Mid, Treble)
    float first_group_x = control_start + switch_w * 3 + splitter_w + padding * 2;

    AidaKnob bass(first_group_x, control_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        });
    bass.set_range(0.0f, 1.0f);
    bass.set_value(m_parameters[kParamBassGain]);
    bass.set_label("BASS");

    AidaKnob middle(first_group_x + knob_size + padding, control_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        });
    middle.set_range(0.0f, 1.0f);
    middle.set_value(m_parameters[kParamMidGain]);
    middle.set_label("MID");

    AidaKnob treble(first_group_x + knob_size * 2 + padding * 2, control_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        });
    treble.set_range(0.0f, 1.0f);
    treble.set_value(m_parameters[kParamTrebleGain]);
    treble.set_label("TREBLE");

    AidaSplitter spl1(first_group_x + knob_size * 3 + splitter_w + padding * 3, control_y);

    // Second group of knobs (Depth, Presence)
    AidaKnob depth(first_group_x + knob_size * 3 + splitter_w + padding * 4, control_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        });
    depth.set_range(0.0f, 1.0f);
    depth.set_value(m_parameters[kParamDEPTH]);
    depth.set_label("DEPTH");

    AidaKnob presence(first_group_x + knob_size * 3 + splitter_w + knob_size + padding * 5, control_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        });
    presence.set_range(0.0f, 1.0f);
    presence.set_value(m_parameters[kParamPRESENCE]);
    presence.set_label("PRESENCE");

    AidaSplitter spl2(first_group_x + knob_size * 3 + splitter_w * 2 + padding * 6, control_y);

    // Output knob
    AidaKnob outlevel(first_group_x + knob_size * 3 + splitter_w * 2 + knob_size + padding * 7, control_y, s,
        [this](uint32_t param, float val) {
            m_parameters[param] = val;
            if (m_param_changed) m_param_changed(param, val);
        });
    outlevel.set_range(0.0f, 1.0f);
    outlevel.set_value(m_parameters[kParamOUTLEVEL]);
    outlevel.set_label("OUTPUT");

    // === Meters ===
    float meter_x = px + mh - AidaMeter::kMeterWidth - 10.0f * s;
    float meter_y = py - AidaMeter::kMeterHeight - 5.0f * s;

    AidaMeter in_meter(meter_x - AidaMeter::kMeterWidth - 10.0f * s, meter_y, "INPUT");
    AidaMeter out_meter(meter_x, meter_y, "OUTPUT");
    in_meter.set_scale(s);
    out_meter.set_scale(s);

    in_meter.set_db(m_parameters[kParamMeterIn] * 100.0f - 70.0f);
    out_meter.set_db(m_parameters[kParamMeterOut] * 100.0f - 70.0f);

    // === File loader buttons ===
    float loader_x = px + pw * 2.0f / 3.0f;
    float loader_y = py + ph - hh - 10.0f * s;

    AidaFilenameButton model_btn(loader_x, loader_y - 38.0f * s, s,
        static_cast<uint32_t>(kParamNETBYPASS), "Amp Model",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 0.0f : 1.0f;
            m_bypass_model = !on;
            if (m_bypass_cb) m_bypass_cb(param, on);
        },
        [this](const char* path) {
            if (m_file_loaded) m_file_loaded(path);
            m_model_path = path;
        });
    model_btn.set_filename(m_last_model_file);

    AidaFilenameButton cab_btn(loader_x, loader_y, s,
        static_cast<uint32_t>(kParamCABSIMBYPASS), "Cabinet IR",
        [this](uint32_t param, bool on) {
            m_parameters[param] = on ? 0.0f : 1.0f;
            m_bypass_cab = !on;
            if (m_bypass_cb) m_bypass_cb(param, on);
        },
        [this](const char* path) {
            if (m_file_loaded) m_file_loaded(path);
            m_cab_path = path;
        });
    cab_btn.set_filename(m_last_cab_file);

    // === Process input ===
    float mpx = m_input.mouse_pos.x;
    float mpy = m_input.mouse_pos.y;

    bypass_sw.handle_input(m_input, mpx, mpy);
    inlevel.handle_input(m_input, mpx, mpy);
    eqpos_sw.handle_input(m_input, mpx, mpy);
    midtype_sw.handle_input(m_input, mpx, mpy);
    bass.handle_input(m_input, mpx, mpy);
    middle.handle_input(m_input, mpx, mpy);
    treble.handle_input(m_input, mpx, mpy);
    depth.handle_input(m_input, mpx, mpy);
    presence.handle_input(m_input, mpx, mpy);
    outlevel.handle_input(m_input, mpx, mpy);
    model_btn.handle_input(m_input, mpx, mpy);
    cab_btn.handle_input(m_input, mpx, mpy);

    // Reset input state for next frame
    m_input.mouse_down[0] = false;
    m_input.mouse_down[1] = false;
    m_input.mouse_down[2] = false;
    m_input.scroll += m_last_scroll_delta;
    m_last_scroll_delta = 0.0f;

    // === Render everything ===
    // Pedal header
    rect(m_gui, {px + mh, py + mh, pw - mh * 2.0f, hh}, Color::from_rgba(0x8b, 0xf7, 0x00).with_alpha(0.15f));

    // Draw widgets
    bypass_sw.render(m_gui);
    inlevel.render(m_gui);
    eqpos_sw.render(m_gui);
    midtype_sw.render(m_gui);
    bass.render(m_gui);
    middle.render(m_gui);
    treble.render(m_gui);
    spl1.render(m_gui);
    depth.render(m_gui);
    presence.render(m_gui);
    spl2.render(m_gui);
    outlevel.render(m_gui);
    in_meter.render(m_gui);
    out_meter.render(m_gui);
    model_btn.render(m_gui);
    cab_btn.render(m_gui);

    // Update WGPU
    m_backend.poll_events();
    if (m_backend.begin_frame()) {
        m_backend.end_frame(m_gui);
    }

    glfwSwapBuffers(static_cast<GLFWwindow*>(m_window));
}
