# Data lifecycle and safe maintenance

This document defines what ModelPort may prune and what must remain auditable. It applies to the
PostgreSQL runtime; file-backed development state is disposable only when it is not the selected
source of truth.

## Ownership classes

| Data | Policy |
| --- | --- |
| Organizations, projects, environments, providers, routes, users and API-key metadata | Configuration authority; back up and retain while referenced |
| Provider secrets | Keep only in the configured secret store; never export into logs or repository files |
| Gateway requests and provider attempts | Operational evidence; retain according to an explicit environment policy |
| Routing decisions and feedback | Operational/evaluation evidence; retain or archive with the referenced gateway request |
| Budget accounts, reservations and events | Financial/audit ledger; events are append-only and are not ordinary cleanup targets |
| Sessions and transient login/rate-limit state | Expire through the built-in lifecycle, not manual table deletion |
| Acceptance providers/users/teams | Scripts remove control objects on exit; request ledger evidence can remain |

The normalized PostgreSQL budget ledger deliberately uses restrictive foreign keys and an
append-only event trigger. Deleting a gateway row that has attempts, reservations or budget events
is rejected. Do not disable those controls to make a dashboard look clean. If a deployment needs
physical retention, implement an reviewed archive/partition policy that preserves account
reconciliation and evidence export before dropping a partition.

Migration `0005_current_operational_schema.sql` preserves existing normalized
gateway requests and Provider attempts. It backfills conservative identity,
path, traffic, Tool Use, and latency values and derives request/attempt
Provider, retry, and fallback snapshots from existing attempt order before
enforcing the current constraints. It does not delete history or invent token
and monetary values.

Migration `0007_smart_routing_decisions.sql` adds at most one decision row per
gateway request plus optional feedback rows. It stores no prompt, response, or
raw session identifier. Composite foreign keys cascade request retention to its
decision and feedback, so archive and remove them as one retention unit rather
than leaving orphaned evidence.

## Testing without contaminating a shared ledger

`scripts/acceptance.sh` and `scripts/tool-use-acceptance.sh` clean temporary control-plane objects,
but calls sent through a long-running PostgreSQL gateway are real ledger entries. For a zero-residue
test, start an isolated ModelPort instance with its own database and budget accounts, run acceptance,
export the report, then remove the whole isolated database. Never point destructive test cleanup at
the QuantPilot environment.

## Maintenance sequence

1. Run `scripts/status.sh` and `scripts/doctor.sh`.
2. Create and verify a backup with `scripts/backup-compose.sh`; keep it outside build/temp folders.
3. Export the request/usage window needed for audit and capacity analysis.
4. Remove only unreferenced failed request shells or expired transient auth state. Preserve all rows
   reachable from a budget event.
5. Re-run doctor, budget reconciliation checks and dashboard usage totals.

The backup script and restore drill are documented in [Operations](OPERATIONS.md). Configuration and
secret placement are documented in [Configuration](CONFIGURATION.md). The
administrator-only live DeepSeek balance action is a point-in-time Provider
diagnostic; it does not settle the ModelPort ledger. Gateway-observed usage and
configured internal budgets are not a Provider invoice or authoritative
external balance.
