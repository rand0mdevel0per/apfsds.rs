#!/bin/bash
# Network Partition Simulation Script
#
# Simulates network partitions using iptables for testing
# Raft leader election and consensus behavior.
#
# Usage:
#   ./partition_test.sh isolate <node_ip>   - Isolate a node from others
#   ./partition_test.sh heal                 - Restore connectivity
#   ./partition_test.sh status               - Show current rules

set -e

NODE1_IP="${NODE1_IP:-192.168.1.101}"
NODE2_IP="${NODE2_IP:-192.168.1.102}"
NODE3_IP="${NODE3_IP:-192.168.1.103}"
ALL_NODES="${NODE1_IP} ${NODE2_IP} ${NODE3_IP}"

case "$1" in
    isolate)
        TARGET="$2"
        if [ -z "$TARGET" ]; then
            echo "Usage: $0 isolate <node_ip>"
            exit 1
        fi

        echo "Isolating node ${TARGET}..."
        for node in $ALL_NODES; do
            if [ "$node" != "$TARGET" ]; then
                # Block traffic from this node to target
                ssh root@${node} "iptables -A INPUT -s ${TARGET} -j DROP"
                ssh root@${node} "iptables -A OUTPUT -d ${TARGET} -j DROP"
                echo "  Blocked ${node} <-> ${TARGET}"
            fi
        done
        echo "Node ${TARGET} is now isolated."
        ;;

    heal)
        echo "Healing network partition..."
        for node in $ALL_NODES; do
            ssh root@${node} "iptables -F" 2>/dev/null || true
            echo "  Flushed rules on ${node}"
        done
        echo "Network connectivity restored."
        ;;

    status)
        echo "Current iptables rules:"
        for node in $ALL_NODES; do
            echo "=== ${node} ==="
            ssh root@${node} "iptables -L -n" 2>/dev/null || echo "  (unreachable)"
        done
        ;;

    *)
        echo "Network Partition Test Script"
        echo ""
        echo "Usage:"
        echo "  $0 isolate <node_ip>  - Isolate a node from the cluster"
        echo "  $0 heal               - Restore network connectivity"
        echo "  $0 status             - Show current firewall rules"
        echo ""
        echo "Environment Variables:"
        echo "  NODE1_IP, NODE2_IP, NODE3_IP - Node IP addresses"
        ;;
esac
