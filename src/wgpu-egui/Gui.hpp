// AIDA-X WGPU+egui Backend - GUI Types and Drawing Primitives
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <cstdint>
#include <cstring>
#include <string>
#include <vector>
#include <functional>

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
    Rect translated(float tx, float ty) const {
        return {x + tx, y + ty, w, h};
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
    static Color from_hsl(float h, float s, float l) {
        float r, g, b;
        if (s == 0.0f) {
            r = g = b = l;
        } else {
            auto hue_to_rgb = [](float p, float q, float t) {
                if (t < 0.0f) t += 1.0f;
                if (t > 1.0f) t -= 1.0f;
                if (t < 1.0f / 6.0f) return p + (q - p) * 6.0f * t;
                if (t < 1.0f / 2.0f) return q;
                if (t < 2.0f / 3.0f) return p + (q - p) * (2.0f / 3.0f - t) * 6.0f;
                return p;
            };
            float q = l < 0.5f ? l * (1.0f + s) : l + s - l * s;
            float p = 2.0f * l - q;
            r = hue_to_rgb(p, q, h + 1.0f / 3.0f);
            g = hue_to_rgb(p, q, h);
            b = hue_to_rgb(p, q, h - 1.0f / 3.0f);
        }
        return {r, g, b, 1.0f};
    }
    Color with_alpha(float a) const { return {r, g, b, a}; }
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
    uint8_t round_corners;
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

struct Mesh {
    std::vector<Vertex> vertices;
    std::vector<uint16_t> indices;
};

struct Cursor {
    Vec2 pos;
    Vec2 clip;
};

struct InputState {
    bool mouse_down[3] = {};
    Vec2 mouse_pos;
    float scroll;
    char typed_char = 0;
    bool ctrl = false;
    bool shift = false;
    float last_click_time = 0.0f;
    Vec2 last_click_pos = {0.f, 0.f};
    int last_clicked_widget = -1;
};

struct WidgetState {
    bool hovered;
    bool clicked;
    bool active;
};

struct GuiContext {
    float width, height;
    float dpr;
    Cursor cursor;
    std::vector<PrimRect> rects;
    std::vector<PrimText> texts;
    std::vector<PrimImage> images;
    std::vector<PrimLine> lines;
    std::vector<Mesh> meshes;

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

// Drawing primitives (add to GuiContext)
void rect(GuiContext& ctx, Rect r, Color fill);
void rect_rounded(GuiContext& ctx, Rect r, Color fill, float radius);
void line(GuiContext& ctx, Vec2 a, Vec2 b, Color c, float thickness);
void text(GuiContext& ctx, Rect r, const char* s, float font_size, Color c);
void text_centered(GuiContext& ctx, float cx, float cy, const char* s, float font_size, Color c);
void image(GuiContext& ctx, Rect r, void* img_id, Color tint);

// Widget callbacks
using ParamChangedCallback = std::function<void(uint32_t param_index, float value)>;
using BypassCallback = std::function<void(uint32_t param_index, bool on)>;
using FileLoadedCallback = std::function<void(const char* path)>;

} // namespace AidaGui
