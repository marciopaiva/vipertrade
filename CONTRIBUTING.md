# Contributing

## Commit Message Convention

Use short Conventional Commit style:

- `feat(scope): add deterministic risk gate`
- `fix(scope): handle redis reconnect on startup`
- `chore(scope): harden podman wrapper cleanup`
- `docs(scope): update release checklist`

Guidelines:

- Avoid generic titles like `chore` or `feat` alone.
- Keep subject in imperative mood.
- Prefer explicit scope (`api`, `strategy`, `compose`, `docs`, `ci`).

## Local Hook Setup

Enable local commit message validation:

```bash
./scripts/setup-git-hooks.sh
```

This configures `core.hooksPath=.githooks` and blocks generic commit subjects.
