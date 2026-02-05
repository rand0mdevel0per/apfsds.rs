# Build stage
FROM rust:latest AS builder

# Get target architecture for multi-arch builds
ARG TARGETARCH

WORKDIR /app

# Install mold linker for faster compilation
RUN apt-get update && apt-get install -y --no-install-recommends \
    mold \
    clang \
    && rm -rf /var/lib/apt/lists/*

# Configure Rust to use mold linker
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold -C target-cpu=native"
ENV CARGO_BUILD_JOBS=8

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/protocol/Cargo.toml crates/protocol/
COPY crates/crypto/Cargo.toml crates/crypto/
COPY crates/obfuscation/Cargo.toml crates/obfuscation/
COPY crates/transport/Cargo.toml crates/transport/
COPY crates/storage/Cargo.toml crates/storage/
COPY crates/raft/Cargo.toml crates/raft/
COPY daemon/Cargo.toml daemon/
COPY client/Cargo.toml client/
COPY cli/Cargo.toml cli/
COPY tests/Cargo.toml tests/

# Create dummy src files for dependency caching
RUN mkdir -p crates/protocol/src crates/crypto/src crates/obfuscation/src \
    crates/transport/src crates/storage/src crates/raft/src \
    daemon/src client/src cli/src tests/src && \
    echo "fn main() {}" > daemon/src/main.rs && \
    echo "fn main() {}" > client/src/main.rs && \
    echo "fn main() {}" > cli/src/main.rs && \
    echo "pub fn dummy() {}" > crates/protocol/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/crypto/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/obfuscation/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/transport/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/storage/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/raft/src/lib.rs && \
    echo "pub fn dummy() {}" > tests/src/lib.rs

# Build dependencies only (cached layer)
# Use fewer codegen units for dependencies to speed up linking
RUN CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
    cargo build --release --bin apfsdsd || true

# Copy actual source code
COPY crates/ crates/
COPY daemon/ daemon/
COPY client/ client/
COPY cli/ cli/
COPY tests/ tests/

# Touch all source files to invalidate incremental cache
RUN touch daemon/src/main.rs client/src/main.rs cli/src/main.rs && \
    touch crates/protocol/src/lib.rs crates/crypto/src/lib.rs crates/obfuscation/src/lib.rs && \
    touch crates/transport/src/lib.rs crates/storage/src/lib.rs crates/raft/src/lib.rs

# Build release binary (daemon only for deployment)
# Optimize for faster compilation with parallel codegen
RUN CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
    cargo build --release --bin apfsdsd

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false apfsds

WORKDIR /app

# Copy daemon binary from builder
COPY --from=builder /app/target/release/apfsdsd /usr/local/bin/

# Create data directory
RUN mkdir -p /var/lib/apfsds && chown apfsds:apfsds /var/lib/apfsds

USER apfsds

# Default to daemon
ENTRYPOINT ["/usr/local/bin/apfsdsd"]
CMD ["--config", "/etc/apfsds/daemon.toml"]

EXPOSE 25347 25348 9090

LABEL org.opencontainers.image.source="https://github.com/rand0mdevel0per/apfsds.rs"
LABEL org.opencontainers.image.description="APFSDS - Privacy-preserving Forwarding System"
LABEL org.opencontainers.image.licenses="MIT"
