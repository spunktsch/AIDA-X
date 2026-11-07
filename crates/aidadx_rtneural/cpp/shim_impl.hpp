// Heavy per-model template code (JSON weight loading + forward loop).
//
// `shim.cpp` only *declares* these templates; they are explicitly instantiated
// for the 54 model types in `parts/part*.cpp`, so the expensive Eigen /
// nlohmann::json instantiations are spread over several translation units and
// compiled in parallel (a single TU takes several minutes with MSVC).
#pragma once

#include <cstddef>

#include "model_variant.hpp"

namespace aidadx
{
/// RTNeural `parseJson` + `reset` (may throw; caller catches).
template <typename M>
void load_model(M& model, const nlohmann::json& model_json);

/// out[i] = net(in[i], param1, param2); `in` may alias `out`. Must not allocate.
template <typename M>
void process_model(M& model, const float* in, float* out, size_t n, float param1, float param2) noexcept;
} // namespace aidadx

#ifdef AIDADX_SHIM_DEFINE_TEMPLATES
namespace aidadx
{
template <typename M>
void load_model(M& model, const nlohmann::json& model_json)
{
    model.parseJson(model_json, false);
    model.reset();
}

template <typename M>
void process_model(M& model, const float* in, float* out, size_t n, float param1, float param2) noexcept
{
    if constexpr (M::input_size == 1)
    {
        for (size_t i = 0; i < n; ++i)
            out[i] = model.forward(in + i);
    }
    else
    {
        // Eigen maps multi-input vectors with RTNEURAL_DEFAULT_ALIGNMENT.
        alignas(RTNEURAL_DEFAULT_ALIGNMENT) float x[4] = { 0.0f, param1, param2, 0.0f };
        for (size_t i = 0; i < n; ++i)
        {
            x[0] = in[i];
            out[i] = model.forward(x);
        }
    }
}
} // namespace aidadx

#define AIDADX_INSTANTIATE(M)                                                         \
    template void aidadx::load_model<M>(M&, const nlohmann::json&);                    \
    template void aidadx::process_model<M>(M&, const float*, float*, size_t, float, float) noexcept;
#endif
