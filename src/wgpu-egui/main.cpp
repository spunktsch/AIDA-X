// AIDA-X WGPU+egui Standalone Application
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

// Entry point for the WGPU+egui standalone version of AIDA-X.
// Replicates the original standalone application using a WGPU+egui
// rendering backend instead of DPF/DGL with OpenGL/NanoVG.

#include "WgpuApp.hpp"
#include "WgpuBackend.hpp"
#include "Widgets.hpp"

#include <cstdio>
#include <cstdlib>
#include <cstring>

static void print_usage(const char* prog) {
    fprintf(stdout,
        "AIDA-X WGPU+egui Standalone\n"
        "Usage: %s [options]\n"
        "\n"
        "Options:\n"
        "  --model <path>     Path to amp model JSON file\n"
        "  --cab <path>       Path to cabinet IR file\n"
        "  --width <px>       Window width (default: 900)\n"
        "  --height <px>      Window height (default: 442)\n"
        "  --dpr <factor>     Display pixel ratio (default: 1.0)\n"
        "  --help             Show this help\n",
        prog ? prog : "aida-x-wgpu");
}

int main(int argc, char* argv[]) {
    WgpuApp::Config config;
    const char* model_path = "aida.json";
    const char* cab_path = "aida_ir.wav";

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            print_usage(argv[0]);
            return 0;
        } else if (strcmp(argv[i], "--model") == 0 && i + 1 < argc) {
            model_path = argv[++i];
        } else if (strcmp(argv[i], "--cab") == 0 && i + 1 < argc) {
            cab_path = argv[++i];
        } else if (strcmp(argv[i], "--width") == 0 && i + 1 < argc) {
            config.width = static_cast<uint32_t>(atoi(argv[++i]));
        } else if (strcmp(argv[i], "--height") == 0 && i + 1 < argc) {
            config.height = static_cast<uint32_t>(atoi(argv[++i]));
        } else if (strcmp(argv[i], "--dpr") == 0 && i + 1 < argc) {
            config.dpr = static_cast<float>(atof(argv[++i]));
        } else {
            fprintf(stderr, "Unknown option: %s\n", argv[i]);
            print_usage(argv[0]);
            return 1;
        }
    }

    fprintf(stdout, "AIDA-X WGPU+egui v1.1.0\n");
    fprintf(stdout, "Model: %s\n", model_path);
    fprintf(stdout, "Cabinet: %s\n", cab_path);
    fprintf(stdout, "Press Escape or close window to exit.\n\n");

    WgpuApp app;
    if (!app.init(config)) {
        fprintf(stderr, "Failed to initialize WGPU application\n");
        return 1;
    }

    app.m_model_path = model_path;
    app.m_cab_path = cab_path;
    app.m_last_model_file = model_path;
    app.m_last_cab_file = cab_path;

    app.set_param_changed_callback([](uint32_t param_index, float value) {
        fprintf(stdout, "[PARAM] #%u = %.4f\n", param_index, value);
    });

    app.set_file_loaded_callback([](const char* path) {
        fprintf(stdout, "[FILE] Selected: %s\n", path ? path : "(null)");
    });

    app.set_bypass_callback([](uint32_t param_index, bool on) {
        fprintf(stdout, "[BYPASS] #%u = %s\n", param_index, on ? "ON" : "OFF");
    });

    fprintf(stdout, "Starting main loop...\n");
    app.run();

    fprintf(stdout, "Shut down complete.\n");
    return 0;
}
