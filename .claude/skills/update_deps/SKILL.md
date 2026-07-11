---
name: update_deps
description: Update all dependencies in this repo - Rust crates, the blitz git pin, Python/uv deps, and GitHub Actions versions. Use when the user asks to update/bump dependencies.
---

# Update dependencies (dynimg)

Update in this order. After each stage, build/test before moving on so failures are attributable.

## 1. Blitz git pin

Blitz is pinned by git rev in `Cargo.toml` (blitz-dom, blitz-html, blitz-paint, blitz-traits — all must share the SAME rev).

1. Find the newest commit: `git ls-remote https://github.com/DioxusLabs/blitz.git HEAD`
2. **Review the commit log since the last pin** — the user wants a survey of changes that could improve or simplify this codebase, not just a rev bump. Use cargo's bare clone:
   `git -C ~/.cargo/git/db/blitz-* log --oneline <old-rev>..<new-rev>`
   Look for: feature flag changes in `packages/blitz-dom/Cargo.toml`, `DocumentConfig` changes, new CSS support, and anything touching workarounds in `src/lib.rs`/`src/net.rs`.
3. Replace the rev in all blitz entries (replace_all), then `cargo update` + `command make dev`.
4. Our pins for `anyrender*`, `vello_cpu`, `peniko`, `kurbo` MUST match the versions in blitz's Cargo.toml at that rev, or you get duplicate-crate type mismatches.
5. The `[patch.crates-io]` parley pin and the direct `fontique` git dep must stay on the SAME rev as each other, and the patch must satisfy blitz's parley version requirement (see comments in Cargo.toml for why each exists). Verify:
   `cargo tree -f '{p} {f}' -i yeslogic-fontconfig-sys --target x86_64-unknown-linux-gnu` → single fontique, features include `dlopen`.
   `cargo tree -i openssl-sys --target x86_64-unknown-linux-gnu` → must error (no OpenSSL in graph).

## 2. Rust crates

- `cargo update` for semver-compatible; for majors, check crates.io API (REQUIRES a User-Agent header):
  `curl -s -A 'dynimg-dep-check' https://crates.io/api/v1/crates/<crate>` → `.crate.max_stable_version`
- Keep the pyo3 `abi3-pyXY` feature matching `requires-python` in pyproject.toml. User policy: don't drop older Python versions unless it buys performance or functionality.

## 3. Python / uv

- `uv lock --upgrade` then `uv sync`. Check the `[build-system]` maturin bound still covers the new version.

## 4. GitHub Actions

- List current: `rg -o 'uses: [^\s]+' .github/workflows/ -N | sort -u`
- Latest tags without cloning: `git ls-remote --tags https://github.com/<owner>/<repo>` + sort -V.
- upload-artifact and download-artifact majors are NOT in lockstep — check each separately.

## 5. Verify

- `command make test` (clippy -D warnings, fmt, cargo test, wheel test, snapshot tests).
- Snapshot failures may be legitimate rendering changes: do NOT auto-update — flag the changed files for the user to evaluate.
- To verify the Linux build locally (user has Docker):
  `docker run --rm -v "$PWD":/work -v "$HOME/.cargo/registry":/usr/local/cargo/registry -w /work rust:slim sh -c 'apt-get update -qq && apt-get install -y -qq python3 cmake >/dev/null; cargo build --release --target-dir /work/scratch/target-linux'`
  (python3 is for stylo's build script). Run/`ldd` the result in `debian:stable-slim` — only libgcc/libm/libc should be needed.
- Do not commit without the user reviewing the changes first.
