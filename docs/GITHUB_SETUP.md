# Repository, CI, And Release Setup

This document records the intended GitHub maintenance policy. It is not runtime
configuration.

## Repository Metadata

- Description: `Self-hosted multi-protocol model gateway for Anthropic and OpenAI-compatible clients.`
- Topics: `claude-code`, `anthropic`, `openai-compatible`, `llm-gateway`,
  `model-router`, `rust`, `vscode`, `deepseek`.
- License: MIT.

Do not use topics or the description to claim verified providers that have no
dated result in [Provider Matrix](PROVIDER_MATRIX.md).

## Branch Protection

Protect `main` with a repository ruleset:

- pull requests before merge;
- **Repository checks**, **PostgreSQL dashboard E2E**, dependency review, and
  CodeQL as required statuses;
- up-to-date branches and resolved review conversations;
- at least one maintainer review for security/persistence/deployment changes;
- restricted force pushes and branch deletion;
- signed commits/tags when the maintainer signing setup is available.

The current CI uses the pinned Rust 1.96.0 toolchain, Node.js 24, locked npm
dependencies, Rust fmt/test/clippy, dashboard typecheck/lint/unit/build, shell
syntax, and configuration-example checks through `scripts/check-all.sh`.
Playwright E2E and paid upstream tests remain separate unless a deterministic
CI environment is added.

Workflows use least-privilege permissions, full commit-SHA Action pins,
concurrency cancellation, and no persistent checkout credential where it is
unnecessary.

## Issues And Pull Requests

The repository includes bug/feature issue forms, a PR template, and CODEOWNERS.
Issue forms must not require or invite a full `.env`, provider key, session/API
token, complete backup, raw prompt/response, or unreviewed log.

Provider choices in forms should track the catalog or allow free text rather
than silently omitting supported templates.

Security details belong in the private process in [SECURITY.md](../SECURITY.md),
not a public bug report.

## Releases

Every release should include:

- behavior summary and upgrade steps;
- configuration variables/defaults added, changed, or removed;
- persistence schema/migration and rollback notes;
- Docker/systemd changes;
- validation commands and exact commit;
- only dated real-provider results that were actually run;
- stream, quota, DNS SSRF, persistence, and cost-estimation limits that remain.

Maintain `CHANGELOG.md` from the first public release. Backend and dashboard
share one version, and runtime diagnostics expose the version, revision, and
source state. Follow [the release process](RELEASING.md).

Enable private vulnerability reporting, Dependabot alerts/security updates,
immutable releases where available, tag protection, and artifact attestations
in repository settings. Workflow files cannot enable those external controls.

## CI Secret Policy

Routine CI must not make paid upstream calls. If a future protected workflow
does so, use a dedicated low-quota account, manual/environment approval,
redacted artifacts, strict timeout/cost limits, and no fork access. GitHub
Secrets can protect storage, but they do not remove billing, prompt disclosure,
or third-party availability risk.
