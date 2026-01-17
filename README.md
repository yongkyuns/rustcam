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

- ESP-IDF toolchain with Xtensa support
- Rust esp toolchain (`rustup toolchain install esp`)
- esptool (`pip install esptool`)

## Build

```bash
source ~/export-esp.sh
./build.sh all
```

## Flash

```bash
esptool.py -p /dev/ttyACM0 -c esp32s3 write_flash 0x0 external/nuttx/nuttx.bin
```

## Run

```bash
picocom -b 115200 /dev/ttyACM0
```

From NSH:
```
nsh> rustcam
```

Commands: `s` spawn, `t` terminate, `m` memory stats, `q` quit
