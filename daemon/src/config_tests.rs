
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DaemonConfig::default();
        assert_eq!(config.server.mode, "handler");
        assert_eq!(config.raft.node_id, 1);
        assert!(config.exit_nodes.is_empty());
    }

    #[test]
    fn test_merge_scalars() {
        let mut config = DaemonConfig::default();
        let mut other = DaemonConfig::default();
        other.server.mode = "exit".to_string();
        other.raft.node_id = 2; // differs from default

        config.merge(other);

        assert_eq!(config.server.mode, "exit");
        assert_eq!(config.raft.node_id, 2);
    }

    #[test]
    fn test_merge_options() {
        let mut config = DaemonConfig::default();
        let mut other = DaemonConfig::default();
        other.server.location = Some("US-East".to_string());

        config.merge(other);
        assert_eq!(config.server.location, Some("US-East".to_string()));
    }

    #[test]
    fn test_merge_exit_nodes() {
        let mut config = DaemonConfig::default();
        config.exit_nodes.push(ExitNodeConfig {
            name: "node1".to_string(),
            endpoint: "1.1.1.1".to_string(),
            weight: 1.0,
            location: None,
            group_id: 0,
        });

        let mut other = DaemonConfig::default();
        // Update existing
        other.exit_nodes.push(ExitNodeConfig {
            name: "node1".to_string(),
            endpoint: "2.2.2.2".to_string(), // Changed
            weight: 1.0,
            location: None,
            group_id: 0,
        });
        // Add new
        other.exit_nodes.push(ExitNodeConfig {
            name: "node2".to_string(),
            endpoint: "3.3.3.3".to_string(),
            weight: 2.0,
            location: None,
            group_id: 1,
        });

        config.merge(other);

        assert_eq!(config.exit_nodes.len(), 2);
        
        // precise ordering depends on impl, but let's find by name
        let n1 = config.exit_nodes.iter().find(|n| n.name == "node1").unwrap();
        assert_eq!(n1.endpoint, "2.2.2.2");

        let n2 = config.exit_nodes.iter().find(|n| n.name == "node2").unwrap();
        assert_eq!(n2.endpoint, "3.3.3.3");
    }

    #[test]
    fn test_merge_raft_peers() {
        let mut config = DaemonConfig::default();
        config.raft.peers = vec!["peer1".to_string()];

        let mut other = DaemonConfig::default();
        other.raft.peers = vec!["peer2".to_string(), "peer1".to_string()];

        config.merge(other);

        assert_eq!(config.raft.peers.len(), 2);
        assert!(config.raft.peers.contains(&"peer1".to_string()));
        assert!(config.raft.peers.contains(&"peer2".to_string()));
    }
}
