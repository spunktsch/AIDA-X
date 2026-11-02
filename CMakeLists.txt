cmake_minimum_required(VERSION 3.15)
project(AIDA-X VERSION 1.1.0)

set(CMAKE_POLICY_DEFAULT_CMP0063 NEW)
set(CMAKE_POLICY_DEFAULT_CMP0069 NEW)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

set(CMAKE_C_VISIBILITY_PRESET hidden)
set(CMAKE_CXX_VISIBILITY_PRESET hidden)
set(CMAKE_VISIBILITY_INLINES_HIDDEN TRUE)

set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -DDISTRHO_NAMESPACE=AidaDISTRHO -DDGL_NAMESPACE=AidaDGL")

# if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(amd64|AMD64|i.86|x64|X64|x86|x86_64|X86)$")
#   set(PREFER_RTNEURAL_XSIMD TRUE)
# else()
set(PREFER_RTNEURAL_XSIMD FALSE)
# endif()
set(RTNEURAL_XSIMD ${PREFER_RTNEURAL_XSIMD} CACHE BOOL "Use RTNeural with this backend")
message("RTNEURAL_XSIMD in ${CMAKE_PROJECT_NAME} = ${RTNEURAL_XSIMD}, using processor type ${CMAKE_SYSTEM_PROCESSOR} and system name ${CMAKE_SYSTEM_NAME}")

add_subdirectory(modules/dpf)
add_subdirectory(modules/rtneural)

find_package(Threads REQUIRED)
set_property(GLOBAL PROPERTY INTERPROCEDURAL_OPTIMIZATION TRUE)

dpf_add_plugin(AIDA-X
  TARGETS au clap lv2 vst2 vst3
  #NO_SHARED_RESOURCES
  UI_TYPE opengl
  USE_FILE_BROWSER
  FILES_DSP
    Files.cpp
    modules/FFTConvolver/AudioFFT.cpp
    modules/FFTConvolver/FFTConvolver.cpp
    modules/FFTConvolver/TwoStageFFTConvolver.cpp
    modules/FFTConvolver/Utilities.cpp
    modules/r8brain/pffft.cpp
    modules/r8brain/r8bbase.cpp
    src/aidadsp-plugin.cpp
    src/Biquad.cpp
    src/3rd-party.cpp
  FILES_UI
    Graphics.cpp
    src/aidadsp-ui.cpp)

dpf_add_plugin(AIDA-X-Standalone
  TARGETS jack
  #NO_SHARED_RESOURCES
  UI_TYPE opengl
  USE_FILE_BROWSER
  FILES_DSP
    Files.cpp
    modules/FFTConvolver/AudioFFT.cpp
    modules/FFTConvolver/FFTConvolver.cpp
    modules/FFTConvolver/TwoStageFFTConvolver.cpp
    modules/FFTConvolver/Utilities.cpp
    modules/r8brain/pffft.cpp
    modules/r8brain/r8bbase.cpp
    src/aidadsp-plugin.cpp
    src/Biquad.cpp
    src/3rd-party.cpp
  FILES_UI
    Graphics.cpp
    modules/dpf-widgets/opengl/Blendish.cpp
    src/aidadsp-ui.cpp)

target_compile_definitions(dgl-opengl PUBLIC
  DGL_NAMESPACE=AidaDGL
  DGL_WINDOWS_ICON_ID=401
  NVG_FONT_TEXTURE_FLAGS=NVG_IMAGE_NEAREST
)

target_include_directories(AIDA-X PUBLIC
  src
  src/plugin
  modules/dr_libs
  modules/FFTConvolver
  modules/r8brain
  modules/rtneural
  ${CMAKE_BINARY_DIR}
)

target_include_directories(AIDA-X-Standalone PUBLIC
  src
  src/standalone
  modules/dpf-widgets/opengl
  modules/dr_libs
  modules/FFTConvolver
  modules/r8brain
  modules/rtneural
  ${CMAKE_BINARY_DIR}
)

# needed for enabling SSE in pffft
if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(i.86|x86|X86)$")
  target_compile_definitions(AIDA-X PUBLIC i386)
  target_compile_definitions(AIDA-X-Standalone PUBLIC i386)
endif()

# needed for emscripten
if(EMSCRIPTEN)
  target_compile_definitions(RTNeural PUBLIC EIGEN_DONT_VECTORIZE=1)
endif()

# needed for RISC-V
if(CMAKE_SYSTEM_PROCESSOR MATCHES "riscv64")
  set(CMAKE_OPTIONAL_LIBATOMIC "-latomic")
endif()

target_link_libraries(AIDA-X PUBLIC RTNeural ${CMAKE_THREAD_LIBS_INIT} ${CMAKE_OPTIONAL_LIBATOMIC})
target_link_libraries(AIDA-X-Standalone PUBLIC RTNeural ${CMAKE_THREAD_LIBS_INIT} ${CMAKE_OPTIONAL_LIBATOMIC})

# convert data into code
add_custom_command(
  PRE_BUILD
  COMMAND ${CMAKE_SOURCE_DIR}/modules/dpf/utils/res2c.py Files ${CMAKE_SOURCE_DIR}/files ${CMAKE_BINARY_DIR}
  MAIN_DEPENDENCY ${CMAKE_SOURCE_DIR}/files
  OUTPUT Files.cpp
  WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)

add_custom_command(
  PRE_BUILD
  COMMAND ${CMAKE_SOURCE_DIR}/modules/dpf/utils/res2c.py Graphics ${CMAKE_SOURCE_DIR}/graphics/png ${CMAKE_BINARY_DIR}
  MAIN_DEPENDENCY ${CMAKE_SOURCE_DIR}/graphics
  OUTPUT Graphics.cpp
  WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)

# set up extra standalone details
set_target_properties(AIDA-X-Standalone-jack PROPERTIES OUTPUT_NAME "AIDA-X")

if(APPLE)
set_target_properties(AIDA-X-Standalone-jack PROPERTIES
  MACOSX_BUNDLE TRUE
  MACOSX_BUNDLE_INFO_PLIST "${CMAKE_SOURCE_DIR}/utils/Info.plist"
)
add_custom_command(
  POST_BUILD
  COMMAND cmake -E copy "${CMAKE_SOURCE_DIR}/utils/ax.icns" "${CMAKE_BINARY_DIR}/bin/AIDA-X.app/Contents/Resources/ax.icns"
  DEPENDS AIDA-X-Standalone-jack
  OUTPUT "${CMAKE_BINARY_DIR}/bin/AIDA-X.app/Contents/Resources/ax.icns"
)
add_custom_target(AIDA-X-App ALL
  DEPENDS "${CMAKE_BINARY_DIR}/bin/AIDA-X.app/Contents/Resources/ax.icns"
)
elseif(WIN32)
target_sources(AIDA-X-Standalone-jack PUBLIC utils/ax.rc)
endif()

# ============================================================================
# WGPU+egui Standalone Backend (TICKET-006)
# ============================================================================

option(WITH_WGPU_EGUI "Build WGPU+egui standalone backend" OFF)

if(WITH_WGPU_EGUI)
    find_package(Threads REQUIRED)

    # Find wgpu-native library
    set(WGPU_INCLUDE_DIR "${CMAKE_SOURCE_DIR}/modules/wgpu-native/ffi" CACHE PATH "wgpu-native include directory")
    set(WGPU_HEADERS_DIR "${CMAKE_SOURCE_DIR}/modules/wgpu-native/ffi/webgpu-headers" CACHE PATH "webgpu headers directory")

    # Find GLFW
    find_package(glfw3 QUIET)
    if(NOT glfw3_FOUND)
        # Try to find GLFW via pkg-config
        find_package(PkgConfig QUIET)
        if(PKG_CONFIG_FOUND)
            pkg_check_modules(GLFW REQUIRED glfw3)
        endif()
    endif()

    add_executable(AIDA-X-WGPU
        src/wgpu-egui/main.cpp
        src/wgpu-egui/WgpuBackend.cpp
        src/wgpu-egui/Gui.cpp
    )

    target_include_directories(AIDA-X-WGPU PRIVATE
        src
        src/wgpu-egui
        src/plugin
        modules/rtneural
        modules/wgpu-native/ffi
        modules/wgpu-native/ffi/webgpu-headers
        modules/FFTConvolver
        modules/r8brain
        modules/dpf/distrho
        ${CMAKE_BINARY_DIR}
    )

    if(WIN32)
        target_link_libraries(AIDA-X-WGPU PRIVATE
            ${WGPU_LIBRARY}
            ${GLFW_LIBRARY}
            d3dcompiler
            ws2_32
            userenv
            bcrypt
            ntdll
            opengl32
            Propsys
            RuntimeObject
        )
        target_compile_definitions(AIDA-X-WGPU PRIVATE
            _WIN32_WINNT=0x0A00
            WIN32_LEAN_AND_MEAN
        )
    elseif(APPLE)
        target_link_libraries(AIDA-X-WGPU PRIVATE
            ${WGPU_LIBRARY}
            ${GLFW_LIBRARY}
            "-framework Cocoa"
            "-framework IOKit"
            "-framework CoreFoundation"
            "-framework QuartzCore"
            "-framework Metal"
        )
    else()
        target_link_libraries(AIDA-X-WGPU PRIVATE
            ${WGPU_LIBRARY}
            ${GLFW_LIBRARY}
            "-lm"
            "-ldl"
            "-lpthread"
        )
        if(UNIX)
            target_link_libraries(AIDA-X-WGPU PRIVATE X11)
        endif()
    endif()

    target_compile_features(AIDA-X-WGPU PRIVATE cxx_std_17)

    # Link wgpu-native library
    if(WGPU_LIBRARY)
        target_link_libraries(AIDA-X-WGPU PRIVATE ${WGPU_LIBRARY})
    endif()

    set_target_properties(AIDA-X-WGPU PROPERTIES
        OUTPUT_NAME "AIDA-X-WGPU"
        RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
    )

    message(STATUS "WGPU+egui standalone backend enabled")
    message(STATUS "  Binary: AIDA-X-WGPU")
    message(STATUS "  WGPU include: ${WGPU_INCLUDE_DIR}")
endif()

# set up windows installer
if(WIN32 AND MINGW)
add_custom_command(
  POST_BUILD
  COMMAND utils/windows-installer.sh "${CMAKE_BINARY_DIR}" "${CMAKE_PROJECT_VERSION}"
  DEPENDS AIDA-X-clap AIDA-X-lv2 AIDA-X-lv2-ui AIDA-X-vst2 AIDA-X-vst3 AIDA-X-Standalone-jack
  OUTPUT "${CMAKE_SOURCE_DIR}/utils/inno/AIDA-X-${CMAKE_PROJECT_VERSION}-win64-installer.exe"
  WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
)
add_custom_target(AIDA-X-Installer ALL
  DEPENDS "${CMAKE_SOURCE_DIR}/utils/inno/AIDA-X-${CMAKE_PROJECT_VERSION}-win64-installer.exe"
)
endif()
