# ADR-0003: Relational State And Transaction Boundaries

- Status: Accepted
- Date: 2026-07-15

## Implementation status

The current baseline implements SQLx/Tokio/rustls, bounded pools, embedded
migrations, normalized tenant parents, request/Provider-attempt lifecycle rows,
hashed idempotency claims, renewable instance leases, expired-row
reconciliation, transactional budgets, complete operational request snapshots,
relational usage/quota/spend queries, and append-only audit events. Runtime
requires PostgreSQL; the memory ledger is test-only. Migration
`0005_current_operational_schema.sql` preserves normalized request/attempt
history, uses conservative defaults for dimensions absent from the older
schema, and derives final Provider/retry snapshots only from existing attempts.
Current estimated cost remains rounded USD micro-units; response replay and
invoice-grade settlement are still out of scope.

## Context

At the time of this decision, the file and PostgreSQL storage backends persisted
two complete JSON documents, and PostgreSQL used a synchronous `NoTls` client.
That baseline could not provide efficient tenant-scoped queries, independent
retention, concurrent mutation safety, schema migrations, or atomic budget
accounting. The implementation-status section records the first completed
expand step without rewriting this decision context.

Enterprise accounting must distinguish a client request from its Provider
attempts and must reserve, settle, release, and reconcile spend without silent
historical mutation.

## Decision

ModelPort uses normalized PostgreSQL tables through SQLx with Tokio, Rustls,
connection pooling, explicit timeouts, and embedded versioned migrations.
PostgreSQL is authoritative for request/attempt lifecycle, usage accounting,
budgets, and audit history. Low-frequency identity and control definitions may
remain documents, but they contain no request usage, spend ledger, or activity
history.

Repository traits isolate domain logic from persistence, but they do not hide
transaction boundaries. Every tenant-owned repository operation requires an
explicit `TenantScope`.

The minimum transaction boundaries are:

- identity, membership, role-binding, and revocation mutation;
- versioned policy/route publication plus audit event;
- request creation plus idempotency claim plus budget reservation;
- Provider-attempt creation and terminal outcome;
- final usage settlement and reservation release;
- append-only usage adjustment and audit export cursor advancement.

Money uses PostgreSQL `NUMERIC` and a Rust decimal type with explicit currency.
Usage records retain price-book revision and evidence source. Historical
settlements are corrected with append-only adjustments, not destructive edits.

Redis may provide distributed rate limits, short leases, cache invalidation, and
ephemeral coordination. Redis is not the authoritative budget or usage ledger.

The current operational migration is an evidence-preserving contract:

1. Back up the database and restore it into an isolated drill environment.
2. Apply all migrations and verify request/attempt counts and representative
   final Provider/retry snapshots.
3. Apply the same migration during the planned production cutover.
4. Retain the pre-upgrade backup according to the operator's recovery policy;
   ModelPort never fabricates prompt content, usage, pricing, or identity facts
   that were absent from the older schema.

## Consequences

- The synchronous `postgres` JSON document backend is removed from enterprise
  mode after the importer and compatibility window are complete.
- Database integration tests require real PostgreSQL and exercise transaction
  conflicts, tenant isolation, migration, and recovery.
- Usage telemetry may remain best effort only where it cannot affect access or
  accounting; budget and settlement writes fail according to an explicit
  dependency policy.
- Database schema and application version skew become release contracts.

## Rejected alternatives

- Continue storing JSONB documents: does not solve concurrency, query, or
  retention boundaries.
- Use Redis as the budget ledger: insufficient as the authoritative auditable
  transaction store.
- Dual-write indefinitely: creates two sources of truth and unbounded recovery
  ambiguity.
