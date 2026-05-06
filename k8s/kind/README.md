# ViperTrade no Kind

Este overlay adapta o stack local do Compose/Podman para o cluster Kind criado por
`/home/paiva/setup-k8s-wsl2/setup.sh`.

## Pré-requisitos

- script `setup-k8s-wsl2/setup.sh` executado (provisiona Podman, Kind, registry local)
- cluster Kind `dev` ativo
- registry local `localhost:5001` acessível
- namespace `vipertrade` criado

## Uso

```bash
# 1. Preparar o registry WSL (garante que está rodando na rede 'kind')
./scripts/kind/prepare-wsl.sh

# 2. Construir e enviar imagens para o registry local
make kind-build-images

# 3. Deploy no cluster
make kind-deploy

# 4. Verificar status
make kind-status

# Health check detalhado
./scripts/kind/health-check.sh

# Validação de runtime (modo kind)
./scripts/validate-runtime.sh kind all
```

Para remover os recursos:

```bash
make kind-delete
```

## WSL2 + Podman

Este ambiente foi otimizado para WSL2 com Podman rootless:

- **Registry local**: roda como container Podman na rede `kind`, acessível em `localhost:5001`
- **KIND_EXPERIMENTAL_PROVIDER=podman**: configurado no `~/.bashrc` pelo setup
- **Imagens**: construídas com Podman, enviadas para o registry local, puxadas pelo Kind
- **Network**: o registry compartilha a rede `kind` com o cluster, permitindo pulls diretos

Se o registry não estiver acessível a partir do WSL (ex.: Docker Desktop rodando no Windows),
use `KIND_REGISTRY=host.docker.internal:5001`:

```bash
KIND_REGISTRY=host.docker.internal:5001 make kind-build-images
```

## Portas de acesso

| Serviço | Porta | URL |
|---------|-------|-----|
| API (REST) | 8080 (NodePort) | http://localhost:8080 |
| Web Dashboard | 30080 (NodePort) | http://localhost:30080 |
| Grafana/Prometheus (se adicionado) | 30000+ | - |

## Troubleshooting

### Registry não responde
```bash
# Verificar se o container do registry está rodando
podman ps | grep kind-registry

# Reiniciar o registry
podman restart kind-registry

# Verificar logs
podman logs kind-registry
```

### Pods em CrashLoopBackOff
```bash
# Logs do pod problemático
kubectl logs -n vipertrade -f deployment/strategy

# Eventos do pod
kubectl get events -n vipertrade --sort-by=.metadata.creationTimestamp | tail -20
```

### Imagens não encontradas no registry
```bash
# Listar tags disponíveis
curl http://localhost:5001/v2/vipertrade-strategy/tags/list

# Rebuild com tag correta
KIND_REGISTRY=localhost:5001 IMAGE_TAG=dev make kind-build-images
```

### Reset completo
```bash
make kind-delete
podman rm -f kind-registry 2>/dev/null || true
./scripts/kind/prepare-wsl.sh
make kind-build-images
make kind-deploy
```

## Notas sobre credenciais

As credenciais em `k8s/kind/secret.yaml` são placeholders locais. Para testnet/mainnet,
edite o Secret antes do deploy:

```bash
kubectl -n vipertrade edit secret vipertrade-secrets
```

Ou crie a partir de `compose/.env`:

```bash
kubectl -n vipertrade create secret generic vipertrade-secrets \
  --from-env-file=compose/.env \
  --dry-run=client -o yaml | kubectl apply -f -
```
