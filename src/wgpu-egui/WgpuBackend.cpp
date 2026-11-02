// AIDA-X WGPU+egui Backend - WGPU Backend Implementation
// Copyright (C) 2023 AidaDSP
// SPDX-License-Identifier: GPL-3.0-or-later

#include "WgpuBackend.hpp"
#include "wgpu.h"

#include <cstdio>
#include <cstring>

// -----------------------------------------------------------------------
// Shader sources for rendering shapes
// -----------------------------------------------------------------------

static const char* kVertexShader = R"(
struct Vertex {
    location(0) pos: vec2<f32>,
    location(1) uv: vec2<f32>,
    location(2) color: vec4<f32>,
}

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(v: Vertex) -> VSOut {
    var out: VSOut;
    out.position = vec4<f32>(v.pos, 0.0, 1.0);
    out.uv = v.uv;
    out.color = v.color;
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    return in.color;
}
)";

static const char* kLineVertexShader = R"(
struct Vertex {
    location(0) pos: vec2<f32>,
    location(1) color: vec4<f32>,
    location(2) thickness: f32,
}

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(v: Vertex) -> VSOut {
    var out: VSOut;
    out.position = vec4<f32>(v.pos, 0.0, 1.0);
    out.color = v.color;
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    return in.color;
}
)";

// -----------------------------------------------------------------------
// WgpuBackend implementation
// -----------------------------------------------------------------------

WgpuBackend::~WgpuBackend() {
    shutdown();
}

bool WgpuBackend::init(const WgpuConfig& config) {
    m_width = config.width;
    m_height = config.height;

    if (!create_instance()) return false;
    if (!create_adapter()) return false;
    if (!create_device()) return false;
    if (!create_surface(nullptr)) return false;
    resize(m_width, m_height);
    return true;
}

bool WgpuBackend::init_embedded(void* parent_window, const WgpuConfig& config) {
    m_width = config.width;
    m_height = config.height;
    m_native_window = parent_window;

    if (!create_instance()) return false;
    if (!create_adapter()) return false;
    if (!create_device()) return false;
    if (!create_surface(parent_window)) return false;
    resize(m_width, m_height);
    return true;
}

void WgpuBackend::shutdown() {
    if (m_device) {
        wgpuDeviceRelease(m_device);
        m_device = nullptr;
    }
    if (m_swap_chain) {
        wgpuSwapChainRelease(m_swap_chain);
        m_swap_chain = nullptr;
    }
    if (m_surface) {
        wgpuSurfaceRelease(m_surface);
        m_surface = nullptr;
    }
    if (m_adapter) {
        wgpuAdapterRelease(m_adapter);
        m_adapter = nullptr;
    }
    if (m_instance) {
        wgpuInstanceRelease(m_instance);
        m_instance = nullptr;
    }
}

void WgpuBackend::poll_events() {
    // wgpu-native doesn't handle events directly;
    // platform-specific code should call this
    // For now, we rely on the application to handle events
}

bool WgpuBackend::begin_frame() {
    if (!m_device || !m_surface) return false;

    WGPUSurfaceTexture surface_texture;
    WGPUSurfaceGetCurrentTexture(m_surface, &surface_texture);

    switch (surface_texture.status) {
    case WGPUSurfaceGetCurrentTextureStatus_SuccessOptimal:
    case WGPUSurfaceGetCurrentTextureStatus_SuccessSuboptimal:
        break;
    case WGPUSurfaceGetCurrentTextureStatus_Timeout:
    case WGPUSurfaceGetCurrentTextureStatus_Outdated:
    case WGPUSurfaceGetCurrentTextureStatus_Lost:
        if (surface_texture.texture) {
            wgpuTextureRelease(surface_texture.texture);
        }
        // Reconfigure on lost/outdated
        if (surface_texture.texture == nullptr) {
            resize(m_width, m_height);
        }
        return false;
    case WGPUSurfaceGetCurrentTextureStatus_Force32:
        return false;
    }

    m_swap_chain_texture = surface_texture.texture;
    if (m_swap_chain_view) {
        wgpuTextureViewRelease(m_swap_chain_view);
    }
    m_swap_chain_view = wgpuTextureCreateView(m_swap_chain_texture, nullptr);
    m_queue = wgpuDeviceGetQueue(m_device);
    wgpuQueueRelease(m_queue);

    return m_swap_chain_view != nullptr;
}

void WgpuBackend::end_frame(GuiContext& gui_ctx, const AidaGui::Mesh& mesh) {
    if (!m_device || !m_swap_chain_view) return;

    WGPUCommandEncoder command_encoder = wgpuDeviceCreateCommandEncoder(
        m_device, nullptr);

    WGPURenderPassDescriptor render_pass_desc = {};
    render_pass_desc.colorAttachmentCount = 1;
    render_pass_desc.colorAttachments = &(WGPURenderPassColorAttachment){
        .view = m_swap_chain_view,
        .loadOp = WGPULoadOp_Clear,
        .storeOp = WGPUStoreOp_Store,
        .depthSlice = WGPU_DEPTH_SLICE_UNDEFINED,
        .clearValue = {0.08f, 0.08f, 0.10f, 1.0f}, // Dark background
    };

    WGPURenderPassEncoder render_pass = wgpuCommandEncoderBeginRenderPass(
        command_encoder, &render_pass_desc);

    // TODO: Set pipeline and draw mesh
    // For now, just clear
    wgpuRenderPassEncoderEnd(render_pass);
    wgpuRenderPassEncoderRelease(render_pass);

    WGPUCommandBuffer command_buffer = wgpuCommandEncoderFinish(
        command_encoder, nullptr);

    wgpuQueueSubmit(m_queue, 1, &command_buffer);
    wgpuSurfacePresent(m_surface);

    wgpuCommandBufferRelease(command_buffer);
    wgpuCommandEncoderRelease(command_encoder);
}

void WgpuBackend::set_size(uint32_t w, uint32_t h) {
    m_width = w;
    m_height = h;
    resize(w, h);
}

void WgpuBackend::get_size(uint32_t* w, uint32_t* h) const {
    if (w) *w = m_width;
    if (h) *h = m_height;
}

void WgpuBackend::set_title(const char* title) {
    // Platform-specific
    (void)title;
}

bool WgpuBackend::is_closed() const {
    return m_closed;
}

void WgpuBackend::close() {
    m_closed = true;
}

// -----------------------------------------------------------------------
// Internal implementation
// -----------------------------------------------------------------------

static void instance_request_adapter_callback(
    WGPURequestAdapterStatus status,
    WGPUAdapter adapter,
    WGPUStringView message,
    void* userdata1,
    void* userdata2)
{
    (void)message;
    (void)userdata2;
    if (status == WGPURequestAdapterStatus_Success) {
        auto* self = static_cast<WgpuBackend*>(userdata1);
        self->m_adapter = adapter;
    }
}

static void device_request_callback(
    WGPURequestDeviceStatus status,
    WGPUDevice device,
    WGPUStringView message,
    void* userdata1,
    void* userdata2)
{
    (void)message;
    (void)userdata2;
    if (status == WGPURequestDeviceStatus_Success) {
        auto* self = static_cast<WgpuBackend*>(userdata1);
        self->m_device = device;
    }
}

bool WgpuBackend::create_instance() {
    m_instance = wgpuCreateInstance(nullptr);
    return m_instance != nullptr;
}

bool WgpuBackend::create_adapter() {
    if (!m_instance) return false;

    wgpuInstanceRequestAdapter(m_instance,
        &(WGPURequestAdapterOptions){
            .compatibleSurface = m_surface,
        },
        (WGPURequestAdapterCallbackInfo){
            .callback = instance_request_adapter_callback,
            .userdata1 = this,
        });

    return m_adapter != nullptr;
}

bool WgpuBackend::create_device() {
    if (!m_adapter) return false;

    wgpuAdapterRequestDevice(m_adapter, nullptr,
        (WGPURequestDeviceCallbackInfo){
            .callback = device_request_callback,
            .userdata1 = this,
        });

    return m_device != nullptr;
}

bool WgpuBackend::create_surface(void* parent_window) {
    if (!m_instance) return false;

    WGPUSurfaceDescriptor surface_desc = {};

#ifdef _WIN32
    // Windows: use HWND
    if (parent_window) {
        WGPUSurfaceSourceWindowsHWND hwnd_source = {};
        hwnd_source.chain.sType = WGPUSType_SurfaceSourceWindowsHWND;
        surface_desc.nextInChain = &hwnd_source.chain;

        // Get HINSTANCE
        HINSTANCE hinstance = GetModuleHandle(nullptr);
        ((WGPUSurfaceSourceWindowsHWND*)surface_desc.nextInChain)->hinstance = hinstance;
        ((WGPUSurfaceSourceWindowsHWND*)surface_desc.nextInChain)->hwnd = parent_window;
    }
#elif defined(__linux__) && defined(GLFW_EXPOSE_NATIVE_X11)
    if (parent_window) {
        WGPUSurfaceSourceXlibWindow xlib_source = {};
        xlib_source.chain.sType = WGPUSType_SurfaceSourceXlibWindow;
        surface_desc.nextInChain = &xlib_source.chain;
        ((WGPUSurfaceSourceXlibWindow*)surface_desc.nextInChain)->display = nullptr;
        ((WGPUSurfaceSourceXlibWindow*)surface_desc.nextInChain)->window = reinterpret_cast<uintptr_t>(parent_window);
    }
#elif defined(__APPLE__)
    // macOS: we'll handle this differently
    (void)parent_window;
#endif

    m_surface = wgpuInstanceCreateSurface(m_instance, &surface_desc);
    return m_surface != nullptr;
}

void WgpuBackend::resize(uint32_t w, uint32_t h) {
    if (!m_device || !m_surface) return;
    m_width = w;
    m_height = h;

    WGPUSurfaceCapabilities caps;
    wgpuSurfaceGetCapabilities(m_surface, m_adapter, &caps);

    WGPUSurfaceConfiguration config = {};
    config.device = m_device;
    config.format = caps.formats[0];
    config.usage = WGPUTextureUsage_RenderAttachment;
    config.viewFormatCount = 0;
    config.viewFormats = nullptr;
    config.alphaMode = caps.alphaModes[0];
    config.width = w;
    config.height = h;
    config.presentMode = WGPUPresentMode_Fifo;
    config.usage = WGPUTextureUsage_RenderAttachment;

    wgpuSurfaceConfigure(m_surface, &config);

    wgpuSurfaceCapabilitiesFreeMembers(caps);
}
