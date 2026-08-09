# Roadmap

Status: accepted Small-Team Beta direction.

Last reviewed: 2026-08-09.

## Product Contract

ModelPort is free, MIT-licensed, self-hosted software for a 20–50 person
Chinese internal development team that uses local models and approved cloud
Providers. The platform administrator is the primary operator; developers get
scoped keys, stable logical models, their own request evidence, and copyable
client configuration.

The core user outcome is:

> Sensitive code stays local by default. A cloud route is used only when an
> administrator-approved project policy permits it, and every egress decision
> has an identity, reason, route, outcome, and cost provenance.

Success is the first governed request within 30 minutes and sustained weekly
team use without policy bypass—not Provider count, raw request volume, GitHub
stars, or revenue. ModelPort has no paid edition, hosted service, or feature
tier.

## v0.1.x Small-Team Beta Freeze

For the first 6–8 weeks after v0.1.0, new protocol, Provider, and platform
breadth is frozen. A change may break the freeze only when it fixes a security
issue, data-loss risk, release/upgrade blocker, or a reproducible blocker found
by a design-partner team.

Work during the freeze is ordered as follows:

1. **Activation:** prebuilt signed images, digest/SBOM evidence, state-driven
   onboarding, credential resolution/test state, stable logical models, and a
   first governed request in at most 30 minutes.
2. **Developer self-service:** own scoped/expiring keys, readable model catalog,
   copyable Claude Code/SDK configuration, own request logs, and explicit local
   versus cloud route evidence.
3. **Privacy and policy:** zero maintainer telemetry, no prompt/response/tool
   content persistence, owner-scoped logs, 30/90/395-day retention preview and
   apply, legal hold, `local_strict` default, and no silent Tool Use downgrade.
4. **Operations:** independent static Dashboard, liveness/readiness separation,
   graceful drain and ledger finalization, safe maintenance upgrade/rollback,
   official Prometheus rules, Grafana dashboard, and alert runbook.
5. **Validation:** two or three real teams, each with an administrator and at
   least five active developers for two weeks, providing only previewed,
   content-free diagnostic evidence.

Beta exit evidence:

- at least 80% of clean Tier 1 installs complete a governed request in 30
  minutes;
- week two active-developer coverage is at least 60%, and week four at least
  80%, using the locally calculated definition in the product plan;
- zero unapproved cloud egress and zero cross-user request-log access;
- every request exposes a stable request ID, logical model, actual route, and
  egress policy basis;
- clean install, upgrade, safe stop, backup, restore, and rollback acceptance
  passes for Linux x86_64;
- no open P0/P1 security, privacy, ledger, or activation blocker.

## Explicitly Deferred During The Freeze

- OpenAI Responses, realtime, embeddings, image/audio/multimodal APIs, and new
  public protocol surfaces.
- Provider breadth that is not required to unblock a design partner's existing
  approved route.
- Kubernetes, multiple active replicas, active-active/high availability,
  distributed limits/sessions/stream permits, or zero-downtime upgrade claims.
- Public multi-tenancy, a hosted service, payment/licensing systems, paid
  features, or an “enterprise ready” label.
- Online learning that directly changes production routing weights, developer-
  exposed router tuning, or silent capability downgrade.
- Dashboard storage of plaintext Provider secrets, full English UI
  internationalization, a chat workspace, and maintainer-operated telemetry.

Experimental compatibility work may continue behind explicit flags, but it
cannot enter the default route or support matrix without the evidence required
by [Compatibility](COMPATIBILITY.md).

## After Beta Evidence

Only measured design-partner needs can reorder these candidates:

- append-only Provider invoice reconciliation and bounded remaining-budget
  metrics;
- PostgreSQL pool, active-stream, and per-Provider latency telemetry;
- resource-level policy and additional identity/secret-manager integrations;
- a typed protocol extension whose fidelity and Tool Use contract fails closed;
- arm64 promotion after equivalent install/upgrade/restore evidence.

Multiple replicas, public tenants, or a new platform target require a separate
ADR and cannot be inferred from Docker build success.

## Stable Release Gate

A stable (`v1.0.0`) claim requires all Beta exit evidence plus at least two
named maintainers with repository release and private security-response access.
Both must complete a release rehearsal and security handoff. Until that gate is
met, the project remains Beta and makes no response-time or availability SLA.

## Decision Rule

Privacy, fail-closed policy, migration safety, explainable evidence, and the
measured dominant workload outrank feature breadth. A proposal that expands the
support surface must name the design-partner blocker, operating cost, rollback,
test evidence, and what existing frozen work it displaces.
