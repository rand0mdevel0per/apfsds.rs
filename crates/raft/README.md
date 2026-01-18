# apfsds-raft

Raft consensus implementation for APFSDS distributed cluster.

## Features

- **async-raft Integration**: Built on the `async-raft` crate
- **Persistent Storage**: WAL-backed log with HardState persistence
- **ClickHouse Backup**: Async export of committed entries
- **Dynamic Membership**: Add/remove nodes at runtime

## Usage

```rust
use apfsds_raft::{RaftNode, RaftConfig};

let config = RaftConfig {
    node_id: 1,
    peers: vec!["192.168.1.2:25347", "192.168.1.3:25347"],
    data_dir: "/var/lib/apfsds/raft".into(),
};

let node = RaftNode::new(config).await?;

// Propose a command (leader only)
node.propose(command).await?;

// Query cluster state
let leader = node.current_leader().await;
```

## Cluster Setup

```toml
[raft]
node_id = 1
peers = ["192.168.1.2:25347", "192.168.1.3:25347"]
data_dir = "/var/lib/apfsds/raft"
```

## License

MIT
