# apfsds-transport

Network transport implementations for APFSDS.

## Features

- **WebSocket (WSS)**: Primary transport with TLS support
- **QUIC**: High-performance UDP transport for handler↔exit communication
- **SSH**: Fallback tunnel transport

## Usage

### WebSocket Server

```rust
use apfsds_transport::wss::WssServer;

let server = WssServer::bind("0.0.0.0:25347").await?;
while let Some(conn) = server.accept().await {
    tokio::spawn(handle_connection(conn));
}
```

### WebSocket Client

```rust
use apfsds_transport::wss::WssClient;

let client = WssClient::connect("wss://handler.example.com:25347/v1/connect").await?;
client.send(frame).await?;
```

### QUIC Transport

```rust
use apfsds_transport::quic::{QuicClient, QuicServer};

// Client
let client = QuicClient::connect("handler.example.com:25347").await?;

// Server
let server = QuicServer::bind("0.0.0.0:25347", cert, key).await?;
```

## License

MIT
