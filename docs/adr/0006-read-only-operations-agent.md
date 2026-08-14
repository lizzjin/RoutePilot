# ADR-0006: Read-Only Operations Agent

- Status: Accepted, MVP implemented
- Date: 2026-08-13

## Context

ModelPort already exposes health, metrics, request evidence, budgets, Provider
state, and a durable request ledger. Operators still have to correlate those
surfaces manually. A general-purpose autonomous agent would increase risk: it
could execute commands, expose request content to a model, or change routing
and budgets without the same governance boundary as the gateway.

## Decision

The first operations agent is a separate Rust process and optional container.
It has one job: turn bounded, non-content operational facts into durable,
reviewable incidents.

- ModelPort remains the policy and data boundary. The agent has no PostgreSQL
  connection and uses `/internal/ops/v1` with a dedicated expiring service
  account whose exact purpose is `modelport_ops_agent`.
- Detection is deterministic and versioned. The MVP evaluates readiness and
  storage, Provider health, request anomalies, budget capacity, ledger backlog,
  and post-change verification.
- Prompt, response, tool arguments, secrets, raw Provider bodies, and arbitrary
  logs are not sent to the agent or persisted as incident evidence.
- A bounded SQLite spool stores observations awaiting delivery. PostgreSQL in
  ModelPort is the authoritative incident ledger.
- Repeated evidence is hash-deduplicated. Recovery is accepted only from the
  same deterministic event key. Administrators may acknowledge, mitigate,
  monitor, or suppress an event, but cannot manually claim it recovered.
- The rollout modes are `replay`, `shadow`, and `read_only`. Only `read_only`
  submits observations. The container profile, binary, and persisted runtime
  gate are off by default; an explicit profile start uses `shadow` first.
- The MVP cannot execute Shell, SQL, Provider calls, configuration writes, key
  changes, or restart workloads. A future action tier requires an explicit
  allowlist, preview, approval, idempotency, rollback evidence, and a separate
  ADR.
- An optional generic webhook receives the same sanitized incident envelope.
  No model is required. An administrator may select a routable base model for
  advisory diagnosis; local Provider routes are recommended first. Model calls
  use a separate least-privilege inference key and sanitized facts, and cannot
  create facts, choose severity, close incidents, or execute actions.

## Consequences

- The agent can fail without blocking inference. Delivery resumes from its
  bounded spool.
- The console distinguishes observed recovery from human workflow state.
- An operator must create and rotate one dedicated control key, plus an
  independently scoped inference key only when model diagnosis is enabled.
- This does not provide high availability, managed on-call, or autonomous
  remediation. The Small-Team Beta remains a single-instance product.

## Rejected Alternatives

- Direct database access from the agent: duplicates authorization and schema
  coupling, and creates an avoidable credential boundary.
- Reusing an administrator session or general inference key: makes the agent
  more privileged than its read-only job.
- Sending logs or prompts to a hosted model: violates the default data
  minimization boundary and is unnecessary for deterministic detection.
- Automatic restart or configuration repair in the MVP: obscures causality and
  can amplify a partial outage.
