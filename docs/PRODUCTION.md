# Production

This document combines the go-live and release-acceptance requirements for the
supported single-host/small-team profile. It is not a certification, warranty,
SLA, or substitute for the operator's threat model and compliance review.

## Supported Boundary

- One trusted backend host or small trusted network.
- PostgreSQL for every durable runtime state path.
- A same-origin HTTPS reverse proxy in front of dashboard and API.
- Server-side Provider credentials and dashboard-issued scoped client keys.
- Operator-owned capacity, upgrades, incidents, retention, backups, Provider
  contracts, privacy, and user support.

Public multi-tenant isolation, active-active operation, distributed
sessions/rate limits, SCIM, and a maintainer-hosted service are not currently
supported.

The accepted forty-user hybrid-routing target is defined in
[ADR-0005](adr/0005-forty-user-hybrid-routing-baseline.md). Its first phase
still uses one ModelPort instance. Routing modes, per-user queue fairness,
managed secrets, and active-active operation remain target behavior until their
individual implementation and acceptance gates pass.

## Go-Live Checklist

- [ ] Pin a released image digest or verified binary provenance.
- [ ] Back up PostgreSQL and apply migrations to an isolated restored copy.
- [ ] Use PostgreSQL TLS `verify-full` for a remote production database.
- [ ] Set unique administrator, router, database, and Provider credentials.
- [ ] Enable `MODELPORT_ENTERPRISE_MODE=1` and resolve every guardrail failure.
- [ ] Configure secure cookies, exact HTTPS origins, exact trusted proxy CIDRs,
      enabled CSRF protection, and private backend/database ports.
- [ ] Set `MODELPORT_REQUIRE_CONTROL_API_KEYS=1`.
- [ ] Verify backup creation, restore drill, encryption, off-host replication,
      retention, and deletion ownership.
- [ ] Alert on readiness, rejection phases, request failures, ledger
      finalization/reconciliation, database saturation, Provider cooldown,
      routing disagreement, latency, and budget exhaustion.
- [ ] Record version, commit, image digest, configuration revision, migration
      set, rollback point, and incident contacts.

## Automated Acceptance

Run configuration validation before starting or restarting the candidate:

```bash
scripts/config-validate.sh
scripts/check-all.sh
scripts/acceptance.sh
scripts/tool-use-acceptance.sh
```

The default acceptance paths use fixtures and temporary control-plane objects.
They do not certify a real Provider. Calls made through a long-running gateway
can leave real request-ledger evidence even when temporary users, teams, keys,
and Providers are removed.

Explicit upstream checks can consume quota:

```bash
scripts/acceptance.sh --upstream
scripts/tool-use-acceptance.sh --upstream
scripts/provider-matrix.sh --model provider:model
```

Record exact Provider/model/path evidence according to
[Providers](PROVIDERS.md#discovery-and-verification). A configured model,
successful discovery, local mock pass, or stream HTTP 200 is not Provider
certification.

## Critical Manual Checks

- Authentication: valid/invalid/locked login, role-filtered navigation,
  ownership isolation, session clearing after principal changes.
- Keys and policy: one-time key reveal, owner status, provider/model/IP scope,
  quota units, rolling spend windows, expiry, revocation, and last-admin
  protection.
- Providers: create/update/disable/delete dependencies, credential state,
  discovery, pool fail-closed behavior, exact model call, stream completion,
  and Tool Use when promised.
- Requests: Messages and Chat Completions text/tool paths, unsupported-field
  rejection, request IDs, idempotency conflict, retry/fallback evidence, usage
  provenance, and smart-routing decision evidence.
- Streaming: handshake validation, semantic first-event latency, terminal SSE
  errors, cancellation, idle/byte limits, and concurrent-stream 429 behavior.
- Dashboard: empty/loading/error/stale states, range provenance, server-side
  log filtering, mobile layout, accessible dialogs, and no invented trace data.
- Operations: authenticated readiness, metrics, backup versus diagnostic export,
  restore drill, retention, migration rollback decision, and incident runbook.

## Release Evidence

Keep:

- commit, build, deployment mode, image digest, SBOM, checksums, and provenance;
- CI, dependency, security, migration, dashboard, and acceptance results;
- exact commands and whether they made paid calls;
- Provider/model/endpoint ownership and dated real-upstream evidence;
- storage backend, backup, restore-drill, retention, and rollback results;
- accepted limits for streaming, quota concurrency, DNS egress, persistence,
  estimates, and process-local enforcement.

A fixture-backed pass supports a controlled gateway trial. A dated Provider
pass supports only the exact model, path, account conditions, and commit tested.

## Reliability Objectives

ModelPort does not publish a universal end-to-end SLO because Provider
availability and local inference capacity dominate results. Each operator
should define:

- data-plane availability and error budget;
- Tool Use workflow success;
- semantic TTFT and full-lifecycle P50/P95 by workload class;
- maximum ledger finalization lag and unreconciled lease count;
- recovery time and recovery point;
- maximum accepted billing reconciliation variance.

Synthetic checks must use the `synthetic` traffic class so they do not distort
business success and cost views.
