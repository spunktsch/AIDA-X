# WGPU+egui Backend CMake Configuration
# Part of AIDA-X (TICKET-006)

# This file defines the wgpu-egui backend for AIDA-X standalone.
# It is included from the main CMakeLists.txt when WITH_WGPU_EGUI is ON.

set(WGPU_EGUI_SOURCES
    src/wgpu-egui/main.cpp
    src/wgpu-egui/WgpuBackend.cpp
    src/wgpu-egui/Gui.cpp
)

set(WGPU_EGUI_HEADERS
    src/wgpu-egui/WgpuApp.hpp
    src/wgpu-egui/WgpuBackend.hpp
    src/wgpu-egui/Gui.hpp
    src/wgpu-egui/Widgets.hpp
)

find_package(Threads REQUIRED)

# Try to find wgpu-native
if(WIN32)
    # On Windows, look for pre-built wgpu-native.dll or wgpu.dll
    find_library(WGPU_LIBRARY NAMES wgpu wgpu-native wgpu.dll PATHS
        ${CMAKE_SOURCE_DIR}/modules/wgpu-native/build/Release
        ${CMAKE_SOURCE_DIR}/modules/wgpu-native/build
        PATHS_SUFFIXES bin lib
    )
elseif(APPLE)
    find_library(WGPU_LIBRARY NAMES wgpu wgpu-native PATHS
        ${CMAKE_SOURCE_DIR}/modules/wgpu-native/build
        PATHS_SUFFIXES lib
    )
else()
    find_library(WGPU_LIBRARY NAMES wgpu wgpu-native)
endif()

if(NOT WGPU_LIBRARY)
    message(WARNING "wgpu-native library not found. Set WGPU_LIBRARY path manually.")
endif()

# GLFW detection
find_package(glfw3 QUIET)
if(NOT glfw3_FOUND)
    find_package(PkgConfig QUIET)
    if(PKG_CONFIG_FOUND)
        pkg_check_modules(GLFW QUIET glfw3)
    endif()
endif()

if(NOT glfw3_FOUND AND NOT GLFW_FOUND)
    message(WARNING "GLFW not found. Install GLFW3 or set GLFW library path.")
endif()
