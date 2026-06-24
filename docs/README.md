# Documentation

This folder is organized by purpose so the current project state stays easy to navigate.

## Structure

- `spec/`
  - current technical specification and runtime design
- `operations/`
  - live runbooks, policies, and operator-facing procedures
- `releases/`
  - release checklists and release notes
- `assets/`
  - static documentation assets such as images

## Recommended entry points

- `spec/README.md`
- `spec/07-configuration.md`
- `spec/08-strategy-execution-model.md`
- `operations/RUNBOOK.md` - includes both Compose and Kind workflows
- `releases/RELEASE_CHECKLIST.md`

## Archive policy

Keep only live, current-state documentation under `docs/`. Dated or point-in-time
material (validation runs, regression snapshots, decision packages) is **not** retained
in-repo — it lives in git history. Do not add closed planning or dated evidence documents
back into the `docs/` surface.
