############################################################################
# rustcam/app/Makefile
#
# Licensed to the Apache Software Foundation (ASF) under one or more
# contributor license agreements.  See the NOTICE file distributed with
# this work for additional information regarding copyright ownership.  The
# ASF licenses this file to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance with the
# License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
# WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
# License for the specific language governing permissions and limitations
# under the License.
#
############################################################################

include $(APPDIR)/Make.defs

# Rust Camera built-in application info

PROGNAME  = $(CONFIG_EXAMPLES_RUSTCAM_PROGNAME)
PRIORITY  = $(CONFIG_EXAMPLES_RUSTCAM_PRIORITY)
STACKSIZE = $(CONFIG_EXAMPLES_RUSTCAM_STACKSIZE)
MODULE    = $(CONFIG_EXAMPLES_RUSTCAM)

# Custom target spec for ESP32-S3 on NuttX
RUST_TARGET_SPEC = $(APPDIR)/examples/rustcam/nuttx.json
RUST_TARGET_NAME = nuttx

context::
	@echo "Building Rust code for ESP32-S3 NuttX..."
	NUTTX_INCLUDE_DIR=$(TOPDIR)/include:$(TOPDIR)/include/arch \
	cargo build --lib --release --no-default-features --features platform-nuttx \
		-Zbuild-std=std,panic_abort \
		-Zbuild-std-features=panic_immediate_abort \
		--manifest-path $(APPDIR)/examples/rustcam/Cargo.toml \
		--target $(RUST_TARGET_SPEC)

clean::
	cargo clean --manifest-path $(APPDIR)/examples/rustcam/Cargo.toml

include $(APPDIR)/Application.mk
