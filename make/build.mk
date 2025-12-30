# =============================================================================
# BUILD OPERATIONS - make/build.mk
# =============================================================================
# Compilation with feature flag support

.PHONY: build

# =============================================================================
# PUBLIC TARGETS
# =============================================================================

build: _check-rust  ## Build project (use FEATURES="--features redis" or "--all-features")
	@echo "Building cache-kit..."
	@$(CARGO) build $(FEATURES)
	@echo "✓ Build complete"
