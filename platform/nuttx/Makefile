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

# Paths relative to this Makefile location (platform/nuttx/)
WORKSPACE_ROOT = $(CURDIR)/../..
RUST_TARGET_SPEC = $(CURDIR)/nuttx.json

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
