// C ABI around RTNeural for the Rust port of AIDA-X.
//
// The model types are NOT copied: this file includes the C++ plugin's own
// `src/model_variant.hpp` (std::variant of the 54 RTNeural::ModelT types +
// NullModel, and `custom_model_creator`), so both implementations always run
// exactly the same compile-time networks. The per-model weight loading and
// forward loops live in `shim_impl.hpp` and are instantiated in `parts/`.
//
// Rules for every exported function:
//   * no C++ exception ever crosses the FFI boundary (all entry points are
//     noexcept and catch everything),
//   * `aidadx_rt_process` / `aidadx_rt_reset` never allocate.

#include <algorithm>
#include <cstddef>
#include <cstdio>
#include <cstring>
#include <exception>
#include <new>
#include <string>
#include <type_traits>
#include <variant>

#include "shim_impl.hpp" // includes model_variant.hpp

namespace
{
struct ShimError
{
    std::string msg;
};

void write_error(char* buf, size_t len, const char* msg) noexcept
{
    if (buf == nullptr || len == 0)
        return;
    const size_t n = std::min(len - 1, std::strlen(msg));
    std::memcpy(buf, msg, n);
    buf[n] = '\0';
}

[[noreturn]] void fail(const std::string& msg)
{
    throw ShimError { msg };
}

const nlohmann::json& require_array(const nlohmann::json& j, size_t size, const std::string& what)
{
    if (! j.is_array())
        fail(what + ": expected an array");
    if (j.size() != size)
        fail(what + ": expected " + std::to_string(size) + " entries, got " + std::to_string(j.size()));
    return j;
}

void require_numbers(const nlohmann::json& j, size_t size, const std::string& what)
{
    require_array(j, size, what);
    for (const auto& v : j)
        if (! v.is_number())
            fail(what + ": non-numeric weight");
}

void require_matrix(const nlohmann::json& j, size_t rows, size_t cols, const std::string& what)
{
    require_array(j, rows, what);
    for (size_t r = 0; r < rows; ++r)
        require_numbers(j[r], cols, what + "[" + std::to_string(r) + "]");
}

int last_dim(const nlohmann::json& shape, const std::string& what)
{
    if (! shape.is_array() || shape.empty() || ! shape.back().is_number_integer())
        fail(what + ": shape must end with an integer");
    return shape.back().get<int>();
}

// RTNeural's loaders trust the JSON (e.g. Dense bias is read through a raw
// pointer, a wrong input size silently leaves the weights at zero), so check
// every shape up front. Mirrors the Rust-side validation in aidadx_core.
void validate(const nlohmann::json& model)
{
    if (! model.is_object())
        fail("model JSON must be an object");
    const int in = last_dim(model.at("in_shape"), "in_shape");
    if (in < 1 || in > MAX_INPUT_SIZE)
        fail("unsupported input size " + std::to_string(in));

    const auto& layers = model.at("layers");
    require_array(layers, 2, "layers (RNN + Dense)");

    const auto& rnn = layers[0];
    const auto type = rnn.at("type").get<std::string>();
    const bool gru = type == "gru";
    if (! gru && type != "lstm")
        fail("first layer must be gru or lstm, got " + type);
    const int h = last_dim(rnn.at("shape"), "rnn shape");
    if (h < 1)
        fail("invalid hidden size");
    const size_t gh = static_cast<size_t>((gru ? 3 : 4) * h);
    const auto& w = rnn.at("weights");
    require_array(w, 3, type + " weights");
    require_matrix(w[0], static_cast<size_t>(in), gh, "kernel");
    require_matrix(w[1], static_cast<size_t>(h), gh, "recurrent kernel");
    if (gru)
        require_matrix(w[2], 2, gh, "gru bias");
    else
        require_numbers(w[2], gh, "lstm bias");

    const auto& dense = layers[1];
    if (dense.at("type").get<std::string>() != "dense" || last_dim(dense.at("shape"), "dense shape") != 1)
        fail("second layer must be dense with 1 output");
    const auto& dw = dense.at("weights");
    require_array(dw, 2, "dense weights");
    require_matrix(dw[0], static_cast<size_t>(h), 1, "dense kernel");
    require_numbers(dw[1], 1, "dense bias");
}
} // namespace

struct AidadxRtModel
{
    ModelVariantType variant;
};

extern "C" {

/// Builds a model from UTF-8 JSON (`json` need not be NUL-terminated).
/// Returns NULL on failure and writes a NUL-terminated message to `err`.
AidadxRtModel* aidadx_rt_create(const char* json, size_t json_len, char* err, size_t err_len) noexcept
{
    if (json == nullptr)
    {
        write_error(err, err_len, "null JSON pointer");
        return nullptr;
    }
    AidadxRtModel* model = nullptr;
    try
    {
        const auto model_json = nlohmann::json::parse(json, json + json_len);
        validate(model_json);

        model = new AidadxRtModel();
        if (! custom_model_creator(model_json, model->variant))
            fail("unable to identify a known model architecture");

        std::visit(
            [&model_json](auto&& m) {
                using M = std::decay_t<decltype(m)>;
                if constexpr (! std::is_same_v<M, NullModel>)
                    aidadx::load_model(m, model_json);
            },
            model->variant);
        return model;
    }
    catch (const ShimError& e)
    {
        write_error(err, err_len, e.msg.c_str());
    }
    catch (const std::exception& e)
    {
        write_error(err, err_len, e.what());
    }
    catch (...)
    {
        write_error(err, err_len, "unknown C++ exception");
    }
    delete model;
    return nullptr;
}

/// Index into `ModelVariantType` (1..=54, 0 = NullModel); order matches
/// `ModelArch::ALL` in aidadx_core.
int aidadx_rt_variant_index(const AidadxRtModel* model) noexcept
{
    return model == nullptr ? -1 : static_cast<int>(model->variant.index());
}

/// Network input size (1 = audio only, 2/3 = audio + params).
int aidadx_rt_input_size(const AidadxRtModel* model) noexcept
{
    if (model == nullptr)
        return 0;
    return std::visit([](const auto& m) { return static_cast<int>(std::decay_t<decltype(m)>::input_size); },
                      model->variant);
}

/// Clears the recurrent state. Real-time safe.
void aidadx_rt_reset(AidadxRtModel* model) noexcept
{
    if (model == nullptr)
        return;
    try
    {
        std::visit(
            [](auto&& m) {
                if constexpr (! std::is_same_v<std::decay_t<decltype(m)>, NullModel>)
                    m.reset();
            },
            model->variant);
    }
    catch (...)
    {
    }
}

/// Raw network forward for `n` samples: out[i] = net(in[i], param1, param2).
/// `in` and `out` may be the same buffer. param1/param2 are only used by 2-/3-input
/// models. No gains, no skip. Real-time safe (no allocation, no locks).
void aidadx_rt_process(AidadxRtModel* model, const float* in, float* out, size_t n, float param1, float param2) noexcept
{
    if (model == nullptr || in == nullptr || out == nullptr)
        return;
    try
    {
        std::visit(
            [=](auto&& m) {
                using M = std::decay_t<decltype(m)>;
                if constexpr (std::is_same_v<M, NullModel>)
                    std::fill(out, out + n, 0.0f);
                else
                    aidadx::process_model(m, in, out, n, param1, param2);
            },
            model->variant);
    }
    catch (...)
    {
        std::fill(out, out + n, 0.0f);
    }
}

void aidadx_rt_destroy(AidadxRtModel* model) noexcept
{
    delete model;
}

} // extern "C"
