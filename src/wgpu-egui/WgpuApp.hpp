// AIDA-X WGPU+egui Backend - Application Window
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "Gui.hpp"
#include <functional>
#include <string>
#include <memory>

class WgpuApp {
public:
    struct Config {
        uint32_t width = 900;
        uint32_t height = 340;
        const char* title = "AIDA-X";
        bool resizable = true;
        float dpr = 1.0f;
    };

    ~WgpuApp();

    // Initialize the WGPU backend with GLFW
    bool init(const Config& config);

    // Run the main event loop
    void run();

    // Shutdown
    void shutdown();

    // Get backend for rendering
    WgpuBackend& backend() { return m_backend; }
    const WgpuBackend& backend() const { return m_backend; }

    // GUI context
    GuiContext& gui() { return m_gui; }
    const GuiContext& gui() const { return m_gui; }

    // Input state
    InputState& input() { return m_input; }
    const InputState& input() const { return m_input; }

    // Callbacks for widget interactions
    using ParamChangedCallback = std::function<void(uint32_t param_index, float value)>;
    using FileLoadedCallback = std::function<void(const char* path, bool is_model)>;
    using BypassCallback = std::function<void(uint32_t param_index, bool on)>;

    void set_param_changed_callback(ParamChangedCallback cb) { m_param_changed = std::move(cb); }
    void set_file_loaded_callback(FileLoadedCallback cb) { m_file_loaded = std::move(cb); }
    void set_bypass_callback(BypassCallback cb) { m_bypass_cb = std::move(cb); }

    // Parameter state (shared with DSP)
    float m_parameters[16] = {};
    bool m_bypass_model = false;
    bool m_bypass_cab = false;
    bool m_bypass_global = false;
    bool m_eq_post = false;
    bool m_mid_bandpass = false;
    const char* m_model_path = "";
    const char* m_cab_path = "";

    // Scale factor for high-DPI displays
    float scale() const { return m_scale; }

private:
    void process_events();
    void render_frame();

    WgpuBackend m_backend;
    GuiContext m_gui;
    InputState m_input;
    float m_scale = 1.0f;
    bool m_running = false;

    ParamChangedCallback m_param_changed;
    FileLoadedCallback m_file_loaded;
    BypassCallback m_bypass_cb;

    // Window handle (platform-dependent)
    void* m_window = nullptr;
    uint32_t m_window_width = 0;
    uint32_t m_window_height = 0;
};
