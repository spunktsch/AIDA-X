// AIDA-X WGPU+egui Standalone Application
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

// This is the entry point for the WGPU+egui standalone version of AIDA-X.
// It replicates the original standalone application but uses a WGPU+egui
// rendering backend instead of DPF/DGL with OpenGL/NanoVG.

#include "WgpuApp.hpp"
#include "WgpuBackend.hpp"
#include "Gui.hpp"
#include "Widgets.hpp"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>

// Simple audio engine stub - in production this would link against RTNeural
// For now, we just provide the UI framework

static const char* kDefaultModelName = "aida.json";
static const char* kDefaultCabinetName = "aida_ir.wav";

// Audio engine state (placeholder)
struct AudioEngine {
    float sample_rate = 48000.0f;
    uint32_t buffer_size = 256;
    bool input_enabled = false;
    float meter_in = -inf_db;
    float meter_out = -inf_db;

    void process(float* in_buf, float* out_buf, uint32_t frames) {
        // Placeholder: copy input to output
        // Real implementation would run the neural amp model + cabinet IR
        for (uint32_t i = 0; i < frames; i++) {
            out_buf[i] = in_buf ? in_buf[i] : 0.0f;
        }
    }
};

static AudioEngine g_audio;

// File browser callback
static void on_file_selected(const char* path, bool is_model) {
    // In a real implementation, this would trigger the file browser dialog
    // and call this callback with the selected file path
    fprintf(stdout, "File selected: %s (is_model=%d)\n", path ? path : "(null)", is_model);
}

// Parameter change callback
static void on_param_changed(uint32_t param_index, float value) {
    // In a real implementation, this would send the parameter value to the DSP
    fprintf(stdout, "Parameter %u changed to %.4f\n", param_index, value);
}

// Bypass callback
static void on_bypass(uint32_t param_index, bool on) {
    fprintf(stdout, "Parameter %u bypass: %s\n", param_index, on ? "ON" : "OFF");
}

// Show usage information
static void print_usage(const char* prog) {
    fprintf(stdout,
        "AIDA-X WGPU+egui Standalone\n"
        "Usage: %s [options]\n"
        "\n"
        "Options:\n"
        "  --model <path>     Path to amp model JSON file\n"
        "  --cab <path>       Path to cabinet IR file\n"
        "  --width <px>       Window width (default: 900)\n"
        "  --height <px>      Window height (default: 340)\n"
        "  --dpr <factor>     Display pixel ratio (default: 1.0)\n"
        "  --buffer <frames>  Audio buffer size in frames (default: 256)\n"
        "  --sr <Hz>          Sample rate (default: 48000)\n"
        "  --help             Show this help\n",
        prog ? prog : "aida-x-wgpu");
}

int main(int argc, char* argv[]) {
    WgpuApp::Config config;
    const char* model_path = kDefaultModelName;
    const char* cab_path = kDefaultCabinetName;

    // Parse command line arguments
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            print_usage(argv[0]);
            return 0;
        } else if (strcmp(argv[i], "--model") == 0 && i + 1 < argc) {
            model_path = argv[++i];
        } else if (strcmp(argv[i], "--cab") == 0 && i + 1 < argc) {
            cab_path = argv[++i];
        } else if (strcmp(argv[i], "--width") == 0 && i + 1 < argc) {
            config.width = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--height") == 0 && i + 1 < argc) {
            config.height = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--dpr") == 0 && i + 1 < argc) {
            config.dpr = atof(argv[++i]);
        } else if (strcmp(argv[i], "--buffer") == 0 && i + 1 < argc) {
            g_audio.buffer_size = static_cast<uint32_t>(atoi(argv[++i]));
        } else if (strcmp(argv[i], "--sr") == 0 && i + 1 < argc) {
            g_audio.sample_rate = static_cast<float>(atof(argv[++i]));
        } else {
            fprintf(stderr, "Unknown option: %s\n", argv[i]);
            print_usage(argv[0]);
            return 1;
        }
    }

    fprintf(stdout, "AIDA-X WGPU+egui v1.1.0\n");
    fprintf(stdout, "Model: %s\n", model_path);
    fprintf(stdout, "Cabinet: %s\n", cab_path);
    fprintf(stdout, "Buffer: %u frames @ %.0f Hz\n", g_audio.buffer_size, g_audio.sample_rate);

    // Initialize the application
    WgpuApp app;
    if (!app.init(config)) {
        fprintf(stderr, "Failed to initialize WGPU application\n");
        return 1;
    }

    // Set callbacks
    app.set_param_changed_callback(on_param_changed);
    app.set_file_loaded_callback([&app](const char* path, bool is_model) {
        on_file_selected(path, is_model);
        if (is_model) {
            app.m_model_path = path;
        }
    });
    app.set_bypass_callback(on_bypass);

    // Initialize file paths
    app.m_model_path = model_path;
    app.m_cab_path = cab_path;

    fprintf(stdout, "Starting main loop...\n");
    fprintf(stdout, "Press Escape or close window to exit.\n");

    // Main event loop
    while (!glfwWindowShouldClose(static_cast<GLFWwindow*>(app.backend().get_native_window()))) {
        app.process_events();

        // Check for escape key
        GLFWwindow* window = static_cast<GLFWwindow*>(app.backend().get_native_window());
        if (glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS) {
            glfwSetWindowShouldClose(window, GLFW_TRUE);
            break;
        }

        app.render_frame();
    }

    app.shutdown();
    fprintf(stdout, "Shut down complete.\n");

    return 0;
}
