############################################################################
# Generic Rust application Makefile for NuttX
############################################################################

include $(APPDIR)/Make.defs

# Application info from Kconfig
PROGNAME  = $(CONFIG_EXAMPLES_RUSTAPP_PROGNAME)
PRIORITY  = $(CONFIG_EXAMPLES_RUSTAPP_PRIORITY)
STACKSIZE = $(CONFIG_EXAMPLES_RUSTAPP_STACKSIZE)
MODULE    = $(CONFIG_EXAMPLES_RUSTAPP)

# Rust package name from Kconfig
RUST_PACKAGE = $(CONFIG_EXAMPLES_RUSTAPP_NAME)

# C source files (BLE wrapper for NimBLE integration)
CSRCS = ble_wrapper.c

# Include paths for NimBLE headers
ifeq ($(CONFIG_NIMBLE),y)
NIMBLE_DIR = $(APPDIR)/wireless/bluetooth/nimble/mynewt-nimble
CFLAGS += -I$(NIMBLE_DIR)/nimble/include
CFLAGS += -I$(NIMBLE_DIR)/nimble/transport/include
CFLAGS += -I$(NIMBLE_DIR)/nimble/host/include
CFLAGS += -I$(NIMBLE_DIR)/nimble/host/services/gap/include
CFLAGS += -I$(NIMBLE_DIR)/nimble/host/services/gatt/include
CFLAGS += -I$(NIMBLE_DIR)/nimble/host/util/include
CFLAGS += -I$(NIMBLE_DIR)/porting/nimble/include
CFLAGS += -I$(NIMBLE_DIR)/porting/npl/nuttx/include
endif

# Paths relative to this Makefile location (platform/nuttx/)
WORKSPACE_ROOT = $(CURDIR)/../..
RUST_TARGET_SPEC = $(CURDIR)/nuttx.json

# Path to the compiled Rust library
RUST_LIB_PATH = $(WORKSPACE_ROOT)/target/nuttx/release/librustcam.a

# Link the Rust library
IMPORTLIBS += $(RUST_LIB_PATH)

context::
	@echo "Building Rust package '$(RUST_PACKAGE)' for NuttX..."
	NUTTX_INCLUDE_DIR=$(TOPDIR)/include:$(TOPDIR)/include/arch \
	cargo build --lib --release -p $(RUST_PACKAGE) \
		--no-default-features --features platform-nuttx \
		-Zbuild-std=std,panic_abort \
		-Zbuild-std-features=panic_immediate_abort \
		--manifest-path $(WORKSPACE_ROOT)/Cargo.toml \
		--target $(RUST_TARGET_SPEC)

clean::
	cargo clean --manifest-path $(WORKSPACE_ROOT)/Cargo.toml -p $(RUST_PACKAGE)

include $(APPDIR)/Application.mk
