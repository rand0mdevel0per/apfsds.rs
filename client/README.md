# apfsds-client

Client application for APFSDS.

## Features

- **SOCKS5 Proxy**: Local SOCKS5 server for application tunneling
- **TUN Mode**: System-wide VPN via TUN interface
- **Local DNS**: DNS-over-WSS for leak-free resolution
- **Emergency Mode**: Automatic shutdown on kill-switch signal
- **Multi-handler Support**: Automatic failover between handlers

## Installation

```bash
cargo install apfsds-client
```

## Usage

```bash
# Run with config file
apfsds --config client.toml

# SOCKS5 mode (default)
# Configure applications to use 127.0.0.1:1080
```

## Configuration

```toml
[client]
mode = "socks5"

[client.socks5]
bind = "127.0.0.1:1080"

[client.dns]
enabled = true
bind = "127.0.0.1:5353"

[connection]
endpoints = ["wss://handler.example.com:25347/v1/connect"]
```

## Modes

| Mode | Description |
|------|-------------|
| `socks5` | Local SOCKS5 proxy (default) |
| `tun` | System-wide TUN interface |

## License

MIT
