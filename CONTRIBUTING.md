# Contributing

## Preferred workflow

Use the repository `make` interface for the main local workflow:

```bash
cp compose/.env.example compose/.env
make build-base-images
make compose-up
make health
make validate-ci
```

For a fresh clone, run the environment bootstrap first:

```bash
sudo dnf install -y openssl-devel
cp compose/.env.example compose/.env
make build-base-images
```

On Fedora 43 WSL, the validation scripts default
`PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` so PyO3 0.21.x remains usable with the
system Python 3.14 during host-side Rust checks.

Recommended sequence before commit/push:

1. run `make validate-workspace-quick` during development
2. run `make validate-ci` before commit/push
3. use `make health` when touching runtime behavior

Prefer `make` targets over calling lower-level scripts directly unless you are working on an advanced or diagnostic flow.

The supported local runtime path is the bridge-based Docker Desktop workflow exposed through `make compose-*`.

## Commit Message Convention

Use Conventional Commit with non-empty description:

- `feat(scope): add deterministic risk gate`
- `fix(scope): handle redis reconnect on startup`
- `chore(scope): simplify compose wrapper cleanup`
- `docs(scope): update release checklist`

Guidelines:

- Required format: `type(scope): description` or `type: description`.
- Do not use generic subjects such as `chore`, `feat`, `docs:` or `feat-monitor:`.
- Keep subject in imperative mood.
- Prefer explicit scope (`api`, `strategy`, `executor`, `compose`, `docs`, `ci`).

## Local validation

Main validation commands:

- `make validate-workspace-quick`
- `make validate-full`
- `make validate-ci`

If you run Cargo directly on the host, prefer:

```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 cargo check --workspace --locked
```

Runtime checks:

- `make compose-up`
- `make health`
- `make validate-runtime`
