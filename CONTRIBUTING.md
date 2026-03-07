# Contributing

## Commit Message Convention

Use Conventional Commit with non-empty description:

- `feat(scope): add deterministic risk gate`
- `fix(scope): handle redis reconnect on startup`
- `chore(scope): harden podman wrapper cleanup`
- `docs(scope): update release checklist`

Guidelines:

- Required format: `type(scope): description` or `type: description`.
- Do not use generic subjects such as `chore`, `feat`, `docs:` or `feat-monitor:`.
- Keep subject in imperative mood.
- Prefer explicit scope (`api`, `strategy`, `executor`, `compose`, `docs`, `ci`).

## Local Hook Setup

Enable local commit message validation:

```bash
./scripts/setup-git-hooks.sh
```

This configures `core.hooksPath=.githooks` and blocks invalid commit subjects.
