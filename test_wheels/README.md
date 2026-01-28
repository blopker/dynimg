# Testing Wheels from CI

This folder contains tools for testing wheels downloaded from CI artifacts.

## Quick Start

```bash
# 1. Download wheel from GitHub Actions artifacts
#    Go to: https://github.com/blopker/dynimg/actions
#    Click on a workflow run → Artifacts → Download the wheel for your platform

# 2. Create a fresh venv
python3 -m venv .venv
source .venv/bin/activate

# 3. Install the wheel
pip install path/to/dynimg-*.whl

# 4. Run tests
python test_dynimg.py
```

## Platform Wheels

Download the appropriate wheel for your system:

| Platform | Artifact Name |
|----------|---------------|
| Linux x86_64 | `wheel-linux-x86_64` |
| Linux ARM64 | `wheel-linux-aarch64` |
| macOS x86_64 | `wheel-macos-x86_64` |
| macOS ARM64 | `wheel-macos-aarch64` |

## Using gh CLI

```bash
# List recent workflow runs
gh run list --workflow=ci.yml

# Download artifacts from a specific run
gh run download <RUN_ID> -n wheel-linux-x86_64

# Or download all wheels
gh run download <RUN_ID>
```

## One-liner Test

```bash
# Create venv, install wheel, run tests
python3 -m venv .venv && source .venv/bin/activate && pip install *.whl && python test_dynimg.py
```
