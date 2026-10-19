# Claude Instructions

## Role

Claude investigates code, implements changes and verifies the result. At the
user's choice, implementation may be delegated to DeepSeek Flash through
OpenCode following the [AI workflow](docs/development/ai-workflow.md). This
workflow is the user's personal preference, not a mandatory project process.
Before first using it in a new session, ask whether to delegate if the user
has not already chosen. Keep the chosen mode until instructed otherwise.

Delegated tasks belong in `.work/ai/<work-id>/`, not tracked prompts in `docs/`.
The lead agent may directly handle minor documentation, configuration, typo
and local build-error fixes even in delegation mode.

After two unsuccessful DeepSeek attempts, Claude finishes the task directly
under the [attempt-limit rule](docs/development/ai-workflow.md#лимит-попыток).

## Repository work

- Record a baseline before starting; do not mix existing user changes into the task.
- Restrict allowed paths and inspect the task-scoped diff.
- Do not treat an agent's answer or checklist as proof of completion.
- Commits and significant architectural decisions require explicit user approval.
- Verify current external information with available research tools; base technical
  conclusions primarily on primary sources.

## Sources of rules

- [Documentation map](docs/README.md)
- [AI-assisted workflow](docs/development/ai-workflow.md)
- [Builds and checks](docs/development/README.md)
- [Architecture](docs/development/architecture.md)
- [Engineering decisions](docs/decisions/README.md)
- Skills in `.claude/skills/` refine specific practices and do not replace
  these normative documents.

Temporary prompts, reviews, logs and screenshots belong only in gitignored
`.work/ai/`. Add roadmaps, tasks, decisions and research to `docs/` only when
they remain useful after the current session.
