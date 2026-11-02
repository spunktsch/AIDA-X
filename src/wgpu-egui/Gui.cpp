// AIDA-X WGPU+egui Backend - GUI Implementation
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#include "Gui.hpp"
#include <cmath>
#include <algorithm>

namespace AidaGui {

// Color helpers
static uint32_t color_to_u32(Color c) {
    uint8_t r = static_cast<uint8_t>(std::round(c.r * 255.f));
    uint8_t g = static_cast<uint8_t>(std::round(c.g * 255.f));
    uint8_t b = static_cast<uint8_t>(std::round(c.b * 255.f));
    uint8_t a = static_cast<uint8_t>(std::round(c.a * 255.f));
    return (a << 24) | (r << 16) | (g << 8) | b;
}

static Color u32_to_color(uint32_t c) {
    return {
        ((c >> 16) & 0xFF) / 255.f,
        ((c >> 8) & 0xFF) / 255.f,
        (c & 0xFF) / 255.f,
        ((c >> 24) & 0xFF) / 255.f
    };
}

void GuiContext::begin(float w, float h) {
    width = w;
    height = h;
    cursor.pos = {0.f, 0.f};
    cursor.clip = {0.f, 0.f};
    rects.clear();
    texts.clear();
    images.clear();
    lines.clear();
    meshes.clear();
}

void GuiContext::end() {
}

Rect GuiContext::next_row(float spacing) {
    Rect r;
    r.w = width - cursor.pos.x;
    r.h = 0.f;
    r.x = cursor.pos.x;
    r.y = cursor.pos.y;
    cursor.pos.x += r.w + spacing;
    return r;
}

Rect GuiContext::next_col(float spacing) {
    Rect r;
    r.w = width;
    r.h = 0.f;
    r.x = 0.f;
    r.y = cursor.pos.y;
    cursor.pos.y += r.h + spacing;
    return r;
}

void GuiContext::push_clip_rect(Rect clip) {
    cursor.clip = {clip.x, clip.y};
}

void GuiContext::pop_clip_rect() {
    cursor.clip = {0.f, 0.f};
}

// Drawing functions
void rect(Rect r, Color fill) {
    // TODO: round rect support
    PrimRect prim;
    prim.rect = r;
    prim.color = fill;
    prim.round_radius = 0.f;
    prim.round_corners = 0;
}

void rect_rounded(Rect r, Color fill, float radius) {
    PrimRect prim;
    prim.rect = r;
    prim.color = fill;
    prim.round_radius = radius;
    prim.round_corners = 15; // all corners
}

void line(Vec2 a, Vec2 b, Color c, float thickness) {
    PrimLine prim;
    prim.from = a;
    prim.to = b;
    prim.color = c;
    prim.thickness = thickness;
}

void text(Rect r, const char* s, float font_size, Color c) {
    PrimText prim;
    prim.rect = r;
    prim.text = s;
    prim.font_size = font_size;
    prim.color = c;
    prim.max_width = r.w;
}

void image(Rect r, void* img_id, Color tint) {
    PrimImage prim;
    prim.rect = r;
    prim.image_id = img_id;
    prim.uv0 = {0.f, 0.f};
    prim.uv1 = {1.f, 1.f};
    prim.tint = tint;
}

// Widget helpers
WidgetState button(Rect r, const char* label) {
    WidgetState state{false, false, false};
    // Check if mouse is within button rect
    state.hovered = true; // TODO: check cursor position
    return state;
}

WidgetState knob(Rect r, float value, float min, float max, const char* label) {
    WidgetState state{false, false, false};
    // TODO: implement knob widget
    return state;
}

WidgetState slider(Rect r, float* value, float min, float max) {
    WidgetState state{false, false, false};
    // TODO: implement slider widget
    return state;
}

} // namespace AidaGui
