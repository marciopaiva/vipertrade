# Documentation

This folder is organized by purpose so the current project state stays easy to navigate.

## Structure

- `spec/`
  - current technical specification and runtime design
- `operations/`
  - live runbooks, gates, policies, and operator-facing procedures
- `operations/evidence/`
  - dated validation reports, baselines, regressions, and decision packages
- `releases/`
  - release checklists and release notes
- `legacy/`
  - historical source material retained for traceability
- `assets/`
  - static documentation assets such as images

## Recommended entry points

- `spec/README.md`
- `spec/07-configuration.md`
- `spec/08-strategy-execution-model.md`
- `operations/RUNBOOK.md`
- `releases/RELEASE_CHECKLIST.md`
- `legacy/README.md`

## Archive policy

Keep only live documentation in:

- `docs/spec/`
- `docs/operations/`
- `docs/releases/`

Move dated or historical material to:

- `docs/operations/evidence/`
- `docs/legacy/`

Do not add closed planning documents back into the top-level `docs/` surface.
