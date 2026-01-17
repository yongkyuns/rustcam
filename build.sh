#!/bin/bash
# ESP32-S3 Camera Project Build Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NUTTX_DIR="$SCRIPT_DIR/external/nuttx"
APPS_DIR="$SCRIPT_DIR/external/nuttx-apps"
APP_NAME="rustcam"

# Board configuration
BOARD="esp32s3-devkit"
CONFIG="nsh"

usage() {
    echo "Usage: $0 <command>"
    echo ""
    echo "Commands:"
    echo "  setup     - Link app into nuttx-apps and initialize"
    echo "  config    - Configure NuttX for ESP32-S3"
    echo "  menuconfig- Run menuconfig for manual configuration"
    echo "  build     - Build the firmware"
    echo "  clean     - Clean build artifacts"
    echo "  distclean - Full clean including configuration"
    echo "  all       - Setup, config, and build"
    echo ""
}

setup() {
    echo "==> Setting up project..."

    # Create symlink for the app in nuttx-apps
    APP_LINK="$APPS_DIR/examples/$APP_NAME"
    if [ ! -L "$APP_LINK" ]; then
        echo "Creating symlink: $APP_LINK -> $SCRIPT_DIR/app"
        ln -sf "$SCRIPT_DIR/app" "$APP_LINK"
    else
        echo "Symlink already exists: $APP_LINK"
    fi

    # Ensure submodules are initialized
    cd "$SCRIPT_DIR"
    git submodule update --init --recursive

    echo "==> Setup complete"
}

config() {
    echo "==> Configuring NuttX for ESP32-S3..."

    cd "$NUTTX_DIR"

    # Clean any existing config
    make distclean 2>/dev/null || true

    # Configure for ESP32-S3 devkit
    # -l creates symlink to apps, -a specifies apps directory path (relative to nuttx dir)
    ./tools/configure.sh -l "$BOARD:$CONFIG"

    # Enable required options for Rust std
    kconfig-tweak --enable CONFIG_SYSTEM_TIME64
    kconfig-tweak --enable CONFIG_FS_LARGEFILE
    kconfig-tweak --set-val CONFIG_TLS_NELEM 8
    kconfig-tweak --set-val CONFIG_TLS_NCLEANUP 4
    kconfig-tweak --enable CONFIG_DEV_URANDOM

    # Enable the rustcam app
    kconfig-tweak --enable CONFIG_EXAMPLES_RUSTCAM

    # Refresh config
    make olddefconfig

    echo "==> Configuration complete"
    echo "Run '$0 menuconfig' to customize further"
}

menuconfig() {
    cd "$NUTTX_DIR"
    make menuconfig
}

build() {
    echo "==> Building firmware..."

    # Source ESP environment if available
    if [ -f ~/export-esp.sh ]; then
        source ~/export-esp.sh
    fi

    cd "$NUTTX_DIR"
    make -j$(nproc)

    echo "==> Build complete"
    echo "Firmware: $NUTTX_DIR/nuttx.bin"
}

clean() {
    echo "==> Cleaning build artifacts..."
    cd "$NUTTX_DIR"
    make clean

    # Clean Rust target directory
    if [ -d "$SCRIPT_DIR/app/target" ]; then
        rm -rf "$SCRIPT_DIR/app/target"
    fi

    echo "==> Clean complete"
}

distclean() {
    echo "==> Full clean..."
    cd "$NUTTX_DIR"
    make distclean 2>/dev/null || true

    # Clean Rust target directory
    if [ -d "$SCRIPT_DIR/app/target" ]; then
        rm -rf "$SCRIPT_DIR/app/target"
    fi

    echo "==> Distclean complete"
}

all() {
    setup
    config
    build
}

# Main
case "${1:-}" in
    setup)
        setup
        ;;
    config)
        config
        ;;
    menuconfig)
        menuconfig
        ;;
    build)
        build
        ;;
    clean)
        clean
        ;;
    distclean)
        distclean
        ;;
    all)
        all
        ;;
    *)
        usage
        exit 1
        ;;
esac
