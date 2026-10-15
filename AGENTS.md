## Agent skills

### Issue tracker

Issues and specs live in this repo's GitHub Issues; use `gh`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default triage labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Use the single-context layout. See `docs/agents/domain.md`.

## Model setup

The main session is the orchestrator (Sonnet 5.5, medium effort; see `.claude/settings.json`). It plans, makes design decisions and reviews. Delegate well-specified implementation subtasks to the `executor` subagent (Haiku 5.5, medium effort; see `.claude/agents/executor.md`) and give it explicit files and acceptance criteria.
