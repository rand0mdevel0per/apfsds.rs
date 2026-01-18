# apfsds-storage

MVCC storage engine with WAL persistence for APFSDS.

## Features

- **MVCC Engine**: Multi-version concurrency control for consistent reads
- **Log-structured Segments**: Append-only segments with efficient compaction
- **Write-Ahead Log (WAL)**: Durability with crash recovery
- **B-link Tree Index**: Fast key lookups
- **ClickHouse Backup**: Async batch export to ClickHouse for analytics

## Usage

```rust
use apfsds_storage::{StorageEngine, StorageConfig};

let config = StorageConfig {
    disk_path: "/var/lib/apfsds".into(),
    segment_size: 64 * 1024 * 1024, // 64MB
    compaction_threshold: 4,
};

let engine = StorageEngine::open(config)?;

// Write
engine.put(b"key", b"value").await?;

// Read
let value = engine.get(b"key").await?;

// Scan range
for (k, v) in engine.scan(b"a"..b"z").await? {
    println!("{:?} = {:?}", k, v);
}
```

## Architecture

```
┌─────────────┐
│   API       │
├─────────────┤
│   MVCC      │ ← Version management
├─────────────┤
│  Segments   │ ← Log-structured storage
├─────────────┤
│    WAL      │ ← Write-ahead log
└─────────────┘
```

## License

MIT
