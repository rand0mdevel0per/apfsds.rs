# APFSDS Deployment Scenarios

## Document Information

**Version:** 1.0
**Date:** 2026-01-25
**Purpose:** Guide for different deployment scenarios and best practices

---

## Overview

This document provides detailed deployment scenarios for APFSDS, covering different scales, architectures, and use cases.

---

## 1. Single-Machine Deployment (Personal/Small Team)

### Use Case
- Personal use (1-10 users)
- Small team testing
- Development environment
- Budget-constrained deployments

### Architecture
```
┌─────────────────────────────┐
│   Single VPS/Server         │
│  ┌──────────────────────┐   │
│  │  K3s (lightweight)   │   │
│  │  ┌────────────────┐  │   │
│  │  │ APFSDS Pod     │  │   │
│  │  │ (all-in-one)   │  │   │
│  │  └────────────────┘  │   │
│  └──────────────────────┘   │
└─────────────────────────────┘
```

### Specifications
- **CPU:** 2 cores minimum
- **RAM:** 4GB minimum
- **Disk:** 20GB SSD
- **Network:** 100Mbps+
- **Cost:** $10-20/month

### Deployment Steps
1. Run the one-click install script
2. Configure domain and secrets
3. Deploy with default settings
4. Test connectivity

---

## 2. Multi-Node Cluster Deployment (High Availability)

### Use Case
- Production environments
- Medium to large teams (50-500 users)
- High availability requirements
- Geographic redundancy

### Architecture
```
┌─────────────────────────────────────────────────┐
│   K8s/K3s Cluster (3+ nodes)                    │
│  ┌──────────────┐  ┌──────────────┐  ┌────────┐│
│  │ Handler-1    │  │ Handler-2    │  │Handler3││
│  │ (Raft Leader)│  │ (Follower)   │  │(Follow)││
│  │ ┌──────────┐ │  │ ┌──────────┐ │  │┌──────┐││
│  │ │ APFSDS   │ │  │ │ APFSDS   │ │  ││APFSDS│││
│  │ │ Pod      │ │  │ │ Pod      │ │  ││Pod   │││
│  │ └──────────┘ │  │ └──────────┘ │  │└──────┘││
│  └──────────────┘  └──────────────┘  └────────┘│
│         ↓                  ↓              ↓     │
│  ┌─────────────────────────────────────────┐   │
│  │   Shared State (Raft Consensus)         │   │
│  └─────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

### Specifications
- **Nodes:** 3 minimum (odd number for Raft)
- **CPU per node:** 4 cores
- **RAM per node:** 8GB
- **Disk:** 50GB SSD per node
- **Network:** 1Gbps+ with low latency between nodes
- **Cost:** $100-200/month

### Deployment Steps

1. **Prepare Infrastructure**
   ```bash
   # Provision 3 VPS instances with static IPs
   # Example IPs: 10.0.1.1, 10.0.1.2, 10.0.1.3
   ```

2. **Install K3s Cluster**
   ```bash
   # On node 1 (master):
   curl -sfL https://get.k3s.io | sh -s - server \
     --cluster-init \
     --disable=traefik

   # Get token:
   sudo cat /var/lib/rancher/k3s/server/node-token

   # On nodes 2 and 3:
   curl -sfL https://get.k3s.io | K3S_URL=https://10.0.1.1:6443 \
     K3S_TOKEN=<token> sh -s - server
   ```

3. **Configure APFSDS Values**
   ```yaml
   # values-ha.yaml
   replicaCount: 3

   raft:
     nodeIds: [1, 2, 3]
     peers:
       - "apfsds-0.apfsds-headless:25347"
       - "apfsds-1.apfsds-headless:25347"
       - "apfsds-2.apfsds-headless:25347"
   ```

4. **Deploy with Helm**
   ```bash
   helm upgrade --install apfsds ./helm-chart \
     --namespace apfsds \
     --create-namespace \
     --values values-ha.yaml \
     --wait
   ```

5. **Verify Cluster Health**
   ```bash
   # Check pods are running on different nodes
   kubectl get pods -n apfsds -o wide

   # Check Raft leader election
   kubectl logs -n apfsds apfsds-0 | grep "became leader"

   # Test failover
   kubectl delete pod -n apfsds apfsds-0
   kubectl get pods -n apfsds -w
   ```

### Best Practices
- Use anti-affinity rules to spread pods across nodes
- Configure persistent volumes for Raft WAL
- Set up monitoring with Prometheus
- Enable automatic backups to ClickHouse
- Use LoadBalancer or Ingress for client access
- Test failover scenarios regularly
- Monitor Raft consensus health metrics

---

## 3. Split Handler/Exit Deployment (Production Scale)

### Use Case
- Large-scale deployments (500+ users)
- Geographic distribution
- Traffic optimization
- Regulatory compliance (data residency)

### Architecture
```
┌─────────────────────────────────────────────────┐
│   Handler Cluster (Region A)                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │Handler-1 │  │Handler-2 │  │Handler-3 │      │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘      │
└───────┼─────────────┼─────────────┼─────────────┘
        │             │             │
        └─────────────┼─────────────┘
                      │ PlainPacket forwarding
        ┌─────────────┼─────────────┐
        │             │             │
┌───────▼─────────────▼─────────────▼─────────────┐
│   Exit Nodes (Multiple Regions)                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │Exit-US   │  │Exit-EU   │  │Exit-Asia │      │
│  │(weight:1)│  │(weight:1)│  │(weight:2)│      │
│  └──────────┘  └──────────┘  └──────────┘      │
└─────────────────────────────────────────────────┘
```

### Specifications

**Handler Cluster:**
- **Nodes:** 3-5 nodes
- **CPU per node:** 8 cores
- **RAM per node:** 16GB
- **Disk:** 100GB SSD per node
- **Network:** 10Gbps backbone

**Exit Nodes:**
- **Nodes:** 2-3 per region
- **CPU per node:** 4 cores
- **RAM per node:** 8GB
- **Disk:** 50GB SSD per node
- **Network:** 10Gbps+ with low latency to internet

**Total Cost:** $500-1000/month

### Deployment Steps

1. **Deploy Handler Cluster**
   ```bash
   # Create handler configuration
   cat > values-handler.yaml <<EOF
   deployment:
     mode: handler

   replicaCount: 3

   raft:
     nodeIds: [1, 2, 3]
     peers:
       - "apfsds-0.apfsds-headless:25347"
       - "apfsds-1.apfsds-headless:25347"
       - "apfsds-2.apfsds-headless:25347"

   exitNodes:
     - name: "exit-us-1"
       endpoint: "203.0.113.10:25347"
       weight: 1.0
       location: "US-East"
     - name: "exit-eu-1"
       endpoint: "198.51.100.20:25347"
       weight: 1.0
       location: "EU-Frankfurt"
     - name: "exit-asia-1"
       endpoint: "192.0.2.30:25347"
       weight: 2.0
       location: "Asia-Tokyo"
   EOF
   ```

2. **Deploy Exit Nodes**
   ```bash
   # On each exit node VPS
   cat > values-exit.yaml <<EOF
   deployment:
     mode: exit

   replicaCount: 1

   server:
     bind: "0.0.0.0:25347"
     location: "US-East"  # Change per region

   handler:
     upstream: "wss://handler.example.com:25347"
   EOF

   # Deploy on exit node (can use Docker or K3s)
   helm upgrade --install apfsds-exit ./helm-chart \
     --namespace apfsds-exit \
     --create-namespace \
     --values values-exit.yaml
   ```

3. **Verify Split Architecture**
   ```bash
   # Check handler cluster health
   kubectl get pods -n apfsds-handler
   kubectl logs -n apfsds-handler apfsds-handler-0 | grep "exit node"

   # Check exit node connectivity
   kubectl logs -n apfsds-exit apfsds-exit-0 | grep "connected to handler"
   ```

### Best Practices
- Use separate networks for handler-exit communication
- Implement health checks for exit nodes
- Configure weighted load balancing based on capacity
- Set up geo-routing for optimal latency
- Monitor PlainPacket forwarding metrics
- Use VPN or private network between handler and exit nodes
- Implement automatic failover for exit nodes
- Configure rate limiting per exit node

---

## 4. Troubleshooting Common Issues

### Issue 1: Raft Leader Election Fails

**Symptoms:**
- Pods keep restarting
- Logs show "election timeout"
- No leader elected after 30 seconds

**Solutions:**
```bash
# Check network connectivity between nodes
kubectl exec -n apfsds apfsds-0 -- ping apfsds-1.apfsds-headless

# Verify Raft configuration
kubectl logs -n apfsds apfsds-0 | grep "raft"

# Check if node IDs are unique
kubectl get pods -n apfsds -o yaml | grep "node_id"
```

### Issue 2: Exit Node Connection Timeout

**Symptoms:**
- Handler logs show "exit node unreachable"
- Client connections fail intermittently
- High latency on specific routes

**Solutions:**
```bash
# Test exit node connectivity from handler
kubectl exec -n apfsds-handler apfsds-handler-0 -- \
  curl -v http://203.0.113.10:25347/health

# Check exit node firewall rules
# Ensure port 25347 is open for handler IPs

# Verify exit node is running
kubectl get pods -n apfsds-exit

# Check exit node logs
kubectl logs -n apfsds-exit apfsds-exit-0 | tail -100
```

### Issue 3: High Memory Usage

**Symptoms:**
- Pods OOMKilled
- Memory usage grows over time
- Performance degradation

**Solutions:**
```bash
# Check current memory usage
kubectl top pods -n apfsds

# Increase memory limits
# Edit values.yaml:
resources:
  limits:
    memory: 8Gi  # Increase from 4Gi
```

---

## 5. Performance Tuning Recommendations

### Network Optimization

**TCP Tuning:**
```bash
# On handler and exit nodes
sysctl -w net.core.rmem_max=16777216
sysctl -w net.core.wmem_max=16777216
sysctl -w net.ipv4.tcp_rmem="4096 87380 16777216"
sysctl -w net.ipv4.tcp_wmem="4096 65536 16777216"
```

**Connection Pooling:**
```yaml
# values.yaml
connection:
  poolSize: 1000
  keepaliveInterval: 30s
  maxIdleTime: 300s
```

### Storage Optimization

**tmpfs Configuration:**
```yaml
storage:
  tmpfs:
    enabled: true
    size: 1Gi  # Increase for high-traffic scenarios
    path: /dev/shm/apfsds
```

**ClickHouse Tuning:**
```yaml
storage:
  clickhouse:
    enabled: true
    flushInterval: 1000  # Reduce for faster writes
    batchSize: 10000     # Increase for better throughput
```

### Resource Limits

**CPU Optimization:**
```yaml
resources:
  limits:
    cpu: 8000m      # 8 cores for high-traffic handlers
    memory: 16Gi
  requests:
    cpu: 4000m      # Reserve 4 cores minimum
    memory: 8Gi
```

**Horizontal Pod Autoscaling:**
```yaml
autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 10
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80
```

### Monitoring and Metrics

**Prometheus Configuration:**
```yaml
monitoring:
  prometheus:
    enabled: true
    scrapeInterval: 15s
  metrics:
    - connection_count
    - throughput_bytes
    - latency_ms
    - raft_leader_changes
```

---

## 6. Summary

This document covered five deployment scenarios for APFSDS:

1. **Single-Machine Deployment** - Ideal for personal use and small teams (1-10 users)
   - Cost: $10-20/month
   - Setup: Simple one-click installation
   - Use case: Testing, development, budget-constrained

2. **Multi-Node Cluster** - Production-ready high availability (50-500 users)
   - Cost: $100-200/month
   - Setup: 3-node Raft cluster with K3s
   - Use case: Production environments requiring HA

3. **Split Handler/Exit** - Large-scale geographic distribution (500+ users)
   - Cost: $500-1000/month
   - Setup: Separate handler cluster and exit nodes
   - Use case: Enterprise deployments with geo-routing

4. **Troubleshooting** - Common issues and solutions
   - Raft leader election failures
   - Exit node connectivity problems
   - Memory management

5. **Performance Tuning** - Optimization recommendations
   - Network and TCP tuning
   - Storage optimization (tmpfs, ClickHouse)
   - Resource limits and autoscaling
   - Monitoring with Prometheus

### Next Steps

- Review [Configuration Reference](configuration.md) for detailed settings
- Check [Testing Plan](testing-plan.md) for validation procedures
- See [Comparison Document](comparison.md) for alternatives analysis

