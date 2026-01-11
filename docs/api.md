# APFSDS API Specifications

## Management API
**Base URL**: `http://localhost:25348`

### Cluster
- **POST** `/admin/cluster/membership`
    - Change Raft cluster membership (add/remove nodes).
    - Body: `{ "members": [1, 2, 3] }`

### Users
- **POST** `/admin/users`
    - Create a new user.
    - Body: `{ "username": "alice", "quota_bytes": 1000000 }`
- **DELETE** `/admin/users/:id`
    - Delete a user.

### Nodes
- **POST** `/admin/nodes`
    - Register a new exit node.
    - Body: `{ "name": "exit-01", "endpoint": "1.2.3.4:8080", "weight": 1.0 }`

### Monitoring
- **GET** `/admin/stats`
    - Get system statistics (active connections, throughput).
- **GET** `/`
    - Web Dashboard (HTML).

## Client Control Protocol (WebSocket)

The client communicates with the daemon via a secure WebSocket upgrade using a custom binary protocol.

### Handshake
1.  **Client** connects to `/v1/connect`.
2.  **Daemon** sends `200 OK` (Upgrade to WebSocket).
3.  **Client** sends `AuthRequest` (Encrypted with Server Public Key).
4.  **Daemon** verifies token and responds with `AuthResponse`.

### Frame Types
- **Data (0x00)**: Encapsulated TCP/UDP payload.
- **Control (0x01)**:
    - `DohQuery` / `DohResponse`: DNS Traffic.
    - `Ping` / `Pong`: Keepalive.
    - `KeyRotation`: Server announcing new public key.
    - `Emergency`: Server announcing threat level.
