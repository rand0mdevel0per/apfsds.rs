# Build stage
FROM rust:1.85-bookworm AS builder

WORKDIR /app

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

# Create dummy src files for dependency caching
RUN mkdir -p crates/protocol/src crates/crypto/src crates/obfuscation/src \
    crates/transport/src crates/storage/src crates/raft/src \
    daemon/src client/src cli/src && \
    echo "fn main() {}" > daemon/src/main.rs && \
    echo "fn main() {}" > client/src/main.rs && \
    echo "fn main() {}" > cli/src/main.rs && \
    echo "pub fn dummy() {}" > crates/protocol/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/crypto/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/obfuscation/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/transport/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/storage/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/raft/src/lib.rs

# Build dependencies only (cached layer)
RUN cargo build --release --bin apfsdsd || true

# Copy actual source code
COPY crates/ crates/
COPY daemon/ daemon/
COPY client/ client/
COPY cli/ cli/

# Touch to invalidate cache on source change
RUN touch daemon/src/main.rs client/src/main.rs cli/src/main.rs

# Build release binaries
RUN cargo build --release --bin apfsdsd --bin apfsds --bin apfsds-cli

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false apfsds

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/apfsdsd /usr/local/bin/
COPY --from=builder /app/target/release/apfsds /usr/local/bin/
COPY --from=builder /app/target/release/apfsds-cli /usr/local/bin/

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
