# Copilot Remote Agents at Forgepoint

This guide explains how we work with GitHub Copilot coding agents when they collaborate on this repository. Share it with anyone who plans to delegate work to a remote agent so we all follow the same playbook.

## How to engage a remote agent

- Open or update a GitHub issue that clearly describes the task and required outcomes. Group related work together so the agent can complete it end-to-end.
- Add the `#github-pull-request_copilot-coding-agent` trigger phrase in a comment on the issue or pull request when you are ready to hand off the task. Include:
  - The current problem statement and acceptance criteria.
  - Any files, commands, or APIs the agent must touch.
  - Constraints such as "docs only" or "no production deploy" if relevant.
- Review the agent's pull request like any other contribution. Merge once the code, docs, and checks satisfy the acceptance criteria.
- Capture learnings. If the agent needed clarification or tooling that was missing, update this guide or the workflow.

## Environment prepared for the agent

Remote agents run inside an ephemeral GitHub Actions runner. Before each agent session starts, we execute `.github/workflows/copilot-setup-steps.yml` to guarantee the tooling matches our expectations:

- Checks out the repository at the requested revision.
- Installs Bun **1.1.30** and runs `bun install` to hydrate JavaScript workspaces (`apps/*`, `design`).
- Installs the latest stable Rust toolchain plus `clippy` and `rustfmt`, and downloads crates with `cargo fetch` so the server builds quickly.
- Caches Cargo artifacts between workflow runs to cut down on repeated dependency downloads.

You can modify these steps as our stack evolves. Keep the job name `copilot-setup-steps` intact—Copilot will silently ignore workflow files that rename the job.

For more details on how the environment hook works, see the GitHub documentation: [Customizing the development environment for GitHub Copilot coding agent](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/customize-the-agent-environment).

## Repository layout

- Feature work now lives under `extensions/<feature>/` with three sibling folders:
  - `api/` – Rust crate compiled to WebAssembly (`wasm32-wasip1`).
  - `shared/` – Canonical GraphQL schema, fixtures, and other cross-cutting assets.
  - `ui/` – Astro/Vue integration published as `@forgepoint/astro-integration-<feature>`.
- The issues feature is the canonical example: see `extensions/issues/{api,shared,ui}`.
- Legacy `packages/extensions/*` and `packages/integrations/*` paths are removed; update prompts and scripts accordingly.

## Running commands with Nix

GitHub runners and local developers should execute Rust, Bun, or other toolchain commands inside the Nix shell so dependencies (linker, Bun 1.1.30, etc.) are present. Wrap each command with `nix develop --impure -c` from the repository root or feature subdirectory, for example:

```bash
# Build the WASM crate
nix develop --impure -c cargo build --target wasm32-wasip1 --release

# Run Rust tests
nix develop --impure -c cargo test

# Install JS deps or run tests for a UI package
nix develop --impure -c bun install
nix develop --impure -c bun test

# Invoke project justfile recipes
nix develop --impure -c just install-local
```

Always include the full command after `-c`; the shell will provision compilers and environment variables needed for deterministic builds.

> **Non-interactive Vitest:** Bun’s `vitest` runner launches an interactive UI by default. Run it as shown above (`nix develop --impure -c bun test`) or add `--runInBand`/`--reporter=basic` so automated sessions don’t hang waiting for input.

## Providing secrets and configuration

Some tooling requires credentials or configuration values. Store these as GitHub Actions variables or secrets in the `copilot` environment:

1. Repository **Settings → Environments → copilot**.
2. Add environment variables for non-sensitive values (for example, `FORGE_API_BASE_URL`).
3. Add environment secrets for anything sensitive (for example, API keys, tokens).
4. Reference them from workflow steps or remind the agent to read them via `$ENV_VAR_NAME`.

Secrets are **not** committed to the repository and are automatically injected when the agent runs.

## Recommended prompts and boundaries

- Use explicit acceptance criteria and list the files the agent should edit. The more context you provide, the fewer iterations we'll need.
- Mention non-negotiables such as linting rules, formatting tools, or prohibited directories.
- Ask for tests (unit, integration, or snapshot) whenever code paths change. The agent can run them before opening a pull request.
- If the task is inherently exploratory, ask for a short design summary or checklist first, then confirm before the agent commits to a direction.
- Encourage small, reviewable pull requests. Large multi-feature changes are harder to validate and revert.

## Extending the setup

If we adopt additional tooling (for example, Python, Terraform, or database migrations), extend `.github/workflows/copilot-setup-steps.yml` with the necessary installation and caching steps. Keep the total runtime under 15 minutes—longer jobs slow down agent feedback loops.

For heavier compute needs, consider upgrading to larger GitHub-hosted runners and update `runs-on` accordingly (only Ubuntu x64 runners are supported). Document all such changes here so future maintainers know what to expect.

## Troubleshooting

- **Workflow failed:** Check the Actions log for `.github/workflows/copilot-setup-steps.yml`. Fix the failure before retrying an agent run.
- **Missing dependency:** Add installation/cache steps to the workflow or update this guide with manual instructions the agent can follow.
- **Secret unavailable:** Verify it exists in the `copilot` environment and that the workflow references it properly.
- **Unexpected behaviour:** Review the agent session logs and add clarifying guidance to the originating issue or to this document.

Keep this document living—update it whenever the workflow or our expectations evolve.
