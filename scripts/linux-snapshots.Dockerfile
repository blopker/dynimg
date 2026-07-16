# Approximates the GitHub ubuntu-latest runner for snapshot purposes: same
# Ubuntu release, same font packages from the same apt archive (fontconfig,
# DejaVu, Liberation for the Arial alias, Noto Color Emoji). Used by
# scripts/linux-snapshots.sh to run/update Linux snapshots locally instead of
# bootstrapping baselines from CI artifacts.
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl build-essential python3 cmake bc \
    fontconfig fonts-dejavu-core fonts-liberation fonts-noto-color-emoji \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /work
