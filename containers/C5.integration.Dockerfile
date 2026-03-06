FROM rust:1.93-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    xorriso \
    squashfs-tools \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
