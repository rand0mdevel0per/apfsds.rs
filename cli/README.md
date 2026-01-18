# apfsds-cli

Management CLI for APFSDS.

## Installation

```bash
cargo install apfsds-cli
```

## Usage

```bash
# User management
apfsds-cli user create --username alice --email alice@example.com
apfsds-cli user delete --id 123
apfsds-cli user list

# Node management
apfsds-cli node register --name exit-us-1 --endpoint 203.0.113.1:25347
apfsds-cli node list
apfsds-cli node remove --id 456

# Cluster status
apfsds-cli cluster status
apfsds-cli cluster leader
```

## Configuration

The CLI reads configuration from `~/.apfsds/cli.toml` or via `--config` flag:

```toml
[api]
endpoint = "http://localhost:25348"
api_key = "your-api-key"
```

## Commands

| Command | Description |
|---------|-------------|
| `user create` | Create new user account |
| `user delete` | Delete user account |
| `user list` | List all users |
| `node register` | Register exit node |
| `node remove` | Remove node from cluster |
| `cluster status` | Show cluster health |

## License

MIT
