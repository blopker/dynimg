.PHONY: install dev test lint fmt clean build

# Install Python package into local venv
install:
	maturin develop --release

# Install in development mode (faster builds, debug symbols)
dev:
	maturin develop

# Run tests
test: install
	cargo test
	python test_wheels/test_dynimg.py

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
	rm -rf target/
	rm -rf *.egg-info/

# Build release wheels
build:
	maturin build --release

test_render:
	./scripts/render-examples.sh

release:
	./scripts/release.sh
