# apfsds-protocol

Wire protocol definitions and frame serialization for APFSDS.

## Features

- **Zero-copy serialization** using `rkyv`
- **ProxyFrame**: Core data transmission unit with connection ID, flags, and payload
- **ControlMessage**: Enum for control frames (DoH, Ping/Pong, KeyRotation, Emergency)
- **Authentication types**: `AuthRequest`, `AuthResponse`

## Usage

```rust
use apfsds_protocol::{ProxyFrame, FrameFlags, ControlMessage};

// Create a data frame
let frame = ProxyFrame {
    conn_id: 12345,
    flags: FrameFlags::DATA,
    payload: data.into(),
};

// Control messages
let msg = ControlMessage::Ping;
```

## Frame Format

```
┌─────────────┬───────┬─────────────────────────────┐
│ conn_id (8B)│ flags │ payload (variable)          │
└─────────────┴───────┴─────────────────────────────┘
```

## License

MIT
