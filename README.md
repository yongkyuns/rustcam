# rustcam

Experimenting with Rust std using NuttX on ESP32-S3.

## Overview

This project explores running Rust with full std library support on the ESP32-S3 using NuttX RTOS. It includes a memory profiler and interactive thread demo to understand how std features behave on embedded targets.

## Features

- Rust std library on bare-metal ESP32-S3 via NuttX
- Memory profiling for std types (Vec, String, HashMap, Arc, etc.)
- Thread spawning and management demo
- Custom Xtensa target spec for NuttX

## Prerequisites

- Git submodules checked out
- Rust Xtensa toolchain via `espup`
- esptool (`pip install esptool`)
- Python 3 (the build script installs `kconfiglib` automatically if needed)

Toolchain setup:

```bash
git submodule update --init --recursive
cargo install espup --locked
```

On Linux and Apple Silicon:

```bash
espup install -f ~/export-esp.sh -t esp32s3
```

On Intel macOS, use the last Xtensa toolchain release that still ships
`x86_64-apple-darwin` assets:

```bash
espup install -f ~/export-esp.sh -t esp32s3 -v 1.90.0.0
```

## Build

```bash
source ~/export-esp.sh
./cargo-nuttx hello
```

`cargo-nuttx` now defaults to the `ESP32-S3-WROOM-1-N8R8` module profile and
enables PSRAM as part of the common NuttX heap. If your board uses a different
ESP32-S3 module, override it per build:

```bash
NUTTX_ESP32S3_MODULE=wroom1n4 ./cargo-nuttx hello
```

Supported module values:
`wroom1n4`, `wroom1n8r2`, `wroom1n8r8`, `wroom1n16r8`, `wroom1n16r16v`,
`wroom2n16r8v`, `wroom2n32r8v`, `mini1n8`.

Local NuttX fixes are kept as patch files under `patches/nuttx/` and applied
automatically by `cargo-nuttx` before each build, so the `external/nuttx`
submodule can stay close to upstream.

## Flash

```bash
esptool.py --chip esp32s3 --port /dev/cu.usbmodem143301 write_flash 0x0 external/nuttx/nuttx.bin
```

## Run

```bash
python3 -m serial.tools.miniterm --raw /dev/cu.usbmodem143301 115200
```

`cargo-nuttx` now enables the ESP32-S3 USB-Serial/JTAG console by default, so
the board is usable over USB-OTG without an external UART adapter.

From NSH:
```
nsh> hello
```

Expected output:

```text
Hello from Rust!
Current heap usage: <bytes> bytes
```
