.PHONY: *

# Install Python package into local venv
install:
	maturin develop --release

# Install in development mode (faster builds, debug symbols)
dev:
	maturin develop

# Run tests
test: lint install
	cargo test
	.venv/bin/python -m pytest
	./scripts/snapshot-tests.sh

# Regenerates the shared baselines and the macOS system-fonts baseline here,
# then the Linux system-fonts baseline in a CI-matching container (needs Docker)
update_snapshot:
	./scripts/snapshot-tests.sh --update
	./scripts/linux-snapshots.sh --update

# Run lints
lint:
	cargo clippy -- -D warnings
	cargo fmt -- --check

# Format code
fmt:
	cargo fmt

# Clean build artifacts
clean:
	cargo clean
	rm -rf dist/
	rm -rf *.egg-info/

# Build release wheels
build:
	maturin build --release

test_render:
	./scripts/render-examples.sh

# Bump the version, tag and push: make release [BUMP=patch|minor|major]
release:
	./scripts/release.sh $(BUMP)
