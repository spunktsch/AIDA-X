// AIDA-X WGPU+egui Backend
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <cstdint>
#include <cstring>
#include <string>
#include <vector>
#include <functional>

// wgpu-native FFI types
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

// egui-inspired immediate-mode GUI types
namespace AidaGui {

struct Vec2 {
    float x, y;
    static Vec2 zero() { return {0.f, 0.f}; }
    static Vec2 one() { return {1.f, 1.f}; }
    Vec2 operator+(const Vec2& o) const { return {x + o.x, y + o.y}; }
    Vec2 operator-(const Vec2& o) const { return {x - o.x, y - o.y}; }
    Vec2 operator*(float s) const { return {x * s, y * s}; }
    Vec2& operator+=(const Vec2& o) { x += o.x; y += o.y; return *this; }
};

struct Rect {
    float x, y, w, h;
    static Rect zero() { return {0.f, 0.f, 0.f, 0.f}; }
    float left() const { return x; }
    float right() const { return x + w; }
    float top() const { return y; }
    float bottom() const { return y + h; }
    float center_x() const { return x + w / 2.f; }
    float center_y() const { return y + h / 2.f; }
    Vec2 size() const { return {w, h}; }
    bool contains(float px, float py) const {
        return px >= x && px < x + w && py >= y && py < y + h;
    }
};

struct Color {
    float r, g, b, a;
    static Color white() { return {1.f, 1.f, 1.f, 1.f}; }
    static Color black() { return {0.f, 0.f, 0.f, 1.f}; }
    static Color transparent() { return {0.f, 0.f, 0.f, 0.f}; }
    static Color from_rgba(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255) {
        return {r / 255.f, g / 255.f, b / 255.f, a / 255.f};
    }
};

struct Vertex {
    float pos[2];
    float uv[2];
    uint32_t color;
};

struct PrimRect {
    Rect rect;
    Color color;
    float round_radius;
    uint8_t round_corners; // bitfield: top-left=1, top-right=2, bottom-right=4, bottom-left=8
};

struct PrimText {
    Rect rect;
    const char* text;
    float font_size;
    Color color;
    float max_width;
};

struct PrimImage {
    Rect rect;
    void* image_id;
    Vec2 uv0;
    Vec2 uv1;
    Color tint;
};

struct PrimLine {
    Vec2 from;
    Vec2 to;
    Color color;
    float thickness;
};

using Shape = PrimRect;

struct Mesh {
    std::vector<Vertex> vertices;
    std::vector<uint16_t> indices;
};

struct Cursor {
    Vec2 pos;
    Vec2 clip;
};

// Immediate-mode GUI context
struct GuiContext {
    float width, height;
    float dpr;
    Cursor cursor;
    std::vector<PrimRect> rects;
    std::vector<PrimText> texts;
    std::vector<PrimImage> images;
    std::vector<PrimLine> lines;
    std::vector<Mesh> meshes;

    // Widget state
    std::function<bool(float, float)> hover_test;
    bool hovered = false;
    bool clicked = false;

    void begin(float w, float h);
    void end();
    Rect next_row(float spacing);
    Rect next_col(float spacing);
    void push_clip_rect(Rect clip);
    void pop_clip_rect();
};

// Input state
struct InputState {
    bool mouse_down[3] = {};
    Vec2 mouse_pos;
    float scroll;
    char typed_char = 0;
    bool ctrl = false;
    bool shift = false;
};

// Widget callbacks
struct WidgetState {
    bool hovered;
    bool clicked;
    bool active;
};

} // namespace AidaGui
