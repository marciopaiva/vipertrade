# ViperTrade on Kind

This overlay adapts the local Compose/Podman stack for the Kind cluster provisioned by
`/home/paiva/setup-k8s-wsl2/setup.sh`.

## Deployment Options

ViperTrade can run via:

1. **Docker/Podman Compose** (recommended for local development)
   - See main README.md for compose workflow
   - Services communicate via compose network

2. **Kind Kubernetes** (alternative for K8s development)
   - See below for Kind-specific instructions
   - Services communicate via K8s DNS

## Kind Prerequisites

- script `setup-k8s-wsl2/setup.sh` executed (provisions Podman, Kind, local registry)
- Kind cluster `dev` active
- local registry `localhost:5001` accessible
- namespace `vipertrade` created

## Usage

```bash
# 1. Prepare WSL registry (ensures it's running on 'kind' network)
./scripts/kind/prepare-wsl.sh

# 2. Build and push images to local registry
make kind-build-images

# 3. Deploy to cluster
make kind-deploy

# 4. Check status
make kind-status

# Detailed health check
./scripts/kind/health-check.sh

# Runtime validation (kind mode)
./scripts/validate-runtime.sh kind all
```

To remove resources:

```bash
make kind-delete
```

## WSL2 + Podman

This environment is optimized for WSL2 with rootless Podman:

- **Local registry**: runs as Podman container on `kind` network, accessible at `localhost:5001`
- **KIND_EXPERIMENTAL_PROVIDER=podman**: configured in `~/.bashrc` by the setup script
- **Images**: built with Podman, pushed to local registry, pulled by Kind
- **Network**: registry shares the `kind` network with the cluster for direct pulls

If the registry is not accessible from WSL (e.g., Docker Desktop running on Windows),
use `KIND_REGISTRY=host.docker.internal:5001`:

```bash
KIND_REGISTRY=host.docker.internal:5001 make kind-build-images
```

## Service Ports

| Service | Port | URL |
|---------|------|-----|
| API (REST) | 8080 (NodePort) | http://localhost:8080 |
| Web Dashboard | 30080 (NodePort) | http://localhost:30080 |
| Grafana/Prometheus (if added) | 30000+ | - |

## Troubleshooting

### Registry not responding

```bash
# Check if registry container is running
podman ps | grep kind-registry

# Restart registry
podman restart kind-registry

# Check logs
podman logs kind-registry
```

### Pods in CrashLoopBackOff

```bash
# Logs from problematic pod
kubectl logs -n vipertrade -f deployment/strategy

# Pod events
kubectl get events -n vipertrade --sort-by=.metadata.creationTimestamp | tail -20
```

### Images not found in registry

```bash
# List available tags
curl http://localhost:5001/v2/vipertrade-strategy/tags/list

# Rebuild with correct tag
KIND_REGISTRY=localhost:5001 IMAGE_TAG=dev make kind-build-images
```

### Full reset

```bash
make kind-delete
podman rm -f kind-registry 2>/dev/null || true
./scripts/kind/prepare-wsl.sh
make kind-build-images
make kind-deploy
```

## Credentials Note

Credentials in `k8s/kind/secret.yaml` are local placeholders. For testnet/mainnet,
edit the Secret before deploying:

```bash
kubectl -n vipertrade edit secret vipertrade-secrets
```

Or create from `compose/.env`:

```bash
kubectl -n vipertrade create secret generic vipertrade-secrets \
  --from-env-file=compose/.env \
  --dry-run=client -o yaml | kubectl apply -f -
```