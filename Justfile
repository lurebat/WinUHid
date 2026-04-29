# Justfile — central command runner for the WinUHid repo.
#
# Install just from https://github.com/casey/just
# (or `winget install Casey.Just`).
# Run `just` (no args) to see all recipes.
#
# These recipes are thin wrappers; if you don't have just installed,
# read this file for the equivalent raw command.
#
# Native recipes auto-locate Visual Studio's `msbuild` and
# `vstest.console.exe`, so they work from a regular PowerShell session.
# The web recipes still expect `cargo` to be on PATH.

set windows-shell := ["powershell", "-NoLogo", "-NoProfile", "-Command"]

# --- Defaults ------------------------------------------------------------

# MSBuild configuration: Debug or Release.
config   := env_var_or_default("WINUHID_CONFIG", "Release")

# MSBuild platform: x86, x64, or ARM64.
platform := env_var_or_default("WINUHID_PLATFORM", "x64")

# Address the web app should bind to.
web_addr := env_var_or_default("WINUHID_WEB_ADDR", "127.0.0.1:7878")

build_dir := join(justfile_directory(), "build", config, platform)
web_dir   := join(justfile_directory(), "web")
vs_tool   := join(justfile_directory(), "scripts", "invoke-vs-tool.ps1")

# --- Help ----------------------------------------------------------------

# List all recipes.
default:
    @just --list

# Print where the web UI will look for the WinUHid DLLs.
where-dlls:
    @echo "Configured build dir: {{ build_dir }}"
    @echo "Override with WINUHID_DLL_DIR or `--dll-dir <path>` on the web app CLI."

# --- Native build / test -------------------------------------------------

# Restore NuGet + vcpkg packages used by the native solution.
restore:
    & "{{ vs_tool }}" msbuild -t:restore -p:RestorePackagesConfig=true

# Build the whole solution (override config/platform: `just cfg=Debug plat=ARM64 build`).
build cfg=config plat=platform:
    & "{{ vs_tool }}" msbuild WinUHid.sln /p:Configuration={{ cfg }} /p:Platform={{ plat }} /m /verbosity:minimal

# Clean the entire solution and the `build/` directory.
clean cfg=config plat=platform:
    & "{{ vs_tool }}" msbuild WinUHid.sln /t:Clean /p:Configuration={{ cfg }} /p:Platform={{ plat }} /m /verbosity:minimal
    -Remove-Item -Recurse -Force "{{ justfile_directory() }}/build" -ErrorAction SilentlyContinue

# Clean + build.
rebuild cfg=config plat=platform:
    @just clean {{ cfg }} {{ plat }}
    @just build {{ cfg }} {{ plat }}

# Build the user-mode SDK only (WinUHid.dll).
build-sdk cfg=config plat=platform:
    & "{{ vs_tool }}" msbuild "WinUHid/WinUHid.vcxproj" /p:Configuration={{ cfg }} /p:Platform={{ plat }} /m /verbosity:minimal

# Build the kernel/UMDF driver only.
build-driver cfg=config plat=platform:
    & "{{ vs_tool }}" msbuild "WinUHid Driver/WinUHid Driver.vcxproj" /p:Configuration={{ cfg }} /p:Platform={{ plat }} /m /verbosity:minimal

# Build the preset device helpers (WinUHidDevs.dll).
build-devs cfg=config plat=platform:
    & "{{ vs_tool }}" msbuild "WinUHidDevs/WinUHidDevs.vcxproj" /p:Configuration={{ cfg }} /p:Platform={{ plat }} /m /verbosity:minimal

# Build the unit-test EXE.
build-tests cfg=config plat=platform:
    & "{{ vs_tool }}" msbuild "WinUHidUnitTests/WinUHidUnitTests.vcxproj" /p:Configuration={{ cfg }} /p:Platform={{ plat }} /m /verbosity:minimal

# Build the MSI installer (WiX).
package cfg=config plat=platform:
    & "{{ vs_tool }}" msbuild "Installer/WinUHid Package/WinUHid Package.wixproj" /p:Configuration={{ cfg }} /p:Platform={{ plat }} /m /verbosity:minimal

# Build + run the unit tests.
test cfg=config plat=platform:
    @just build-tests {{ cfg }} {{ plat }}
    & "{{ vs_tool }}" vstest "{{ justfile_directory() }}/build/{{ cfg }}/{{ plat }}/WinUHidUnitTests.dll" /Platform:{{ plat }}

# --- Web app -------------------------------------------------------------

# Build the web UI (debug).
web-build:
    cargo build --manifest-path "{{ web_dir }}/Cargo.toml"

# Build the web UI in release mode.
web-build-release:
    cargo build --manifest-path "{{ web_dir }}/Cargo.toml" --release

# Run the web UI (uses {{ build_dir }} as the default DLL directory).
web-run *args:
    $env:WINUHID_DLL_DIR = "{{ build_dir }}"; cargo run --manifest-path "{{ web_dir }}/Cargo.toml" -- --addr {{ web_addr }} {{ args }}

# Run the web UI (release).
web-run-release *args:
    $env:WINUHID_DLL_DIR = "{{ build_dir }}"; cargo run --manifest-path "{{ web_dir }}/Cargo.toml" --release -- --addr {{ web_addr }} {{ args }}

# cargo fmt the web crate.
web-fmt:
    cargo fmt --manifest-path "{{ web_dir }}/Cargo.toml" --all

# cargo fmt --check the web crate.
web-fmt-check:
    cargo fmt --manifest-path "{{ web_dir }}/Cargo.toml" --all -- --check

# cargo check + clippy with warnings as errors.
web-check:
    cargo check --manifest-path "{{ web_dir }}/Cargo.toml" --all-targets
    cargo clippy --manifest-path "{{ web_dir }}/Cargo.toml" --all-targets -- -D warnings

# cargo test for the web crate.
web-test:
    cargo test --manifest-path "{{ web_dir }}/Cargo.toml"

# Wipe the web crate's `target/` directory.
web-clean:
    cargo clean --manifest-path "{{ web_dir }}/Cargo.toml"

# --- Driver install ------------------------------------------------------

# Install the test cert and register the driver. Requires elevation
# and prior `just build-driver` and `just package`. The script will
# refuse to do anything destructive without an explicit `INSTALL`
# confirmation.
install-driver cfg=config plat=platform:
    powershell -NoProfile -ExecutionPolicy Bypass -File "{{ justfile_directory() }}/scripts/install-driver.ps1" -Configuration {{ cfg }} -Platform {{ plat }}

# Remove every WinUHidDriver registered with PnP. Requires elevation.
uninstall-driver:
    powershell -NoProfile -ExecutionPolicy Bypass -File "{{ justfile_directory() }}/scripts/uninstall-driver.ps1"

# --- Catch-alls ----------------------------------------------------------

# Format everything that has a formatter.
fmt: web-fmt

# Run the per-PR sanity checks.
check:
    @just build Release x64
    @just build Debug   x64
    @just web-fmt-check
    @just web-check
