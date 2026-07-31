# Roadmap

Status: product direction. Only capabilities described as implemented in the
current README, API, architecture, configuration, and operations documents are
shipped.

Last reviewed: 2026-07-27.

## Current Product

ModelPort is a self-hosted gateway for one trusted host or small trusted
network. The current baseline includes:

- Anthropic Messages and a scoped OpenAI Chat Completions edge through one
  typed governance pipeline;
- Anthropic and OpenAI-compatible Provider adapters, Tool Use conversion,
  bounded streaming, deterministic routing, and explainable smart routing;
- optional CPA Codex/Claude account channels behind the same Provider,
  governance, retry, and evidence boundaries;
- users, teams, scoped client API keys, quotas, spend checks, Provider
  credential pools, cooldown, and bounded fallback;
- mandatory PostgreSQL request/attempt, usage, budget, audit, idempotency,
  lease, and routing-decision evidence;
- a dashboard, metrics, acceptance tooling, Docker Compose, systemd, and
  backup/restore workflows.

This is useful commercial self-hosting software, but it is not yet a public
multi-tenant or horizontally scalable enterprise platform.

## Near-Term Priorities

The staged forty-user hybrid-routing target is governed by
[ADR-0005](adr/0005-forty-user-hybrid-routing-baseline.md); the current release
remains a single-instance product until each target capability is implemented
and accepted.

1. Prove migration, secret-free backup, recovery, Provider, Tool Use, routing, and
   accounting behavior with repeatable release evidence.
2. Build on the completed auth/control revision-CAS guard by normalizing those
   document domains into tenant-scoped relational repositories and
   cross-domain transactions.
3. Add Provider evidence ingestion and append-only settlement corrections
   without rewriting historical attempts.
4. Improve Chat Completions conformance and extend the typed exchange model
   before adding another public protocol surface.
5. Add OpenTelemetry trace propagation/export and measured performance,
   failure, and recovery objectives.

## Enterprise Admission Work

ModelPort may be described as enterprise ready only after evidence exists for:

| Area | Required outcome |
| --- | --- |
| Protocols | Anthropic Messages and OpenAI Chat are conformant; Responses has a typed, documented contract before beta. |
| Tenancy | Every resource and query is tenant-scoped with negative cross-tenant CI tests. |
| Identity | OIDC, service accounts, group/role mapping, revocation, and recovery controls work across instances. |
| Authorization | Resource-level RBAC and model/Provider/tool/data policies fail closed. |
| Accounting | Every hard limit uses transactional reservation/settlement and reconcilable price evidence. |
| Availability | Multiple data-plane replicas pass failover, drain, rolling-upgrade, and dependency-degradation tests. |
| Security | TLS, DNS-aware egress, secret-manager integration, threat modeling, and security regression evidence are release gates. |
| Observability | W3C trace context, OTLP export, bounded metrics, structured logs, and complete stream outcomes are available without prompt capture by default. |
| Operations | Migrations, backup, point-in-time recovery, rollback, retention, and disaster recovery are tested. |

Compliance certifications are organizational audit outcomes, not repository
features, and must never be claimed by the software alone.

## Later, When Workloads Require It

- Typed OpenAI Responses and selected multimodal/item-oriented operations.
- Distributed limits, sessions, Provider health, cache invalidation, and
  deployable data-plane/control-plane/worker roles.
- Service-account lifecycle, resource-level policy, SCIM, external secret
  managers, SIEM exports, and content-policy hooks.
- Embeddings, images, audio, batch, realtime, regional control planes, and
  advanced learning-based routing only after concrete demand and safety
  evidence.

## Deliberate Non-Goals

- Model inference, training, fine-tuning orchestration, or GPU scheduling.
- A general chat product or end-user prompt workspace.
- A payment processor or authoritative Provider invoice.
- Silent lowest-common-denominator emulation of Provider-specific behavior.
- A LiteLLM runtime dependency or a second public gateway/control plane.
- A custom identity provider, secret manager, metrics database, or policy
  language when established integrations satisfy the need.
- Premature microservice decomposition without an independent scaling or trust
  boundary.

## Decision Rule

Correctness, migration safety, operational evidence, and the measured dominant
workload take priority over adding protocol, Provider, or platform breadth.
Proposals belong in issues, milestones, or an RFC; this roadmap remains a short
statement of accepted direction rather than a dated task backlog.
