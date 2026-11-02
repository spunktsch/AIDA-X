// AIDA-X WGPU+egui Backend - Custom Widgets
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "Gui.hpp"
#include "WgpuBackend.hpp"
#include <cmath>
#include <cstring>

namespace AidaGui {

// Helper: convert degrees to radians
static inline float deg_to_rad(float deg) {
    return deg * 3.14159265358979323846f / 180.0f;
}

// ============================================================================
// AidaKnob - Rotating knob with scale markings and value display
// Matches the original AidaKnob widget from src/Widgets.hpp
// ============================================================================

class AidaKnob {
public:
    static constexpr float kScaleSize = 80.0f;
    static constexpr float kKnobSize = 55.0f;
    static constexpr float kKnobMargin = (kScaleSize - kKnobSize) / 2.0f;
    static constexpr float kFullHeight = 90.0f;

    AidaKnob(float x, float y, float scale, ParamChangedCallback on_change)
        : m_x(x), m_y(y), m_scale(scale), m_on_change(std::move(on_change))
    {
        m_size = {kScaleSize * scale, kFullHeight * scale};
        m_knob_size = kKnobSize * scale;
        m_knob_half = m_knob_size / 2.0f;
        m_knob_margin = kKnobMargin * scale;
        m_font_size = 14.0f * scale;
        m_value = 0.5f; // center position
        m_default_value = 0.5f;
    }

    void set_value(float v) { m_value = v; }
    void set_range(float min, float max) { m_min = min; m_max = max; }

    bool handle_input(InputState& input, float px, float py) {
        float cx = m_x + m_size.w / 2.0f;
        float cy = m_y + m_knob_margin + m_knob_half + m_size.h * 0.2f;
        float dist = std::sqrt((px - cx) * (px - cx) + (py - cy) * (py - cy));

        bool hit = dist < m_knob_size * 0.6f;
        m_hovered = hit && !m_pressed;
        m_active = hit && m_pressed;

        if (hit && input.mouse_down[0] && !m_pressed) {
            m_pressed = true;
            m_drag_start_x = px;
            m_drag_start_val = m_value;
        }

        if (m_pressed && !input.mouse_down[0]) {
            m_pressed = false;
        }

        if (m_pressed) {
            float dx = px - m_drag_start_x;
            float delta = dx / (m_knob_size * 0.5f) * 0.5f;
            m_value = std::clamp(m_drag_start_val + delta, 0.0f, 1.0f);

            float param_value = m_min + m_value * (m_max - m_min);
            if (m_on_change) {
                m_on_change(m_param_id, param_value);
            }
        }

        // Shift or double-click to reset
        if (hit && (input.shift || input.mouse_down[2])) {
            m_value = m_default_value;
            float param_value = m_min + m_value * (m_max - m_min);
            if (m_on_change) {
                m_on_change(m_param_id, param_value);
            }
        }

        return hit;
    }

    void render(GuiContext& gui) {
        float cx = m_x + m_size.w / 2.0f;
        float cy = m_y + m_knob_margin + m_knob_half + m_size.h * 0.2f;
        float r = m_knob_size * 0.5f;

        // Draw scale background
        Color scale_bg = Color::from_rgba(0x27, 0x28, 0x34);
        rect_rounded({m_x, m_y, m_size.w, m_size.h * 0.6f}, scale_bg, 4.0f * m_scale);

        // Draw tick marks on scale
        float tick_start_y = m_y + m_size.h * 0.15f;
        float tick_end_y = m_y + m_size.h * 0.25f;
        for (int i = 0; i <= 10; i++) {
            float tx = m_x + m_size.w * 0.2f + m_size.w * 0.6f * (i / 10.0f);
            float tick_h = (i == 0 || i == 10) ? 6.0f * m_scale : 3.0f * m_scale;
            Color tick_color = Color::from_rgba(0x50, 0x50, 0x50);
            if (i == 5) tick_color = Color::from_rgba(0x80, 0x80, 0x80);
            line({tx, tick_start_y}, {tx, tick_start_y + tick_h}, tick_color, 1.0f);
        }

        // Draw knob circle
        float angle = deg_to_rad(-135.0f + 270.0f * m_value);
        float knob_color_r = 0.11f, knob_color_g = 0.14f, knob_color_b = 0.05f;
        Color knob_fill = Color(knob_color_r, knob_color_g, knob_color_b, 1.0f);

        // Knob body
        rect({cx - r, cy - r, m_knob_size, m_knob_size}, knob_fill);

        // Knob indicator line
        Vec2 indicator_end = {
            cx + std::cos(angle) * r * 0.7f,
            cy + std::sin(angle) * r * 0.7f
        };
        line({cx, cy}, indicator_end, Color::from_rgba(0xa4, 0xf4, 0x4d), 2.5f * m_scale);

        // Knob outer ring
        Color ring_color = m_active ? Color::from_rgba(0xa4, 0xf4, 0x4d, 0.8f)
                                     : Color::from_rgba(0x55, 0x55, 0x55, 0.4f);
        // Draw ring as a series of small lines forming a circle
        for (int i = 0; i < 64; i++) {
            float a1 = 2.0f * 3.14159f * i / 64.0f;
            float a2 = 2.0f * 3.14159f * (i + 1) / 64.0f;
            float rr = r + 2.0f * m_scale;
            line({cx + std::cos(a1) * rr, cy + std::sin(a1) * rr},
                 {cx + std::cos(a2) * rr, cy + std::sin(a2) * rr}, ring_color, 1.0f);
        }

        // Draw label
        if (m_label && m_label[0]) {
            float text_y = m_y + m_size.h - m_size.h * 0.1f;
            // Simplified: draw text as a colored bar with position marker
            text({m_x + 2.0f * m_scale, text_y, m_size.w - 4.0f * m_scale, m_font_size + 4.0f},
                 m_label, m_font_size, Color::from_rgba(0x90, 0x90, 0x90));
        }

        // Value display when dragging
        if (m_active || m_hovered) {
            float val_bg_y = cy + r + 4.0f * m_scale;
            float val_bg_h = m_font_size + 6.0f * m_scale;
            Color val_bg = Color::from_rgba(0, 0, 0, 0.5f);
            rect_rounded({cx - 30.0f * m_scale, val_bg_y, 60.0f * m_scale, val_bg_h}, val_bg, 2.0f * m_scale);

            char buf[24];
            std::snprintf(buf, sizeof(buf) - 1, "%.2f", m_value);
            buf[sizeof(buf) - 1] = '\0';
            text({cx - 25.0f * m_scale, val_bg_y + 2.0f * m_scale, 50.0f * m_scale, m_font_size},
                 buf, m_font_size, Color::white());
        }
    }

    void set_label(const char* label) { m_label = label; }
    void set_default(float def) { m_default_value = def; }

    Rect geometry() const { return {m_x, m_y, m_size.w, m_size.h}; }

private:
    float m_x, m_y;
    Vec2 m_size;
    float m_scale;
    float m_knob_size;
    float m_knob_half;
    float m_knob_margin;
    float m_font_size;
    float m_value;
    float m_min = 0.0f;
    float m_max = 1.0f;
    float m_default_value = 0.5f;
    float m_drag_start_x = 0.0f;
    float m_drag_start_val = 0.0f;
    bool m_pressed = false;
    bool m_hovered = false;
    bool m_active = false;
    uint32_t m_param_id = 0;
    const char* m_label = nullptr;
    ParamChangedCallback m_on_change;
};

// ============================================================================
// AidaSwitch - Vertical toggle switch with ON/OFF labels
// Matches the original AidaPluginSwitch widget
// ============================================================================

class AidaSwitch {
public:
    static constexpr float kSwitchWidth = 25.0f;
    static constexpr float kSwitchHeight = 60.0f;
    static constexpr float kSwitchRadius = kSwitchWidth / 2.0f;
    static constexpr float kFullWidth = kSwitchWidth + 16.0f;

    AidaSwitch(float x, float y, float scale, uint32_t param_id,
               const char* label_off, const char* label_on,
               BypassCallback on_toggle)
        : m_x(x), m_y(y), m_scale(scale)
        , m_checked(false), m_param_id(param_id)
        , m_label_off(label_off), m_label_on(label_on)
        , m_on_toggle(std::move(on_toggle))
    {
        m_size = {kFullWidth * scale, kSubWidgetsFullHeight * scale};
    }

    void set_checked(bool checked) { m_checked = checked; }
    bool is_checked() const { return m_checked; }

    bool handle_input(InputState& input, float px, float py) {
        Rect r = {m_x, m_y, m_size.w, m_size.h};
        bool hit = px >= r.x && px < r.x + r.w && py >= r.y && py < r.y + r.h;
        m_hovered = hit && !m_clicked;

        if (hit && input.mouse_down[0] && !m_clicked) {
            m_clicked = true;
            m_checked = !m_checked;
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
        float sw = kSwitchWidth * m_scale;
        float sh = kSwitchHeight * m_scale;
        float sr = kSwitchRadius * m_scale;
        float padding = 8.0f * m_scale;
        float label_size = 14.0f * m_scale;

        // ON/OFF labels
        Color text_color = Color::from_rgba(0xe0, 0xe0, 0xe0);
        text({m_x + padding, m_y, m_size.w - padding * 2, label_size},
             m_checked ? m_label_on : m_label_off, label_size, text_color);

        // Switch track
        float track_x = m_x + padding;
        float track_y = m_y + m_size.h * 0.3f;
        float track_w = sw;
        float track_h = sh * 0.55f;

        Color track_bg = m_checked ? Color::from_rgba(0x54, 0x54, 0x54)
                                    : Color::from_rgba(0xa4, 0xf7, 0x00);
        rect_rounded({track_x, track_y, track_w, track_h}, track_bg, sr);

        // Switch knob
        float knob_y = m_checked
            ? track_y + track_h - sw / 2.0f
            : track_y;
        Color knob_fill = m_checked ? Color::from_rgba(0xda, 0xd6, 0xcb)
                                     : Color::from_rgba(0x18, 0x70, 0x04);
        rect({knob_x, knob_y, sw, sw}, knob_fill);

        // Draw a circle for the knob
        float cx = m_x + padding + sw / 2.0f;
        float cy = knob_y + sw / 2.0f;
        float kR = 10.0f * m_scale;
        for (int i = 0; i < 32; i++) {
            float a1 = 2.0f * 3.14159f * i / 32.0f;
            float a2 = 2.0f * 3.14159f * (i + 1) / 32.0f;
            line({cx + std::cos(a1) * kR, cy + std::sin(a1) * kR},
                 {cx + std::cos(a2) * kR, cy + std::sin(a2) * kR}, knob_fill, 1.0f);
        }
    }

    Rect geometry() const { return {m_x, m_y, m_size.w, m_size.h}; }

private:
    float m_x, m_y;
    Vec2 m_size;
    float m_scale;
    bool m_checked;
    uint32_t m_param_id;
    const char* m_label_off;
    const char* m_label_on;
    bool m_clicked = false;
    bool m_hovered = false;
    BypassCallback m_on_toggle;
};

static constexpr float kSubWidgetsFullHeight = 90.0f;

// ============================================================================
// AidaMeter - Level meter with clipping indicators
// Matches the original AidaMeter widget
// ============================================================================

class AidaMeter {
public:
    static constexpr float kMeterWidth = 150.0f;
    static constexpr float kMeterHeight = 16.0f;

    AidaMeter(float x, float y, const char* label)
        : m_x(x), m_y(y), m_label(label)
    {
        m_size = {kMeterWidth, kMeterHeight};
        m_value_db = -inf_db;
        m_clipping = false;
        m_near_clipping = false;
    }

    void set_db(float db) {
        if (std::abs(m_value_db - db) < 0.01f) return;
        m_value_db = db;
        m_filtered_db = (m_value_db + m_filtered_db) / 2.0f;
    }

    bool handle_input(InputState& input, float px, float py) {
        // Meters are read-only
        return false;
    }

    void render(GuiContext& gui) {
        float w = m_size.w;
        float h = m_size.h;
        float r = 4.0f;
        float pad = 4.0f;

        // Background
        Color bg = Color::from_rgba(0x38, 0x37, 0x5c);
        rect_rounded({m_x, m_y, w, h}, bg, r);

        // Calculate normalized level
        float norm = normalized_level(m_filtered_db);

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
        if (m_filtered_db > -inf_db) {
            rect_rounded({m_x + 2.0f, m_y + 2.0f,
                          norm * (w - 4.0f), h - 4.0f},
                         active_color, r);
        }

        // Text (label and value)
        float font_size = 14.0f;

        // Label
        text({m_x + pad, m_y + h / 2.0f - font_size / 2.0f, w - pad * 2, font_size},
             m_label, font_size, bg);

        // Value string
        char buf[32];
        if (m_filtered_db > -inf_db) {
            std::snprintf(buf, sizeof(buf) - 1, "%.1f dB", m_value_db);
        } else {
            std::strncpy(buf, "-inf dB", sizeof(buf) - 1);
            buf[sizeof(buf) - 1] = '\0';
        }

        text({m_x + pad, m_y + h / 2.0f - font_size / 2.0f, w - pad * 2, font_size},
             buf, font_size, bg);
    }

    Rect geometry() const { return {m_x, m_y, m_size.w, m_size.h}; }

private:
    float normalized_level(float db) const {
        if (db < -70.0f) return 0.0f;
        if (db < -60.0f) return (db + 70.0f) * 0.25f;
        if (db < -50.0f) return (db + 60.0f) * 0.50f + 2.5f;
        if (db < -40.0f) return (db + 50.0f) * 0.75f + 7.5f;
        if (db < -30.0f) return (db + 40.0f) * 1.50f + 15.0f;
        if (db < -20.0f) return (db + 30.0f) * 2.00f + 30.0f;
        if (db < 0.0f)  return (db + 20.0f) * 2.50f + 50.0f;
        return 100.0f;
    }

    float m_x, m_y;
    Vec2 m_size;
    const char* m_label;
    float m_value_db = -100.0f;
    float m_filtered_db = -100.0f;
    static constexpr float kMinimumMeterDb = -100.0f;
    bool m_clipping;
    bool m_near_clipping;
};

static constexpr float kMinimumMeterDb = -100.0f;
static constexpr float inf_db = kMinimumMeterDb;

// ============================================================================
// AidaSplitter - Vertical separator line
// ============================================================================

class AidaSplitter {
public:
    static constexpr float kLineWidth = 4.0f;
    static constexpr float kLineHeight = 80.0f;

    AidaSplitter(float x, float y)
        : m_x(x), m_y(y)
    {
        m_size = {kLineWidth, kSubWidgetsFullHeight};
    }

    bool handle_input(InputState& input, float px, float py) { return false; }

    void render(GuiContext& gui) {
        float cx = m_x + m_size.w / 2.0f;
        float y1 = m_y + m_size.h / 2.0f - kLineHeight / 2.0f;
        float y2 = m_y + m_size.h / 2.0f + kLineHeight / 2.0f;
        Color line_color = Color::from_rgba(0x61, 0x61, 0x61, 0.484f);
        line({cx, y1}, {cx, y2}, line_color, m_size.w);
    }

    Rect geometry() const { return {m_x, m_y, m_size.w, m_size.h}; }

private:
    float m_x, m_y;
    Vec2 m_size;
};

// ============================================================================
// AidaFilenameButton - Button with toggle switch for loading files
// ============================================================================

class AidaFilenameButton {
public:
    static constexpr float kHeight = 32.0f;
    static constexpr float kRadius = 10.0f;
    static constexpr float kIconSize = 20.0f;
    static constexpr float kInitialWidth = 120.0f;

    AidaFilenameButton(float x, float y, float scale, uint32_t param_id,
                       const char* label,
                       BypassCallback on_toggle, FileLoadedCallback on_load)
        : m_x(x), m_y(y), m_scale(scale)
        , m_checked(true), m_param_id(param_id)
        , m_label(label)
        , m_on_toggle(std::move(on_toggle))
        , m_on_load(std::move(on_load))
    {
        m_size = {kInitialWidth * scale, kHeight * scale};
        m_toggle_w = (kHeight + 16.0f) * scale;
        m_btn_w = kInitialWidth * scale;
        m_enable_label = "Click to enable " + std::string(label);
        m_disable_label = "Click to disable " + std::string(label);
        m_load_label = "Load " + std::string(label) + "...";
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

    bool handle_input(InputState& input, float px, float py) {
        bool hit = false;

        // Toggle switch area
        Rect toggle_r = {m_x, m_y, m_toggle_w, m_size.h};
        if (px >= toggle_r.x && px < toggle_r.x + toggle_r.w &&
            py >= toggle_r.y && py < toggle_r.y + toggle_r.h) {
            hit = true;
            if (input.mouse_down[0] && !m_toggle_clicked) {
                m_toggle_clicked = true;
                m_checked = !m_checked;
                if (m_on_toggle) {
                    m_on_toggle(m_param_id, m_checked);
                }
            }
            if (!input.mouse_down[0]) m_toggle_clicked = false;
        }

        // Button area
        Rect btn_r = {m_x + 8.0f * m_scale, m_y, m_btn_w - 8.0f * m_scale, m_size.h};
        bool btn_hit = px >= btn_r.x && px < btn_r.x + btn_r.w &&
                       py >= btn_r.y && py < btn_r.y + btn_r.h;
        m_btn_hovered = btn_hit;

        if (btn_hit && input.mouse_down[0] && !m_btn_clicked) {
            m_btn_clicked = true;
            if (m_on_load) {
                m_on_load(m_filename.c_str(), m_param_id == 0); // param_id 0 = model
            }
        }
        if (!input.mouse_down[0]) m_btn_clicked = false;

        return hit || btn_hit;
    }

    void render(GuiContext& gui) {
        // Toggle switch
        float icon_size = kIconSize * m_scale;
        float toggle_x = m_x;
        float toggle_y = m_y;

        // Switch background
        Color switch_bg = m_checked ? Color::from_rgba(0x15, 0x2c, 0x0d)
                                     : Color::from_rgba(0x74, 0x92, 0x32);
        rect_rounded({toggle_x, toggle_y, m_toggle_w, m_size.h}, switch_bg, kRadius * m_scale);

        // Right half of switch (the slider part)
        rect({toggle_x + m_size.h / 2.0f, toggle_y,
              m_toggle_w - m_size.h / 2.0f, m_size.h},
             switch_bg);

        // Icon (simple circle indicator)
        float icon_x = toggle_x + m_size.h / 2.0f - icon_size / 2.0f;
        float icon_y = toggle_y + m_size.h / 2.0f - icon_size / 2.0f;
        Color icon_color = m_checked ? Color::from_rgba(0xa4, 0xf7, 0x00)
                                      : Color::from_rgba(0x60, 0x60, 0x60);
        // Draw small circle
        for (int i = 0; i < 16; i++) {
            float a1 = 2.0f * 3.14159f * i / 16.0f;
            float a2 = 2.0f * 3.14159f * (i + 1) / 16.0f;
            float r = icon_size * 0.4f;
            line({icon_x + icon_size / 2.0f + std::cos(a1) * r,
                  icon_y + icon_size / 2.0f + std::sin(a1) * r},
                 {icon_x + icon_size / 2.0f + std::cos(a2) * r,
                  icon_y + icon_size / 2.0f + std::sin(a2) * r}, icon_color, 2.0f * m_scale);
        }

        // Button area
        Color btn_bg = m_btn_hovered ? Color::from_rgba(0x74, 0x92, 0x32)
                                      : Color::from_rgba(0x56, 0x73, 0x28);
        rect_rounded({toggle_x + 8.0f * m_scale, toggle_y,
                      m_btn_w - 8.0f * m_scale, m_size.h}, btn_bg, kRadius * m_scale);

        // Button text
        float font_size = 16.0f * m_scale;
        const char* display_text = m_checked ? m_disable_label.c_str() : m_enable_label.c_str();
        if (m_btn_hovered) display_text = m_load_label.c_str();

        text({toggle_x + 24.0f * m_scale, toggle_y + 4.0f * m_scale,
              m_btn_w - 32.0f * m_scale, font_size},
             display_text, font_size, Color::white());
    }

    Rect geometry() const { return {m_x, m_y, m_size.w, m_size.h}; }

private:
    float m_x, m_y;
    Vec2 m_size;
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
        : m_x(x), m_y(y), m_label(label)
    {
        m_size = {kWidth, kHeight};
    }

    bool handle_input(InputState& input, float px, float py) {
        Rect r = {m_x, m_y, m_size.w, m_size.h};
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
        case 1: // clicked
            fill = Color::from_rgba(0x15, 0x14, 0x13, 0.4f);
            stroke = fill;
            m_clicked_frame = false;
            break;
        case 2: // hover
            fill = Color::from_rgba(0x15, 0x14, 0x13, 0.8f);
            stroke = fill;
            break;
        default: // default
            fill = Color::from_rgba(0x15, 0x14, 0x13);
            stroke = Color::from_rgba(0x15, 0x14, 0x13, 0.8f);
            break;
        }

        rect_rounded({m_x, m_y, m_size.w, m_size.h}, fill, kRadius);
        // Draw border as thin rectangles
        line({m_x, m_y}, {m_x + m_size.w, m_y}, stroke, 1.0f);
        line({m_x, m_y + m_size.h}, {m_x + m_size.w, m_y + m_size.h}, stroke, 1.0f);
        line({m_x, m_y}, {m_x, m_y + m_size.h}, stroke, 1.0f);
        line({m_x + m_size.w, m_y}, {m_x + m_size.w, m_y + m_size.h}, stroke, 1.0f);

        if (m_label && m_label[0]) {
            text({m_x, m_y, m_size.w, m_size.h},
                 m_label, 16.0f, Color::white());
        }
    }

    Rect geometry() const { return {m_x, m_y, m_size.w, m_size.h}; }

private:
    float m_x, m_y;
    Vec2 m_size;
    const char* m_label;
    bool m_hovered = false;
    bool m_clicked = false;
    bool m_clicked_frame = false;
};

} // namespace AidaGui
