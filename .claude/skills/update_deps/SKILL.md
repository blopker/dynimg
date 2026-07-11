---
name: update_deps
description: Update all dependencies in this repo - Rust crates, the blitz git pin, Python/uv deps, and GitHub Actions versions. Use when the user asks to update/bump dependencies.
---

# Update dependencies (dynimg)

Update in this order. After each stage, build/test before moving on so failures are attributable.

## 1. Blitz git pin (the tricky one)

Blitz is pinned by git rev in `Cargo.toml` (5 crates: blitz-dom, blitz-html, blitz-net, blitz-paint, blitz-traits — all must share the SAME rev).

1. Find the newest commit: `git ls-remote https://github.com/DioxusLabs/blitz.git HEAD`
2. **Review the commit log since the last pin** — blitz is a fast-moving project and the user wants a survey of changes that could improve or simplify this codebase, not just a rev bump. Cargo keeps a bare clone you can query directly (no need to clone):
   `git -C ~/.cargo/git/db/blitz-* log --oneline <old-rev>..<new-rev>`
   Look for: new/renamed feature flags (`git ... show <rev>:packages/blitz-dom/Cargo.toml`), `DocumentConfig` changes (`packages/blitz-dom/src/config.rs`), new CSS property support, and anything touching workarounds we carry in `src/lib.rs`. Read the interesting commits with `git ... show <sha>`.
3. Replace the `rev = "..."` in all 5 blitz entries in `Cargo.toml` (use replace_all).
4. Check blitz's own `Cargo.toml` at that rev for the versions it uses of `anyrender`, `anyrender_vello_cpu`, `vello_cpu`, `peniko`, `kurbo` — our pins MUST match blitz's, or you get duplicate-crate type mismatches. Fetch: `https://raw.githubusercontent.com/DioxusLabs/blitz/<rev>/Cargo.toml`
5. Check whether the `[patch.crates-io]` parley pin is still needed: it exists for the emoji VS16 fix (parley PR #637, landed after 0.10.0). Parley 0.11.0 has the fix, but the patch must stay until blitz bumps its `parley = "0.10"` requirement — a patch rev must satisfy the version blitz asks for, so you can't just point the patch at 0.11.
6. `cargo update` then `command make dev` — fix `src/` call sites as needed (the 2026-07 update needed zero code changes).

## 2. Rust crates

- `cargo update` updates within semver ranges (Cargo.lock).
- For major bumps, check latest versions via the crates.io API — it REQUIRES a User-Agent header or returns an error page:
  `curl -s -A 'dynimg-dep-check' https://crates.io/api/v1/crates/<crate>` → `.crate.max_stable_version`
  Key crates to check majors on: pyo3, png, zenjpeg, webp, clap, thiserror.
- pyo3 major bumps often change the `#[pymodule]`/`Bound` APIs — check `src/python.rs`. (0.28→0.29 needed no changes.)
- Keep the `abi3-pyXY` feature in sync with `requires-python` in pyproject.toml (>=3.11 → abi3-py311). Policy from the user (2026-07): keep compatibility with older Python versions unless dropping them buys performance or functionality; the real floor is `requires-python`, so the abi3 tag should match it, not undercut it.

## 3. Python / uv

- `uv lock --upgrade` then `uv sync`.
- Only dev dep is maturin; check pyproject `[build-system]` maturin bound still covers the new version.

## 4. GitHub Actions

- List current: `rg -o 'uses: [^\s]+' .github/workflows/ -N | sort -u`
- Check latest major of each with: `git ls-remote --tags https://github.com/<owner>/<repo> | rg -o 'v[0-9]+[^^{}]*$' | sort -V | tail -3`
- Actions in use: actions/checkout, actions/cache, actions/upload-artifact, actions/download-artifact, actions/setup-python, dtolnay/rust-toolchain@stable (no bump needed), PyO3/maturin-action@v1 (major-pinned, no bump needed), pypa/gh-action-pypi-publish@release/v1, softprops/action-gh-release.
- upload-artifact and download-artifact majors are NOT in lockstep (e.g. upload v7 pairs with download v8) — check each separately.

## 5. Verify

- `command make test` (runs clippy -D warnings, fmt check, cargo test, wheel test, snapshot tests).
- Snapshot policy from the user (2026-07): if updates cause snapshot failures, do NOT auto-update — flag the changed files and let the user evaluate them.
- CI can't be fully verified locally; the Linux vendored-openssl path (`[target.'cfg(target_os = "linux")']`) only builds in CI.

## Notes learned

- **2026-07 (7ecd70b → 4c54f2a):**
  - Blitz added `DocumentConfig.style_threading` and defaulted it to `StyleThreading::Sequential`, which is safe for concurrent documents (blitz issue #430). This made our global `RENDER_LOCK` mutex obsolete — removed it and set `style_threading` explicitly (the upstream field doc-comment stale-claims Parallel is default; don't rely on the default). Regression test: `tests/concurrent_render.rs`.
  - `render()`'s future is NOT `Send` (blitz documents aren't), so concurrency tests must use one current-thread tokio runtime per OS thread, not `tokio::spawn`.
  - Enabled the `floats` blitz-dom feature (CSS float layout; opt-in upstream, actively fixed). `complex-scripts` (dictionary line-breaking for Thai/CJK) exists but was skipped — consider if users report bad CJK/Thai wrapping.
  - Blitz renamed features to kebab-case (`system_fonts` → `system-fonts`); ours (`tracing`, `cache`) were unaffected, but re-check feature spellings on every bump.
  - `actions/upload-artifact` latest was v7 while `download-artifact` was v8 — confirmed not lockstep.
  - `softprops/action-gh-release` v1→v3: `files` and `generate_release_notes` inputs unchanged, just node runtime bumps.
  - Check latest action tags without cloning: `git ls-remote --tags https://github.com/<owner>/<repo>` and sort -V.
  - **Dropped blitz-net entirely** in favor of our own `src/net.rs` (HttpProvider): blitz-net hard-enables reqwest's `native-tls`, which was the ONLY thing pulling OpenSSL into the graph. reqwest 0.13 defaults to rustls (`default-tls = ["rustls"]`, aws-lc-rs crypto), so a direct dep with `default-features = false` gives pure-Rust TLS. This also removed the vendored-openssl Cargo.toml block, the perl-* yum deps, and the sccache workaround in build-wheels.yml. If blitz-net ever gains a rustls feature upstream, we could switch back. Diff vs blitz-net: no disk cache (http-cache was alpha), added a 30s request timeout, kept the 6-per-host cap and Firefox UA.
  - data: URIs previously rendered blank (CombinedProvider routed them to the assets path); now decoded via the `data-url` crate. Snapshot: `data-uri`. Verify graph stays clean after blitz bumps: `cargo tree -i openssl-sys --target x86_64-unknown-linux-gnu` should error with "did not match any packages".
  - aws-lc-sys (rustls crypto) compiles C via `cc` — worked locally; if manylinux CI complains, it may need `cmake` in before-script-linux.
