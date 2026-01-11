# APFSDS User Guide

## Installation

### Prerequisites
- Windows, Linux, or macOS.
- Rust Toolchain (for building from source).

### Building
```bash
# Build Daemon and Client
cargo build --release
```

## Running the Daemon

1.  **Configuration**: Edit `config/daemon.toml`.
    ```toml
    [server]
    bind = "0.0.0.0:25347"
    
    [raft]
    node_id = 1
    peers = []
    
    [storage]
    disk_path = "./data"
    ```

2.  **Run**:
    ```bash
    ./target/release/apfsdsd --config config/daemon.toml
    ```

## Running the Client

1.  **Configuration**: Edit `config/client.toml`.
    ```toml
    [client]
    mode = "socks5"
    
    [client.socks5]
    bind = "127.0.0.1:1080"
    
    [connection]
    endpoints = ["wss://your-daemon-ip:25347/v1/connect"]
    ```

2.  **Run**:
    ```bash
    ./target/release/apfsds --config config/client.toml
    ```

3.  **Tunneling**: Configure your browser or application to use SOCKS5 proxy at `127.0.0.1:1080`.

## Web Dashboard
Access the control plane dashboard at `http://localhost:25348`.

## Mobile Support
For mobile integration, build the FFI library:
```bash
cargo build -p apfsds-client --lib --release
```
Use the generated `.dll` / `.so` / `.dylib` to link into your Android/iOS project.
Entry point: `apfsds_mobile_start(config_path_cstr)`.
