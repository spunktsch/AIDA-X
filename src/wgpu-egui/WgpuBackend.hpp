// AIDA-X WGPU+egui Backend - WGPU Window and Renderer
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "Gui.hpp"
#include <string>
#include <memory>

typedef void* WGPUInstance;
typedef void* WGPUAdapter;
typedef void* WGPUDevice;
typedef void* WGPUSurface;
typedef void* WGPUSurfaceConfiguration;
typedef void* WGPUSwapChain;
typedef void* WGPUTexture;
typedef void* WGPUTextureView;
typedef void* WGPUCommandBuffer;
typedef void* WGPUQueue;
typedef void* WGPURenderPassEncoder;
typedef void* WGPURenderPipeline;
typedef void* WGPUBuffer;
typedef void* WGPUSampler;

struct WgpuConfig {
    uint32_t width = 800;
    uint32_t height = 600;
    const char* title = "AIDA-X";
    bool resizable = true;
};

class WgpuBackend {
public:
    ~WgpuBackend();

    bool init(const WgpuConfig& config);
    bool init_embedded(void* parent_window, const WgpuConfig& config);
    void poll_events();
    bool begin_frame();
    void end_frame(const GuiContext& gui_ctx);
    void shutdown();

    void set_size(uint32_t w, uint32_t h);
    void get_size(uint32_t* w, uint32_t* h) const;
    void set_title(const char* title);
    bool is_closed() const;
    void close();

    uint32_t get_width() const { return m_width; }
    uint32_t get_height() const { return m_height; }
    float get_dpr() const { return m_dpr; }
    void* get_native_window() const { return m_native_window; }

private:
    bool create_instance();
    bool create_adapter();
    bool create_device();
    bool create_surface(void* parent_window = nullptr);
    void resize(uint32_t w, uint32_t h);

    WGPUInstance m_instance = nullptr;
    WGPUAdapter m_adapter = nullptr;
    WGPUDevice m_device = nullptr;
    WGPUSurface m_surface = nullptr;
    WGPUSwapChain m_swap_chain = nullptr;
    WGPUTexture m_swap_chain_texture = nullptr;
    WGPUTextureView m_swap_chain_view = nullptr;
    WGPUQueue m_queue = nullptr;

    uint32_t m_width = 0;
    uint32_t m_height = 0;
    float m_dpr = 1.0f;
    void* m_native_window = nullptr;
    bool m_closed = false;
};
