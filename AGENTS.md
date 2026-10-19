# Codex Instructions

## Role

Codex investigates code, implements changes and verifies the result. At the
user's choice, implementation may be delegated to DeepSeek Flash through
OpenCode following the [AI workflow](docs/development/ai-workflow.md). This
workflow is the user's personal preference, not a mandatory project process.
Before first using it in a new session, ask whether to delegate if the user
has not already chosen. Keep the chosen mode until instructed otherwise.

Delegated tasks belong in `.work/ai/<work-id>/`, not tracked prompts in `docs/`.
The lead agent may directly handle minor documentation, configuration, typo
and local build-error fixes even in delegation mode.

After two unsuccessful DeepSeek attempts, Codex finishes the task directly
under the [attempt-limit rule](docs/development/ai-workflow.md#лимит-попыток).

## Repository work

- Record a baseline before starting; do not mix existing user changes into the task.
- Restrict allowed paths and inspect the task-scoped diff.
- Do not treat an agent's answer or checklist as proof of completion.
- On Windows, do not run Rust checks/builds with bare `cargo`: use
  `scripts/cargo.ps1` for targeted commands and `scripts/build.ps1` for full
  debug/release builds so local toolchains and native dependencies apply.
- Commits and significant architectural decisions require explicit user approval.
- Verify current external information with available research tools; base technical
  conclusions primarily on primary sources.

## Sources of rules

For UI changes, consult the UI map and verification rules below. Update the
map when adding/moving screens. Respect the status of visual references:
proposals are not an approved general standard. Check relevant states and
scenarios; put the temporary report path in the specific task file.

- [Documentation map](docs/README.md)
- [UI map](docs/development/ui-map.md)
- [UI verification and approval](docs/development/ui-guidelines.md)
- [AI-assisted workflow](docs/development/ai-workflow.md)
- [Builds and checks](docs/development/README.md)
- [Architecture](docs/development/architecture.md)
- [Engineering decisions](docs/decisions/README.md)

Temporary prompts, reviews, logs and screenshots belong only in gitignored
`.work/ai/`. Add roadmaps, tasks, decisions and research to `docs/` only when
they remain useful after the current session.
