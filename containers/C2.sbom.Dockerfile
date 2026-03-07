FROM rust:1.93-bookworm

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-deny for license + advisory policy enforcement
RUN cargo install cargo-deny --version "0.16" --locked 2>/dev/null \
    || cargo install cargo-deny --locked

# Install cargo-audit for advisory database checks
RUN cargo install cargo-audit --locked

# Install syft for SBOM generation (CycloneDX + SPDX)
RUN curl -sSfL https://raw.githubusercontent.com/anchore/syft/main/install.sh \
    | sh -s -- -b /usr/local/bin

WORKDIR /workspace
