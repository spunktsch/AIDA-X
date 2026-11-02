// AIDA-X WGPU+egui Backend - Application Window
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "Gui.hpp"
#include "WgpuBackend.hpp"
#include <functional>
#include <string>
#include <memory>

class WgpuApp {
public:
    struct Config {
        uint32_t width = 900;
        uint32_t height = 442;
        const char* title = "AIDA-X";
        bool resizable = true;
        float dpr = 1.0f;
    };

    ~WgpuApp();

    bool init(const Config& config);
    void run();
    void shutdown();

    WgpuBackend& backend() { return m_backend; }
    const WgpuBackend& backend() const { return m_backend; }
    GuiContext& gui() { return m_gui; }
    const GuiContext& gui() const { return m_gui; }
    InputState& input() { return m_input; }
    const InputState& input() const { return m_input; }

    void set_param_changed_callback(ParamChangedCallback cb) { m_param_changed = std::move(cb); }
    void set_file_loaded_callback(FileLoadedCallback cb) { m_file_loaded = std::move(cb); }
    void set_bypass_callback(BypassCallback cb) { m_bypass_cb = std::move(cb); }

    float m_parameters[24] = {};
    bool m_bypass_model = false;
    bool m_bypass_cab = false;
    bool m_bypass_global = false;
    bool m_eq_post = false;
    bool m_mid_bandpass = false;
    const char* m_model_path = "";
    const char* m_cab_path = "";
    const char* m_last_model_file = "aida.json";
    const char* m_last_cab_file = "aida_ir.wav";

    float scale() const { return m_scale; }
    void set_scale(float s) { m_scale = s; }

    double get_time() const { return m_time; }

private:
    void process_events();
    void render_frame();

    WgpuBackend m_backend;
    GuiContext m_gui;
    InputState m_input;
    float m_scale = 1.0f;
    bool m_running = false;
    double m_time = 0.0;
    float m_last_scroll_delta = 0.0f;
    float m_double_click_threshold = 0.3f;

    ParamChangedCallback m_param_changed;
    FileLoadedCallback m_file_loaded;
    BypassCallback m_bypass_cb;

    void* m_window = nullptr;
    uint32_t m_window_width = 0;
    uint32_t m_window_height = 0;
};
