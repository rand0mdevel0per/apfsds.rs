#!/bin/bash
# VMware P2P Testing - Multi-Node Deployment Script
#
# This script deploys APFSDS nodes across multiple VMs for testing
# distributed consensus and network partition scenarios.
#
# Prerequisites:
# - 3 VMs with SSH access (node1, node2, node3)
# - Rust toolchain installed on each VM
# - APFSDS source cloned to each VM

set -e

COLOR_GREEN='\033[0;32m'
COLOR_YELLOW='\033[1;33m'
COLOR_NC='\033[0m'

log() {
    echo -e "${COLOR_GREEN}[APFSDS-DEPLOY]${COLOR_NC} $1"
}

warn() {
    echo -e "${COLOR_YELLOW}[WARNING]${COLOR_NC} $1"
}

# Configuration - Edit these for your environment
NODE1_IP="${NODE1_IP:-192.168.1.101}"
NODE2_IP="${NODE2_IP:-192.168.1.102}"
NODE3_IP="${NODE3_IP:-192.168.1.103}"
SSH_USER="${SSH_USER:-ubuntu}"
APFSDS_PATH="${APFSDS_PATH:-/opt/apfsds}"
DAEMON_PORT=25347
MGMT_PORT=25348

# Generate node configurations
generate_config() {
    local node_id=$1
    local bind_ip=$2
    local peers=$3

    cat <<EOF
[server]
bind = "${bind_ip}:${DAEMON_PORT}"
mode = "handler"

[raft]
node_id = ${node_id}
peers = [${peers}]

[storage]
disk_path = "/var/lib/apfsds"

[monitoring]
prometheus_bind = "${bind_ip}:9090"
EOF
}

# Deploy to a single node
deploy_node() {
    local node_ip=$1
    local node_id=$2
    local config=$3

    log "Deploying to Node ${node_id} (${node_ip})..."

    # Copy config
    echo "${config}" | ssh ${SSH_USER}@${node_ip} "cat > ${APFSDS_PATH}/daemon.toml"

    # Build and run
    ssh ${SSH_USER}@${node_ip} "cd ${APFSDS_PATH} && cargo build --release -p apfsds-daemon"
    ssh ${SSH_USER}@${node_ip} "sudo systemctl restart apfsdsd || ${APFSDS_PATH}/target/release/apfsdsd --config ${APFSDS_PATH}/daemon.toml &"

    log "Node ${node_id} deployed."
}

# Main deployment
main() {
    log "Starting APFSDS Multi-Node Deployment..."

    PEERS="\"${NODE1_IP}:${DAEMON_PORT}\", \"${NODE2_IP}:${DAEMON_PORT}\", \"${NODE3_IP}:${DAEMON_PORT}\""

    CONFIG1=$(generate_config 1 "${NODE1_IP}" "${PEERS}")
    CONFIG2=$(generate_config 2 "${NODE2_IP}" "${PEERS}")
    CONFIG3=$(generate_config 3 "${NODE3_IP}" "${PEERS}")

    deploy_node "${NODE1_IP}" 1 "${CONFIG1}"
    deploy_node "${NODE2_IP}" 2 "${CONFIG2}"
    deploy_node "${NODE3_IP}" 3 "${CONFIG3}"

    log "All nodes deployed. Waiting for cluster formation..."
    sleep 10

    # Verify cluster health
    log "Checking cluster health..."
    for ip in ${NODE1_IP} ${NODE2_IP} ${NODE3_IP}; do
        if curl -s "http://${ip}:${MGMT_PORT}/admin/stats" > /dev/null 2>&1; then
            log "Node ${ip} is healthy"
        else
            warn "Node ${ip} may not be responding"
        fi
    done

    log "Deployment complete!"
    echo ""
    echo "Management Dashboard URLs:"
    echo "  - http://${NODE1_IP}:${MGMT_PORT}/"
    echo "  - http://${NODE2_IP}:${MGMT_PORT}/"
    echo "  - http://${NODE3_IP}:${MGMT_PORT}/"
}

main "$@"
