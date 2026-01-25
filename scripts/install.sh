#!/bin/bash
# APFSDS One-Click Deployment Script
# Supports single-machine deployment with K3s + Helm

set -e

VERSION="0.2.0"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Banner
echo "=========================================="
echo "  APFSDS Installer v${VERSION}"
echo "  One-Click K3s + Helm Deployment"
echo "=========================================="
echo ""

#==========================================
# CONFIGURATION SECTION
# Edit these values before running
#==========================================

# Deployment Mode
DEPLOYMENT_MODE="all-in-one"  # Options: all-in-one, handler, exit

# Server Configuration
SERVER_DOMAIN="proxy.example.com"
SERVER_BIND="0.0.0.0:25347"

# Exit Nodes (for handler mode)
EXIT_NODES=(
    "tokyo:10.0.1.100:25347:1.0"
    "singapore:10.0.1.101:25347:0.5"
)

# Storage Configuration
TMPFS_SIZE="512Mi"
DISK_SIZE="10Gi"
CLICKHOUSE_ENABLED="false"

# Security (auto-generated if empty)
SERVER_SECRET_KEY=""
HMAC_SECRET=""

# Resource Limits
CPU_LIMIT="2000m"
MEMORY_LIMIT="4Gi"
CPU_REQUEST="500m"
MEMORY_REQUEST="1Gi"

# K3s Configuration
K3S_VERSION="v1.28.5+k3s1"
DISABLE_TRAEFIK="true"

#==========================================
# HELPER FUNCTIONS
#==========================================

# Detect OS
detect_os() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$ID
        OS_VERSION=$VERSION_ID
    else
        log_error "Cannot detect OS"
        exit 1
    fi
    log_info "Detected OS: $OS $OS_VERSION"
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Generate random secret
generate_secret() {
    openssl rand -hex 32
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    if ! command_exists curl; then
        log_error "curl is required but not installed"
        exit 1
    fi

    if ! command_exists openssl; then
        log_error "openssl is required but not installed"
        exit 1
    fi

    # Check for kubectl (required for any K8s deployment)
    if ! command_exists kubectl; then
        log_warn "kubectl not found, will install K3s"
        NEED_K3S=true
    else
        log_info "kubectl found, will use existing K8s cluster"
        NEED_K3S=false
    fi

    log_info "Prerequisites check passed"
}

#==========================================
# K3S INSTALLATION
#==========================================

install_k3s() {
    if [ "$NEED_K3S" != "true" ]; then
        log_info "Using existing Kubernetes cluster, skipping K3s installation"
        return 0
    fi

    if command_exists k3s; then
        log_info "K3s already installed, skipping..."
        return 0
    fi

    log_info "Installing K3s ${K3S_VERSION}..."

    local install_cmd="curl -sfL https://get.k3s.io | INSTALL_K3S_VERSION=${K3S_VERSION}"

    if [ "$DISABLE_TRAEFIK" = "true" ]; then
        install_cmd="${install_cmd} INSTALL_K3S_EXEC='--disable=traefik'"
    fi

    eval "$install_cmd sh -"

    # Setup kubeconfig
    mkdir -p ~/.kube
    sudo cp /etc/rancher/k3s/k3s.yaml ~/.kube/config
    sudo chown $(id -u):$(id -g) ~/.kube/config
    export KUBECONFIG=~/.kube/config

    log_info "K3s installed successfully"
}

#==========================================
# HELM INSTALLATION
#==========================================

install_helm() {
    if command_exists helm; then
        log_info "Helm already installed, skipping..."
        return 0
    fi
    
    log_info "Installing Helm..."
    curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
    log_info "Helm installed successfully"
}

#==========================================
# APFSDS DEPLOYMENT
#==========================================

# Generate secrets if not provided
generate_secrets() {
    if [ -z "$SERVER_SECRET_KEY" ]; then
        SERVER_SECRET_KEY=$(generate_secret)
        log_info "Generated SERVER_SECRET_KEY"
    fi
    
    if [ -z "$HMAC_SECRET" ]; then
        HMAC_SECRET=$(generate_secret)
        log_info "Generated HMAC_SECRET"
    fi
}

# Create values file
create_values_file() {
    log_info "Creating Helm values file..."
    
    cat > /tmp/apfsds-values.yaml <<YAML
replicaCount: 1

image:
  repository: ghcr.io/rand0mdevel0per/apfsds.rs
  tag: "${VERSION}"
  pullPolicy: IfNotPresent

deployment:
  mode: ${DEPLOYMENT_MODE}

server:
  domain: ${SERVER_DOMAIN}
  bind: ${SERVER_BIND}

security:
  serverSecretKey: "${SERVER_SECRET_KEY}"
  hmacSecret: "${HMAC_SECRET}"

storage:
  tmpfs:
    enabled: true
    size: ${TMPFS_SIZE}
  disk:
    enabled: true
    size: ${DISK_SIZE}
  clickhouse:
    enabled: ${CLICKHOUSE_ENABLED}

resources:
  limits:
    cpu: ${CPU_LIMIT}
    memory: ${MEMORY_LIMIT}
  requests:
    cpu: ${CPU_REQUEST}
    memory: ${MEMORY_REQUEST}
YAML

    log_info "Values file created at /tmp/apfsds-values.yaml"
}

# Deploy APFSDS
deploy_apfsds() {
    log_info "Deploying APFSDS..."
    
    # Wait for kubectl to be ready
    log_info "Waiting for kubectl to be ready..."
    until kubectl get nodes >/dev/null 2>&1; do
        sleep 2
    done
    
    # Create namespace
    kubectl create namespace apfsds --dry-run=client -o yaml | kubectl apply -f -
    
    # Deploy with Helm
    helm upgrade --install apfsds "${SCRIPT_DIR}/../helm-chart" \
        --namespace apfsds \
        --values /tmp/apfsds-values.yaml \
        --wait \
        --timeout 10m
    
    log_info "APFSDS deployed successfully!"
}

#==========================================
# MAIN EXECUTION
#==========================================

main() {
    log_info "Starting APFSDS installation..."
    
    # Run checks
    detect_os
    check_prerequisites
    
    # Install components
    install_k3s
    install_helm
    
    # Generate secrets
    generate_secrets
    
    # Create values and deploy
    create_values_file
    deploy_apfsds
    
    # Print summary
    echo ""
    echo "=========================================="
    echo "  Installation Complete! 🎉"
    echo "=========================================="
    echo ""
    echo "Deployment Mode: ${DEPLOYMENT_MODE}"
    echo "Server Domain: ${SERVER_DOMAIN}"
    echo "Namespace: apfsds"
    echo ""
    echo "Check status:"
    echo "  kubectl get pods -n apfsds"
    echo ""
    echo "View logs:"
    echo "  kubectl logs -n apfsds -l app=apfsds -f"
    echo ""
    echo "Access metrics:"
    echo "  kubectl port-forward -n apfsds svc/apfsds 9090:9090"
    echo ""
}

# Run main function
main "$@"
