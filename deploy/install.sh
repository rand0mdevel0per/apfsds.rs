#!/bin/bash
set -e

COLOR_GREEN='\033[0;32m'
COLOR_NC='\033[0m'

log() {
    echo -e "${COLOR_GREEN}[APFSDS]${COLOR_NC} $1"
}

log "Starting APFSDS Installer..."

# Check prerequisites
if ! command -v cargo &> /dev/null; then
    log "Rust not found. Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Build
log "Building release binaries..."
cargo build --release --workspace

# Install (mock)
log "Binaries built successfully at target/release/"
log " - apfsdsd (Daemon)"
log " - apfsds (Client)"

# Deployment suggestion
log "To install globally:"
log "  sudo cp target/release/apfsdsd /usr/local/bin/"
log "  sudo cp target/release/apfsds /usr/local/bin/"

log "Installation complete."
