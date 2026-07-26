# Changelog

All notable ModelPort changes are recorded here. The project follows
[Semantic Versioning](https://semver.org/) once a version is published.

## [Unreleased]

### Added

- Relational PostgreSQL request, Provider-attempt, usage, quota/spend, budget,
  management-statistics, and append-only audit sources.
- Complete request identity, client, traffic, Tool Use, pricing provenance,
  retry/fallback, latency, and TTFT dimensions.
- Authenticated operational views, build identity, Provider evidence output,
  and rejection metrics.
- Opt-in smart-routing groups with policy/capability gates, quality/balanced/
  economy/latency profiles, shadow decisions, stable canary activation,
  session affinity, metrics, and relational decision evidence.
- Commercial open-source governance, support, privacy, release, and supply
  chain policies.

### Changed

- PostgreSQL is mandatory for every runtime deployment.
- The default Compose and CI database is PostgreSQL 18.4, using the PostgreSQL
  18 versioned data directory and a new `modelport-postgres-18` named volume.
- The dashboard runtime uses the current Nginx 1.30.4 stable security release.
- Dashboard, logs, quotas, audit, and management statistics use relational
  operational rows instead of process estimates or control-document arrays.
- Request-log SQL keeps enterprise pagination and operational time-window
  parameter contracts distinct, and minute-precision end times include the
  complete selected minute so current failures remain visible.
- The public model catalog advertises Provider-qualified IDs and explicit
  aliases.

### Removed

- Runtime JSON-file and process-memory persistence fallbacks.
- Automatic import of old JSON state.
- Old usage/activity/spend arrays and legacy management response aliases.
- The no-PostgreSQL Compose override.

### Security

- Configuration fails before binding when PostgreSQL is missing.
- Operational audit records are append-only and durable error details remain
  category-only.

### Upgrade notice

Migration `0005_current_operational_schema.sql` now preserves existing
normalized request/attempt rows, backfills conservative operational defaults,
and derives request-level Provider/retry snapshots from historical attempts.
Back up PostgreSQL and run a restore drill before upgrading. Compose still uses
the PostgreSQL 18 volume `modelport_modelport-postgres-18`; export any older
volume before removing it.

[Unreleased]: https://github.com/tiammomo/ModelPort/commits/main
