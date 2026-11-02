// AIDA-X WGPU+egui Backend - WGPU Window and Renderer
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "Gui.hpp"
#include <string>
#include <memory>

// wgpu-native C FFI
typedef enum WGPUTextureFormat {
    WGPUTextureFormat_Unspecified = 0,
    WGPUTextureFormat_RGBA8Unorm = 1,
    WGPUTextureFormat_RGBA8UnormSrgb = 2,
    WGPUTextureFormat_RGBA8Srgb = 3,
    WGPUTextureFormat_RG11b10Float = 4,
    WGPUTextureFormat_RGB9e5Float = 5,
    WGPUTextureFormat_UInt32 = 13,
    WGPUTextureFormat_Float32 = 14,
    WGPUTextureFormat_Float32Dual = 15,
    WGPUTextureFormat_BC1RGBA = 17,
    WGPUTextureFormat_BC1RGBAUnormSrgb = 18,
    WGPUTextureFormat_BC2RGBA = 19,
    WGPUTextureFormat_BC2RGBAUnormSrgb = 20,
    WGPUTextureFormat_BC3RGBA = 21,
    WGPUTextureFormat_BC3RGBAUnormSrgb = 22,
    WGPUTextureFormat_BC4RUnorm = 23,
    WGPUTextureFormat_BC4RSnorm = 24,
    WGPUTextureFormat_BC5RGUnorm = 25,
    WGPUTextureFormat_BC5RGSnorm = 26,
    WGPUTextureFormat_BC6HRGBFloat = 27,
    WGPUTextureFormat_BC6HRGBUFloat = 28,
    WGPUTextureFormat_BC7RGBAUnorm = 29,
    WGPUTextureFormat_BC7RGBAUnormSrgb = 30,
    WGPUTextureFormat_ETC2RGB8Unorm = 31,
    WGPUTextureFormat_ETC2RGB8UnormSrgb = 32,
    WGPUTextureFormat_ETC2RGBA8Unorm = 33,
    WGPUTextureFormat_ETC2RGBA8UnormSrgb = 34,
    WGPUTextureFormat_EACR11Unorm = 35,
    WGPUTextureFormat_EACR11Snorm = 36,
    WGPUTextureFormat_EACRG11Unorm = 37,
    WGPUTextureFormat_EACRG11Snorm = 38,
    WGPUTextureFormat_ASTC4x4Unorm = 39,
    WGPUTextureFormat_ASTC4x4UnormSrgb = 40,
    WGPUTextureFormat_ASTC5x5Unorm = 41,
    WGPUTextureFormat_ASTC5x5UnormSrgb = 42,
    WGPUTextureFormat_ASTC6x6Unorm = 43,
    WGPUTextureFormat_ASTC6x6UnormSrgb = 44,
    WGPUTextureFormat_ASTC8x5Unorm = 45,
    WGPUTextureFormat_ASTC8x5UnormSrgb = 46,
    WGPUTextureFormat_ASTC8x6Unorm = 47,
    WGPUTextureFormat_ASTC8x6UnormSrgb = 48,
    WGPUTextureFormat_ASTC10x5Unorm = 49,
    WGPUTextureFormat_ASTC10x5UnormSrgb = 50,
    WGPUTextureFormat_ASTC10x6Unorm = 61,
    WGPUTextureFormat_ASTC10x6UnormSrgb = 62,
    WGPUTextureFormat_ASTC10x8Unorm = 51,
    WGPUTextureFormat_ASTC10x8UnormSrgb = 52,
    WGPUTextureFormat_ASTC10x10Unorm = 53,
    WGPUTextureFormat_ASTC10x10UnormSrgb = 54,
    WGPUTextureFormat_ASTC12x10Unorm = 55,
    WGPUTextureFormat_ASTC12x10UnormSrgb = 56,
    WGPUTextureFormat_ASTC12x12Unorm = 57,
    WGPUTextureFormat_ASTC12x12UnormSrgb = 58,
    WGPUTextureFormat_ASTC8x8Unorm = 63,
    WGPUTextureFormat_ASTC8x8UnormSrgb = 64,
    WGPUTextureFormat_ASTC10x10Unorm = 65,
    WGPUTextureFormat_ASTC10x10UnormSrgb = 66,
    WGPUTextureFormat_ASTC12x12Unorm = 67,
    WGPUTextureFormat_ASTC12x12UnormSrgb = 68,
    WGPUTextureFormat_ASTC6x5Unorm = 69,
    WGPUTextureFormat_ASTC6x5UnormSrgb = 70,
    WGPUTextureFormat_ASTC8x7Unorm = 71,
    WGPUTextureFormat_ASTC8x7UnormSrgb = 72,
    WGPUTextureFormat_ASTC10x9Unorm = 73,
    WGPUTextureFormat_ASTC10x9UnormSrgb = 74,
    WGPUTextureFormat_ASTC10x10Unorm = 75,
    WGPUTextureFormat_ASTC10x10UnormSrgb = 76,
    WGPUTextureFormat_ASTC12x11Unorm = 77,
    WGPUTextureFormat_ASTC12x11UnormSrgb = 78,
    WGPUTextureFormat_ASTC12x12Unorm = 79,
    WGPUTextureFormat_ASTC12x12UnormSrgb = 80,
    WGPUTextureFormat_BGRA8Unorm = 81,
    WGPUTextureFormat_BGRA8UnormSrgb = 82,
    WGPUTextureFormat_RGB10a2Uint = 83,
    WGPUTextureFormat_RGB10a2Unorm = 84,
    WGPUTextureFormat_RG16Uint = 85,
    WGPUTextureFormat_RG16Sint = 86,
    WGPUTextureFormat_RG16Float = 87,
    WGPUTextureFormat_RGBA16Uint = 88,
    WGPUTextureFormat_RGBA16Sint = 89,
    WGPUTextureFormat_RGBA16Float = 90,
    WGPUTextureFormat_RG32Uint = 91,
    WGPUTextureFormat_RG32Sint = 92,
    WGPUTextureFormat_RG32Float = 93,
    WGPUTextureFormat_RGBA32Uint = 94,
    WGPUTextureFormat_RGBA32Sint = 95,
    WGPUTextureFormat_RGBA32Float = 96,
    WGPUTextureFormat_STencil8 = 97,
    WGPUTextureFormat_Depth16Plus = 98,
    WGPUTextureFormat_Depth16PlusStencil8 = 99,
    WGPUTextureFormat_Depth24Plus = 100,
    WGPUTextureFormat_Depth24PlusStencil8 = 101,
    WGPUTextureFormat_Depth32Float = 102,
    WGPUTextureFormat_Depth32FloatStencil8 = 103,
    WGPUTextureFormat_BC6HRGBFloat = 104,
    WGPUTextureFormat_BC6HRGBUFloat = 105,
    WGPUTextureFormat_BC7RGBAUnorm = 106,
    WGPUTextureFormat_BC7RGBAUnormSrgb = 107,
    WGPUTextureFormat_ETC2RGB8Unorm = 108,
    WGPUTextureFormat_ETC2RGB8UnormSrgb = 109,
    WGPUTextureFormat_ETC2RGBA8Unorm = 110,
    WGPUTextureFormat_ETC2RGBA8UnormSrgb = 111,
    WGPUTextureFormat_EACR11Unorm = 112,
    WGPUTextureFormat_EACR11Snorm = 113,
    WGPUTextureFormat_EACRG11Unorm = 114,
    WGPUTextureFormat_EACRG11Snorm = 115,
    WGPUTextureFormat_ASTC4x4Unorm = 116,
    WGPUTextureFormat_ASTC4x4UnormSrgb = 117,
    WGPUTextureFormat_ASTC5x5Unorm = 118,
    WGPUTextureFormat_ASTC5x5UnormSrgb = 119,
    WGPUTextureFormat_ASTC6x6Unorm = 120,
    WGPUTextureFormat_ASTC6x6UnormSrgb = 121,
    WGPUTextureFormat_ASTC8x5Unorm = 122,
    WGPUTextureFormat_ASTC8x5UnormSrgb = 123,
    WGPUTextureFormat_ASTC8x6Unorm = 124,
    WGPUTextureFormat_ASTC8x6UnormSrgb = 125,
    WGPUTextureFormat_ASTC10x5Unorm = 126,
    WGPUTextureFormat_ASTC10x5UnormSrgb = 127,
    WGPUTextureFormat_ASTC10x6Unorm = 128,
    WGPUTextureFormat_ASTC10x6UnormSrgb = 129,
    WGPUTextureFormat_ASTC10x8Unorm = 130,
    WGPUTextureFormat_ASTC10x8UnormSrgb = 131,
    WGPUTextureFormat_ASTC10x10Unorm = 132,
    WGPUTextureFormat_ASTC10x10UnormSrgb = 133,
    WGPUTextureFormat_ASTC12x10Unorm = 134,
    WGPUTextureFormat_ASTC12x10UnormSrgb = 135,
    WGPUTextureFormat_ASTC12x12Unorm = 136,
    WGPUTextureFormat_ASTC12x12UnormSrgb = 137,
    WGPUTextureFormat_ASTC6x5Unorm = 138,
    WGPUTextureFormat_ASTC6x5UnormSrgb = 139,
    WGPUTextureFormat_ASTC8x7Unorm = 140,
    WGPUTextureFormat_ASTC8x7UnormSrgb = 141,
    WGPUTextureFormat_ASTC10x9Unorm = 142,
    WGPUTextureFormat_ASTC10x9UnormSrgb = 143,
    WGPUTextureFormat_ASTC10x10Unorm = 144,
    WGPUTextureFormat_ASTC10x10UnormSrgb = 145,
    WGPUTextureFormat_ASTC12x11Unorm = 146,
    WGPUTextureFormat_ASTC12x11UnormSrgb = 147,
    WGPUTextureFormat_ASTC12x12Unorm = 148,
    WGPUTextureFormat_ASTC12x12UnormSrgb = 149,
} WGPUTextureFormat;

struct WgpuConfig {
    uint32_t width = 800;
    uint32_t height = 600;
    const char* title = "AIDA-X";
    bool resizable = true;
};

class WgpuBackend {
public:
    ~WgpuBackend();

    // Initialize the WGPU backend with the given config
    bool init(const WgpuConfig& config);

    // Create a window with the given parent window handle (for plugin embedding)
    bool init_embedded(void* parent_window, const WgpuConfig& config);

    // Process platform events (should be called every frame)
    void poll_events();

    // Begin a frame
    bool begin_frame();

    // End a frame, present to screen
    void end_frame(GuiContext& gui_ctx, const AidaGui::Mesh& mesh);

    // Shutdown
    void shutdown();

    // Window management
    void set_size(uint32_t w, uint32_t h);
    void get_size(uint32_t* w, uint32_t* h) const;
    void set_title(const char* title);
    bool is_closed() const;
    void close();

    // Get viewport size
    uint32_t get_width() const { return m_width; }
    uint32_t get_height() const { return m_height; }
    float get_dpr() const { return m_dpr; }

    // Platform-specific window handle
    void* get_native_window() const { return m_native_window; }

private:
    bool create_instance();
    bool create_adapter();
    bool create_device();
    bool create_surface(void* parent_window = nullptr);
    bool create_swap_chain();

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
