#!/bin/bash
# Multi-app, multi-platform build script
#
# Usage:
#   ./build.sh <platform> <app>
#   ./build.sh <command>
#
# Examples:
#   ./build.sh linux rustcam
#   ./build.sh linux hello
#   ./build.sh nuttx rustcam
#   ./build.sh nuttx hello
#   ./build.sh setup
#   ./build.sh config

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NUTTX_DIR="$SCRIPT_DIR/external/nuttx"
APPS_DIR="$SCRIPT_DIR/external/nuttx-apps"

# Board configuration
BOARD="esp32s3-devkit"
CONFIG="nsh"

usage() {
    echo "Usage: ./build.sh <platform> <app>"
    echo "       ./build.sh <command>"
    echo ""
    echo "Build:"
    echo "  linux <app>  - Build app for Linux"
    echo "  nuttx <app>  - Build app for NuttX (configures and builds firmware)"
    echo ""
    echo "Apps: $(ls -1 apps/ 2>/dev/null | tr '\n' ' ')"
    echo ""
    echo "Commands:"
    echo "  setup       - Initialize project (symlinks, submodules)"
    echo "  config      - Configure NuttX for ESP32-S3"
    echo "  menuconfig  - Run menuconfig"
    echo "  clean       - Clean build artifacts"
}

setup() {
    echo "==> Setting up project..."

    # Create symlink for NuttX platform in nuttx-apps
    APP_LINK="$APPS_DIR/examples/rustapp"
    if [ ! -L "$APP_LINK" ]; then
        echo "Creating symlink: $APP_LINK -> $SCRIPT_DIR/platform/nuttx"
        ln -sf "$SCRIPT_DIR/platform/nuttx" "$APP_LINK"
    else
        echo "Symlink already exists: $APP_LINK"
    fi

    # Ensure submodules are initialized
    cd "$SCRIPT_DIR"
    git submodule update --init --recursive

    echo "==> Setup complete"
}

config() {
    local app_name="${1:-rustcam}"
    echo "==> Configuring NuttX for ESP32-S3 with app: $app_name..."

    cd "$NUTTX_DIR"

    # Clean any existing config
    make distclean 2>/dev/null || true

    # Configure for ESP32-S3 devkit
    ./tools/configure.sh -l "$BOARD:$CONFIG"

    # Enable required options for Rust std
    kconfig-tweak --enable CONFIG_SYSTEM_TIME64
    kconfig-tweak --enable CONFIG_FS_LARGEFILE
    kconfig-tweak --set-val CONFIG_TLS_NELEM 8
    kconfig-tweak --set-val CONFIG_TLS_NCLEANUP 4
    kconfig-tweak --enable CONFIG_DEV_URANDOM

    # Enable the Rust app with specified package name
    kconfig-tweak --enable CONFIG_EXAMPLES_RUSTAPP
    kconfig-tweak --set-str CONFIG_EXAMPLES_RUSTAPP_NAME "$app_name"
    kconfig-tweak --set-str CONFIG_EXAMPLES_RUSTAPP_PROGNAME "$app_name"

    # Refresh config
    make olddefconfig

    echo "==> Configuration complete for $app_name"
}

build_linux() {
    local app="$1"
    echo "==> Building $app for Linux..."
    cargo build -p "$app"
    echo ""
    echo "Output: target/debug/$app"
}

build_nuttx() {
    local app="$1"
    echo "==> Building $app for NuttX..."

    # Use cargo-nuttx which handles everything
    cargo nuttx "$app"
}

clean() {
    echo "==> Cleaning build artifacts..."
    cargo clean
    make -C "$NUTTX_DIR" clean 2>/dev/null || true
    echo "==> Clean complete"
}

# Main
case "${1:-}" in
    linux)
        if [ -z "$2" ]; then
            echo "Error: specify app name"
            echo "Usage: ./build.sh linux <app>"
            exit 1
        fi
        build_linux "$2"
        ;;
    nuttx)
        if [ -z "$2" ]; then
            echo "Error: specify app name"
            echo "Usage: ./build.sh nuttx <app>"
            exit 1
        fi
        build_nuttx "$2"
        ;;
    setup)
        setup
        ;;
    config)
        config "${2:-rustcam}"
        ;;
    menuconfig)
        cd "$NUTTX_DIR"
        make menuconfig
        ;;
    clean)
        clean
        ;;
    *)
        usage
        exit 1
        ;;
esac
