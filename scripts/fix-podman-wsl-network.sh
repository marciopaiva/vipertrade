#!/bin/bash
set -euo pipefail

echo "ViperTrade - Fix Podman WSL Network (netavark + iptables)"

if ! command -v podman >/dev/null 2>&1; then
  echo "ERROR: podman not found"
  exit 1
fi

# System override
sudo mkdir -p /etc/containers/containers.conf.d
sudo tee /etc/containers/containers.conf.d/99-vipertrade-wsl-network.conf >/dev/null <<'CONF'
[network]
network_backend = "netavark"
firewall_driver = "iptables"
CONF

# User override
mkdir -p ~/.config/containers/containers.conf.d
cat > ~/.config/containers/containers.conf.d/99-vipertrade-wsl-network.conf <<'CONF'
[network]
network_backend = "netavark"
firewall_driver = "iptables"
CONF

# Keep base files simple and modern
cat > ~/.config/containers/containers.conf <<'CONF'
[network]
network_backend = "netavark"
CONF

sudo tee /etc/containers/containers.conf >/dev/null <<'CONF'
[network]
network_backend = "netavark"
CONF

# Reload Podman network stack
podman system migrate || true
sudo podman system migrate || true

echo "Rootless network backend:"
podman info --format json | jq -r '.host.networkBackend'

echo "Rootful network backend:"
sudo podman info --format json | jq -r '.host.networkBackend'

echo "Smoke test (rootless bridge):"
podman run --rm --network bridge docker.io/library/alpine:3.20 true

echo "SUCCESS: Podman bridge networking configured for WSL"
