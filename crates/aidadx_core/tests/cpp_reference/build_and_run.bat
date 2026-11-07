@echo off
rem Regenerate tests/fixtures/biquad_golden.{bin,idx} from the original C++ code (TICKET-022).
rem Needs MSVC (cl.exe). Run from any directory:
rem     crates\aidadx_core\tests\cpp_reference\build_and_run.bat
rem DPF headers: %DPF_DIR% if set, else <repo>\modules\dpf (git submodule update --init modules/dpf).
setlocal
set HERE=%~dp0
set REPO=%HERE%..\..\..\..
if "%DPF_DIR%"=="" set DPF_DIR=%REPO%\modules\dpf
if not exist "%DPF_DIR%\distrho\extra\ValueSmoother.hpp" (
    echo DPF not found at %DPF_DIR% - run "git submodule update --init modules/dpf" or set DPF_DIR
    exit /b 1
)

where cl >nul 2>nul
if errorlevel 1 (
    set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
    call "%%VCVARS%%" x64 >nul || exit /b 1
)

set BUILD=%TEMP%\aidadx_biquad_ref
if not exist "%BUILD%" mkdir "%BUILD%"

rem /fp:precise (MSVC default) and no /arch:AVX2: no FMA contraction, plain SSE2 double math,
rem matching how the plugin is built and how rustc compiles the Rust port.
cl /nologo /std:c++17 /O2 /fp:precise /EHsc /W3 /D_CRT_SECURE_NO_WARNINGS ^
   /I"%REPO%\src" /I"%DPF_DIR%\distrho" ^
   "%HERE%gen_biquad_ref.cpp" /Fo"%BUILD%\\" /Fe"%BUILD%\gen_biquad_ref.exe" || exit /b 1

"%BUILD%\gen_biquad_ref.exe" "%HERE%..\fixtures" || exit /b 1
