// AIDA-X WGPU+egui Backend - Custom Widgets
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "Gui.hpp"
#include "WgpuBackend.hpp"
#include <cmath>
#include <cstring>
#include <string>

namespace AidaGui {

static constexpr float kSubWidgetsFullHeight = 90.0f;
static constexpr float kSubWidgetsFontSize = 14.0f;
static constexpr float kSubWidgetsPadding = 8.0f;

static inline float deg_to_rad(float deg) {
    return deg * 3.14159265358979323846f / 180.0f;
}

static inline float normalized_level_meter_value(float db) {
    if (db < -70.f) return 0.f;
    if (db < -60.f) return (db + 70.f) * 0.25f;
    if (db < -50.f) return (db + 60.f) * 0.50f + 2.5f;
    if (db < -40.f) return (db + 50.f) * 0.75f + 7.5f;
    if (db < -30.f) return (db + 40.f) * 1.50f + 15.0f;
    if (db < -20.f) return (db + 30.f) * 2.00f + 30.0f;
    if (db < 0.f)   return (db + 20.f) * 2.50f + 50.0f;
    return 100.f;
}

// ============================================================================
// AidaKnob - Rotary knob with tick markers, rotating indicator, value display
// Matches original src/Widgets.hpp AidaKnob
// ============================================================================

class AidaKnob {
public:
    static constexpr float kScaleSize = 80.0f;
    static constexpr float kKnobSize = 55.0f;
    static constexpr float kKnobMargin = (kScaleSize - kKnobSize) / 2.0f;

    AidaKnob(float x, float y, float scale, ParamChangedCallback on_change)
        : m_x(x), m_y(y), m_scale(scale), m_on_change(std::move(on_change))
    {
        m_size_w = kScaleSize * scale;
        m_size_h = kSubWidgetsFullHeight * scale;
        m_knob_size = kKnobSize * scale;
        m_knob_half = m_knob_size / 2.0f;
        m_knob_margin = kKnobMargin * scale;
        m_font_size = kSubWidgetsFontSize * scale;
        m_radius = m_knob_half * 0.8f;
        m_value = 0.5f;
        m_default_value = 0.5f;
        m_min = 0.0f;
        m_max = 1.0f;
        m_drag_start_x = 0.0f;
        m_drag_start_val = 0.0f;
        m_prev_norm_value = -1.0f;
        m_last_scroll_time = 0.0f;
    }

    void set_value(float v) {
        v = std::clamp(v, m_min, m_max);
        m_value = (v - m_min) / (m_max - m_min);
    }
    void set_default(float def) {
        m_default_value = (def - m_min) / (m_max - m_min);
        if (m_value == m_default_value) m_value = m_default_value;
    }
    void set_range(float min, float max) {
        m_min = min;
        m_max = max;
        m_value = std::clamp(m_value, 0.0f, 1.0f);
    }
    void set_label(const char* label) { m_label = label; }

    bool handle_input(InputState& input, float px, float py) {
        float cx = m_x + m_size_w / 2.0f;
        float cy = m_y + m_knob_margin + m_knob_half;
        float dx = px - cx;
        float dy = py - cy;
        float dist = std::sqrt(dx * dx + dy * dy);
        float hit_radius = m_knob_half * 0.85f;

        bool hit = dist < hit_radius;
        m_hovered = hit && !m_pressed && !m_double_clicked;

        // Double-click reset
        if (hit && input.mouse_down[1] && !m_pressed) {
            m_value = m_default_value;
            float param_val = m_min + m_value * (m_max - m_min);
            if (m_on_change) m_on_change(m_param_id, param_val);
            return hit;
        }

        // Left-click drag
        if (hit && input.mouse_down[0] && !m_pressed) {
            m_pressed = true;
            m_drag_start_x = px;
            m_drag_start_val = m_value;
        }

        if (m_pressed && !input.mouse_down[0]) {
            m_pressed = false;
        }

        if (m_pressed) {
            float delta_x = px - m_drag_start_x;
            float delta = delta_x / (m_knob_half * 2.0f) * 0.5f;
            float new_val = m_drag_start_val + delta;
            if (std::abs(new_val - m_value) > 0.001f) {
                m_value = std::clamp(new_val, 0.0f, 1.0f);
                float param_val = m_min + m_value * (m_max - m_min);
                if (m_on_change) m_on_change(m_param_id, param_val);
                m_prev_norm_value = m_value;
            }
        }

        // Ctrl + scroll for fine adjustment
        float cx2 = m_x + m_size_w / 2.0f;
        float cy2 = m_y + m_knob_margin + m_knob_half;
        float dist2 = std::sqrt((px - cx2) * (px - cx2) + (py - cy2) * (py - cy2));
        if (dist2 < hit_radius && input.ctrl && std::abs(input.scroll) > 0.01f) {
            float scroll_delta = -input.scroll * 0.01f;
            float new_val = std::clamp(m_value + scroll_delta, 0.0f, 1.0f);
            if (std::abs(new_val - m_value) > 0.0001f) {
                m_value = new_val;
                float param_val = m_min + m_value * (m_max - m_min);
                if (m_on_change) m_on_change(m_param_id, param_val);
                m_prev_norm_value = m_value;
            }
            input.scroll = 0.0f;
        }

        // Shift click reset
        if (hit && input.shift && !m_pressed) {
            m_value = m_default_value;
            float param_val = m_min + m_value * (m_max - m_min);
            if (m_on_change) m_on_change(m_param_id, param_val);
        }

        return hit;
    }

    void render(GuiContext& gui) {
        float cx = m_x + m_size_w / 2.0f;
        float cy = m_y + m_knob_margin + m_knob_half;
        float r = m_knob_half;
        float pad = kSubWidgetsPadding * m_scale;

        // Scale background
        Color scale_bg = Color::from_rgba(0x27, 0x28, 0x34);
        rect_rounded(gui, {m_x, m_y, m_size_w, m_size_h * 0.65f}, scale_bg, 4.0f * m_scale);

        // Tick marks
        float tick_start_y = m_y + m_size_h * 0.12f;
        float tick_end_base = m_y + m_size_h * 0.22f;
        for (int i = 0; i <= 10; i++) {
            float tx = m_x + pad + (m_size_w - pad * 2.0f) * (i / 10.0f);
            float is_center = (i == 5);
            float tick_h = is_center ? 7.0f * m_scale : 3.5f * m_scale;
            Color tick_color = Color::from_rgba(0x50, 0x50, 0x50, 0.8f);
            if (is_center) tick_color = Color::from_rgba(0x90, 0x90, 0x90, 0.9f);
            line(gui, {tx, tick_start_y}, {tx, tick_start_y + tick_h}, tick_color, 1.0f);
        }

        // Arc track (partial circle, -135 to +135 degrees)
        float arc_start = deg_to_rad(-135.0f);
        float arc_end = deg_to_rad(135.0f);
        float arc_r = r + 4.0f * m_scale;
        int arc_segments = 48;

        Color arc_bg = Color::from_rgba(0x33, 0x33, 0x33, 0.5f);
        Color arc_active = Color::from_rgba(0xa4, 0xf4, 0x4d, 0.6f);
        float norm_val = m_value;

        for (int i = 0; i < arc_segments; i++) {
            float t0 = arc_start + (arc_end - arc_start) * (i / (float)arc_segments);
            float t1 = arc_start + (arc_end - arc_start) * ((i + 1) / (float)arc_segments);
            float x0 = cx + std::cos(t0) * arc_r;
            float y0 = cy + std::sin(t0) * arc_r;
            float x1 = cx + std::cos(t1) * arc_r;
            float y1 = cy + std::sin(t1) * arc_r;

            float seg_t = (t0 - arc_start) / (arc_end - arc_start);
            bool active = seg_t <= norm_val;
            Color seg_color = active ? arc_active : arc_bg;

            line(gui, {x0, y0}, {x1, y1}, seg_color, 3.0f * m_scale);
        }

        // Knob body
        float angle = deg_to_rad(-135.0f + 270.0f * norm_val);
        Color knob_body;
        if (m_pressed) {
            knob_body = Color::from_rgba(0x15, 0x2c, 0x0d);
        } else if (m_hovered) {
            knob_body = Color::from_rgba(0x20, 0x3a, 0x15);
        } else {
            knob_body = Color::from_rgba(0x18, 0x28, 0x0a);
        }

        // Draw knob as filled circle using small line segments
        int knob_segments = 32;
        for (int i = 0; i < knob_segments; i++) {
            float a0 = 2.0f * 3.14159f * i / knob_segments;
            float a1 = 2.0f * 3.14159f * (i + 1) / knob_segments;
            line(gui, {cx + std::cos(a0) * r, cy + std::sin(a0) * r},
                        {cx + std::cos(a1) * r, cy + std::sin(a1) * r},
                 knob_body, m_knob_size);
        }

        // Indicator line (rotates with value)
        float indicator_x = cx + std::cos(angle) * r * 0.72f;
        float indicator_y = cy + std::sin(angle) * r * 0.72f;
        Color indicator_color = m_pressed ? Color::from_rgba(0xa4, 0xf4, 0x4d)
                                           : Color::from_rgba(0xc0, 0xff, 0x30);
        line(gui, {cx, cy}, {indicator_x, indicator_y}, indicator_color, 2.5f * m_scale);

        // Label at bottom
        if (m_label && m_label[0]) {
            text_centered(gui, cx, m_y + m_size_h - 6.0f * m_scale, m_label, m_font_size, Color::from_rgba(0xd0, 0xd0, 0xd0, 0.9f));
        }

        // Value display during drag or hover
        if (m_pressed || m_hovered) {
            float param_val = m_min + m_value * (m_max - m_min);
            char buf[32];
            std::snprintf(buf, sizeof(buf) - 1, "%.2f", param_val);
            buf[sizeof(buf) - 1] = '\0';

            float val_display_y = cy + r + 6.0f * m_scale;
            float val_bg_w = 55.0f * m_scale;
            float val_bg_h = m_font_size + 6.0f * m_scale;
            rect_rounded(gui, {cx - val_bg_w / 2.0f, val_display_y, val_bg_w, val_bg_h},
                         Color::from_rgba(0, 0, 0, 0.55f), 3.0f * m_scale);
            text_centered(gui, cx, val_display_y + val_bg_h / 2.0f, buf, m_font_size * 0.85f, Color::from_rgba(0xe0, 0xe0, 0xe0));
        }
    }

    void set_param_id(uint32_t id) { m_param_id = id; }

private:
    float m_x, m_y;
    float m_size_w, m_size_h;
    float m_scale;
    float m_knob_size;
    float m_knob_half;
    float m_knob_margin;
    float m_font_size;
    float m_radius;
    float m_value;
    float m_default_value;
    float m_min, m_max;
    float m_drag_start_x;
    float m_drag_start_val;
    bool m_pressed = false;
    bool m_hovered = false;
    bool m_double_clicked = false;
    float m_prev_norm_value;
    float m_last_scroll_time;
    uint32_t m_param_id = 0;
    const char* m_label = nullptr;
    ParamChangedCallback m_on_change;
};

// ============================================================================
// AidaSwitch - Vertical toggle switch with smooth animation
// Matches original src/Widgets.hpp AidaPluginSwitch
// ============================================================================

class AidaSwitch {
public:
    static constexpr float kSwitchWidth = 25.0f;
    static constexpr float kSwitchHeight = 60.0f;
    static constexpr float kSwitchRadius = kSwitchWidth / 2.0f;
    static constexpr float kFullWidth = kSwitchWidth + kSubWidgetsPadding * 2.0f;

    AidaSwitch(float x, float y, float scale, uint32_t param_id,
               const char* label_off, const char* label_on,
               BypassCallback on_toggle)
        : m_x(x), m_y(y), m_scale(scale), m_param_id(param_id)
        , m_label_off(label_off), m_label_on(label_on)
        , m_on_toggle(std::move(on_toggle))
        , m_checked(false), m_anim_pos(0.5f), m_target_pos(0.5f)
        , m_clicked(false), m_hovered(false)
    {
        m_size_w = kFullWidth * scale;
        m_size_h = kSubWidgetsFullHeight * scale;
        m_track_w = kSwitchWidth * scale;
        m_track_h = kSwitchHeight * scale;
        m_track_r = kSwitchRadius * scale;
    }

    void set_checked(bool checked) {
        m_checked = checked;
        m_target_pos = checked ? 1.0f : 0.0f;
    }
    bool is_checked() const { return m_checked; }

    bool handle_input(InputState& input, float px, float py) {
        float track_x = m_x + kSubWidgetsPadding * m_scale;
        float track_y = m_y + m_size_h * 0.3f;

        bool hit = px >= m_x && px < m_x + m_size_w && py >= m_y && py < m_y + m_size_h;
        m_hovered = hit && !m_clicked;

        if (hit && input.mouse_down[0] && !m_clicked) {
            m_clicked = true;
            m_checked = !m_checked;
            m_target_pos = m_checked ? 1.0f : 0.0f;
            if (m_on_toggle) {
                m_on_toggle(m_param_id, m_checked);
            }
        }

        if (!input.mouse_down[0]) {
            m_clicked = false;
        }

        return hit;
    }

    void render(GuiContext& gui) {
        // Smooth animation
        float anim_speed = 8.0f;
        float dt = 1.0f / 60.0f;
        m_anim_pos += (m_target_pos - m_anim_pos) * std::min(1.0f, anim_speed * dt);

        float pad = kSubWidgetsPadding * m_scale;
        float track_x = m_x + pad;
        float track_y = m_y + m_size_h * 0.3f;
        float track_w = m_track_w;
        float track_h = m_track_h;
        float track_r = m_track_r;

        // ON/OFF labels
        float label_size = kSubWidgetsFontSize * m_scale;
        Color text_color = Color::from_rgba(0xd0, 0xd0, 0xd0, 0.85f);
        text(gui, {m_x + pad, m_y, m_size_w - pad * 2, label_size},
             m_checked ? m_label_on : m_label_off, label_size, text_color);

        // Track background
        Color track_bg;
        if (m_checked) {
            track_bg = Color::from_rgba(0x54, 0x54, 0x54);
        } else {
            track_bg = Color::from_rgba(0xa4, 0xf7, 0x00, 0.9f);
        }
        rect_rounded(gui, {track_x, track_y, track_w, track_h}, track_bg, track_r);

        // Switch knob position
        float knob_center_y = track_y + track_h / 2.0f;
        float knob_y = track_y + track_h / 2.0f - m_track_h * 0.15f +
                       m_anim_pos * (track_h - m_track_h * 0.3f);

        float knob_color_r = m_checked ? 0.85f : 0.1f;
        float knob_color_g = m_checked ? 0.84f : 0.44f;
        float knob_color_b = m_checked ? 0.80f : 0.02f;
        Color knob_fill = Color(knob_color_r, knob_color_g, knob_color_b, 1.0f);

        // Draw knob as circle
        float knob_cx = track_x + track_w / 2.0f;
        float knob_cy = knob_y + m_track_h * 0.15f / 2.0f;
        float knob_r = 10.0f * m_scale;

        for (int i = 0; i < 32; i++) {
            float a0 = 2.0f * 3.14159f * i / 32.0f;
            float a1 = 2.0f * 3.14159f * (i + 1) / 32.0f;
            line(gui, {knob_cx + std::cos(a0) * knob_r, knob_cy + std::sin(a0) * knob_r},
                        {knob_cx + std::cos(a1) * knob_r, knob_cy + std::sin(a1) * knob_r},
                 knob_fill, knob_r * 2.2f);
        }
    }

private:
    float m_x, m_y;
    float m_size_w, m_size_h;
    float m_track_w, m_track_h, m_track_r;
    float m_scale;
    bool m_checked;
    uint32_t m_param_id;
    const char* m_label_off;
    const char* m_label_on;
    float m_anim_pos;
    float m_target_pos;
    bool m_clicked;
    bool m_hovered;
    BypassCallback m_on_toggle;
};

// ============================================================================
// AidaMeter - Peak meter with color-coded fill
// Matches original src/Widgets.hpp AidaMeter
// ============================================================================

class AidaMeter {
public:
    static constexpr float kMeterWidth = 150.0f;
    static constexpr float kMeterHeight = 24.0f;

    AidaMeter(float x, float y, const char* label)
        : m_x(x), m_y(y), m_label(label)
    {
        m_size_w = kMeterWidth;
        m_size_h = kMeterHeight;
        m_value_db = kMinimumMeterDb;
        m_filtered_db = kMinimumMeterDb;
        m_clipping = false;
        m_near_clipping = false;
    }

    void set_scale(float s) { m_scale = s; }

    void set_db(float db) {
        m_value_db = db;
        if (db > kMinimumMeterDb) {
            m_filtered_db = (db + m_filtered_db) * 0.5f;
        }
    }

    bool handle_input(InputState& input, float px, float py) {
        (void)input; (void)px; (void)py;
        return false;
    }

    void render(GuiContext& gui) {
        float w = m_size_w;
        float h = m_size_h;
        float r = 4.0f;
        float pad = 5.0f;

        // Background
        Color bg = Color::from_rgba(0x38, 0x37, 0x5c);
        rect_rounded(gui, {m_x, m_y, w, h}, bg, r);

        // Calculate normalized level
        float norm = 0.0f;
        if (m_filtered_db > kMinimumMeterDb) {
            norm = normalized_level_meter_value(m_filtered_db) / 100.0f;
        }
        norm = std::clamp(norm, 0.0f, 1.0f);

        // Determine color based on level
        Color active_color;
        if (m_value_db > 0.0f || (m_filtered_db > (m_clipping ? -3.0f : 0.0f))) {
            m_clipping = m_near_clipping = true;
            active_color = Color::from_rgba(0xf4, 0x4d, 0x50); // red
        } else if (m_filtered_db > (m_near_clipping ? -6.0f : -3.0f)) {
            m_near_clipping = true;
            active_color = Color::from_rgba(0xf4, 0xf1, 0x4d); // yellow
        } else {
            m_clipping = m_near_clipping = false;
            active_color = Color::from_rgba(0xa4, 0xf4, 0x4d); // green
        }

        // Active level bar
        if (m_filtered_db > kMinimumMeterDb) {
            float fill_w = norm * (w - pad * 2.0f * m_scale);
            if (fill_w > 0.5f) {
                rect_rounded(gui, {m_x + pad * m_scale, m_y + pad * m_scale,
                                   fill_w, h - pad * 2.0f * m_scale},
                             active_color, r);
            }
        }

        // Text
        float font_size = kSubWidgetsFontSize * m_scale;
        Color text_bg = Color::from_rgba(0x38, 0x37, 0x5c);

        // Draw label on left
        text(gui, {m_x + pad * m_scale, m_y + h / 2.0f - font_size / 2.0f,
                    w * 0.4f, font_size},
             m_label, font_size, text_bg);

        // Draw dB value on right
        std::string db_str = format_db();
        text(gui, {m_x + pad * m_scale, m_y + h / 2.0f - font_size / 2.0f,
                    w - pad * 2.0f * m_scale, font_size},
             db_str.c_str(), font_size, text_bg);
    }

    Rect geometry() const { return {m_x, m_y, m_size_w, m_size_h}; }

    float get_width() const { return m_size_w; }
    float get_height() const { return m_size_h; }

private:
    std::string format_db() const {
        char buf[32];
        if (m_filtered_db > kMinimumMeterDb) {
            std::snprintf(buf, sizeof(buf) - 1, "%.1f dB", m_value_db);
        } else {
            std::strncpy(buf, "-inf dB", sizeof(buf) - 1);
        }
        buf[sizeof(buf) - 1] = '\0';
        return std::string(buf);
    }

    float m_x, m_y;
    float m_size_w, m_size_h;
    float m_scale = 1.0f;
    const char* m_label;
    float m_value_db;
    float m_filtered_db;
    bool m_clipping;
    bool m_near_clipping;

    static constexpr float kMinimumMeterDb = -100.0f;
};

// ============================================================================
// AidaSplitter - Vertical decorative separator line
// ============================================================================

class AidaSplitter {
public:
    static constexpr float kLineWidth = 4.0f;
    static constexpr float kLineHeight = 80.0f;

    AidaSplitter(float x, float y)
        : m_x(x), m_y(y)
    {
        m_size_w = kLineWidth;
        m_size_h = kSubWidgetsFullHeight;
    }

    bool handle_input(InputState& input, float px, float py) {
        (void)input; (void)px; (void)py;
        return false;
    }

    void render(GuiContext& gui) {
        float cx = m_x + m_size_w / 2.0f;
        float line_y1 = m_y + m_size_h / 2.0f - kLineHeight / 2.0f;
        float line_y2 = m_y + m_size_h / 2.0f + kLineHeight / 2.0f;
        Color line_color = Color::from_rgba(0x61, 0x61, 0x61, 0.484f);
        line(gui, {cx, line_y1}, {cx, line_y2}, line_color, m_size_w);
    }

    Rect geometry() const { return {m_x, m_y, m_size_w, m_size_h}; }

private:
    float m_x, m_y;
    float m_size_w, m_size_h;
};

// ============================================================================
// AidaFilenameButton - File loader with toggle icon and filename text
// Matches original src/Widgets.hpp AidaFilenameButton
// ============================================================================

class AidaFilenameButton {
public:
    static constexpr float kHeight = 32.0f;
    static constexpr float kRadius = 10.0f;
    static constexpr float kIconSize = 20.0f;
    static constexpr float kInitialWidth = 120.0f;

    AidaFilenameButton(float x, float y, float scale, uint32_t param_id,
                       const char* label,
                       BypassCallback on_toggle,
                       FileLoadedCallback on_load)
        : m_x(x), m_y(y), m_scale(scale)
        , m_param_id(param_id)
        , m_checked(true)
        , m_label(label)
        , m_on_toggle(std::move(on_toggle))
        , m_on_load(std::move(on_load))
    {
        m_size_w = kInitialWidth * scale;
        m_size_h = kHeight * scale;
        m_toggle_w = (kHeight + kSubWidgetsPadding * 2) * scale;
        m_btn_w = m_size_w;
        m_enable_label = "Click to enable " + std::string(label);
        m_disable_label = "Click to disable " + std::string(label);
        m_load_label = "Load " + std::string(label) + "...";
        m_filename = label;
    }

    void set_filename(const char* fname) {
        if (const char* sep = std::strrchr(fname, '/')) {
            m_filename = sep + 1;
        } else if (const char* sep = std::strrchr(fname, '\\')) {
            m_filename = sep + 1;
        } else {
            m_filename = fname;
        }
    }
    void set_checked(bool checked) { m_checked = checked; }

    bool handle_input(InputState& input, float px, float py) {
        bool hit = false;

        // Toggle switch area (left part)
        Rect toggle_r = {m_x, m_y, m_toggle_w, m_size_h};
        bool toggle_hit = px >= toggle_r.x && px < toggle_r.x + toggle_r.w &&
                          py >= toggle_r.y && py < toggle_r.y + toggle_r.h;

        if (toggle_hit && input.mouse_down[0] && !m_toggle_clicked) {
            m_toggle_clicked = true;
            m_checked = !m_checked;
            if (m_on_toggle) {
                m_on_toggle(m_param_id, m_checked);
            }
            hit = true;
        }
        if (!input.mouse_down[0]) m_toggle_clicked = false;

        // Button text area (right part)
        float btn_x = m_x + m_toggle_w - 4.0f * m_scale;
        Rect btn_r = {btn_x, m_y, m_btn_w - 4.0f * m_scale, m_size_h};
        bool btn_hit = px >= btn_r.x && px < btn_r.x + btn_r.w &&
                       py >= btn_r.y && py < btn_r.y + btn_r.h;
        m_btn_hovered = btn_hit;

        if (btn_hit && input.mouse_down[0] && !m_btn_clicked) {
            m_btn_clicked = true;
            if (m_on_load) {
                m_on_load(m_filename.c_str());
            }
            hit = true;
        }
        if (!input.mouse_down[0]) m_btn_clicked = false;

        return hit;
    }

    void render(GuiContext& gui) {
        float font_size = 16.0f * m_scale;
        float icon_size = kIconSize * m_scale;

        // Toggle switch background
        Color switch_bg;
        if (m_checked) {
            switch_bg = Color::from_rgba(0x15, 0x2c, 0x0d);
        } else {
            switch_bg = Color::from_rgba(0x74, 0x92, 0x32);
        }
        rect_rounded(gui, {m_x, m_y, m_toggle_w, m_size_h}, switch_bg, kRadius * m_scale);

        // Right half of toggle (slider part)
        float slider_x = m_x + m_size_h / 2.0f;
        float slider_w = m_toggle_w - m_size_h / 2.0f;
        rect(gui, {slider_x, m_y, slider_w, m_size_h}, switch_bg);

        // Icon indicator circle
        float icon_cx = m_x + m_size_h / 2.0f;
        float icon_cy = m_y + m_size_h / 2.0f;
        float icon_r = icon_size * 0.35f;
        Color icon_color = m_checked ? Color::from_rgba(0xa4, 0xf7, 0x00, 0.9f)
                                      : Color::from_rgba(0x80, 0x80, 0x80, 0.7f);
        for (int i = 0; i < 16; i++) {
            float a0 = 2.0f * 3.14159f * i / 16.0f;
            float a1 = 2.0f * 3.14159f * (i + 1) / 16.0f;
            line(gui, {icon_cx + std::cos(a0) * icon_r, icon_cy + std::sin(a0) * icon_r},
                        {icon_cx + std::cos(a1) * icon_r, icon_cy + std::sin(a1) * icon_r},
                 icon_color, 2.5f * m_scale);
        }

        // Button area
        float btn_x = m_x + m_toggle_w - 4.0f * m_scale;
        Color btn_bg = m_btn_hovered ? Color::from_rgba(0x74, 0x92, 0x32)
                                      : Color::from_rgba(0x56, 0x73, 0x28);
        rect_rounded(gui, {btn_x, m_y, m_btn_w - btn_x + m_x, m_size_h}, btn_bg, kRadius * m_scale);

        // Button text
        const char* display_text;
        if (m_btn_hovered) {
            display_text = m_load_label.c_str();
        } else {
            display_text = m_checked ? m_disable_label.c_str() : m_enable_label.c_str();
        }

        text(gui, {btn_x + 8.0f * m_scale, m_y + 4.0f * m_scale,
                    m_btn_w - btn_x - 16.0f * m_scale, font_size + 4.0f * m_scale},
             display_text, font_size, Color::white());
    }

private:
    float m_x, m_y;
    float m_size_w, m_size_h;
    float m_scale;
    float m_toggle_w;
    float m_btn_w;
    bool m_checked;
    uint32_t m_param_id;
    const char* m_label;
    std::string m_filename;
    std::string m_enable_label;
    std::string m_disable_label;
    std::string m_load_label;
    bool m_toggle_clicked = false;
    bool m_btn_clicked = false;
    bool m_btn_hovered = false;
    BypassCallback m_on_toggle;
    FileLoadedCallback m_on_load;
};

// ============================================================================
// AidaPushButton - Simple rectangular push button
// ============================================================================

class AidaPushButton {
public:
    static constexpr float kWidth = 140.0f;
    static constexpr float kHeight = 25.0f;
    static constexpr float kRadius = 4.0f;

    AidaPushButton(float x, float y, const char* label)
        : m_x(x), m_y(y), m_label(label ? label : "")
    {
        m_size_w = kWidth;
        m_size_h = kHeight;
    }

    bool handle_input(InputState& input, float px, float py) {
        Rect r = {m_x, m_y, m_size_w, m_size_h};
        bool hit = px >= r.x && px < r.x + r.w && py >= r.y && py < r.y + r.h;
        m_hovered = hit;

        if (hit && input.mouse_down[0] && !m_clicked) {
            m_clicked = true;
            m_clicked_frame = true;
        }
        if (!input.mouse_down[0]) m_clicked = false;

        return hit;
    }

    void render(GuiContext& gui) {
        Color fill, stroke;
        switch (m_clicked_frame ? 1 : (m_hovered ? 2 : 3)) {
        case 1:
            fill = Color::from_rgba(0x15, 0x14, 0x13, 0.4f);
            stroke = fill;
            m_clicked_frame = false;
            break;
        case 2:
            fill = Color::from_rgba(0x15, 0x14, 0x13, 0.8f);
            stroke = fill;
            break;
        default:
            fill = Color::from_rgba(0x15, 0x14, 0x13);
            stroke = Color::from_rgba(0x15, 0x14, 0x13, 0.8f);
            break;
        }

        rect_rounded(gui, {m_x, m_y, m_size_w, m_size_h}, fill, kRadius);
        line(gui, {m_x, m_y}, {m_x + m_size_w, m_y}, stroke, 1.0f);
        line(gui, {m_x, m_y + m_size_h}, {m_x + m_size_w, m_y + m_size_h}, stroke, 1.0f);
        line(gui, {m_x, m_y}, {m_x, m_y + m_size_h}, stroke, 1.0f);
        line(gui, {m_x + m_size_w, m_y}, {m_x + m_size_w, m_y + m_size_h}, stroke, 1.0f);

        if (!m_label.empty()) {
            text_centered(gui, m_x + m_size_w / 2.0f, m_y + m_size_h / 2.0f,
                          m_label.c_str(), 16.0f, Color::white());
        }
    }

private:
    float m_x, m_y;
    float m_size_w, m_size_h;
    std::string m_label;
    bool m_hovered = false;
    bool m_clicked = false;
    bool m_clicked_frame = false;
};

} // namespace AidaGui
