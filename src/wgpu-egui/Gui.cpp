// AIDA-X WGPU+egui Backend - GUI Drawing Implementation
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#include "Gui.hpp"
#include <cmath>
#include <algorithm>
#include <cstdio>

namespace AidaGui {

static uint32_t color_to_u32(Color c) {
    uint8_t r = static_cast<uint8_t>(std::round(c.r * 255.f));
    uint8_t g = static_cast<uint8_t>(std::round(c.g * 255.f));
    uint8_t b = static_cast<uint8_t>(std::round(c.b * 255.f));
    uint8_t a = static_cast<uint8_t>(std::round(c.a * 255.f));
    return (a << 24) | (r << 16) | (g << 8) | b;
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

// Color helper
static uint32_t c32(Color c) {
    return color_to_u32(c);
}

void rect(GuiContext& ctx, Rect r, Color fill) {
    PrimRect prim;
    prim.rect = r;
    prim.color = fill;
    prim.round_radius = 0.f;
    prim.round_corners = 0;
    ctx.rects.push_back(std::move(prim));
}

void rect_rounded(GuiContext& ctx, Rect r, Color fill, float radius) {
    PrimRect prim;
    prim.rect = r;
    prim.color = fill;
    prim.round_radius = radius;
    prim.round_corners = 15;
    ctx.rects.push_back(std::move(prim));
}

void line(GuiContext& ctx, Vec2 a, Vec2 b, Color c, float thickness) {
    PrimLine prim;
    prim.from = a;
    prim.to = b;
    prim.color = c;
    prim.thickness = thickness;
    ctx.lines.push_back(std::move(prim));
}

void text(GuiContext& ctx, Rect r, const char* s, float font_size, Color c) {
    PrimText prim;
    prim.rect = r;
    prim.text = s;
    prim.font_size = font_size;
    prim.color = c;
    prim.max_width = r.w;
    ctx.texts.push_back(std::move(prim));
}

void text_centered(GuiContext& ctx, float cx, float cy, const char* s, float font_size, Color c) {
    // Approximate text width (10px per char estimate)
    float approx_w = (float)std::strlen(s) * font_size * 0.55f;
    text(ctx, {cx - approx_w / 2.f, cy - font_size / 2.f, approx_w, font_size}, s, font_size, c);
}

} // namespace AidaGui
