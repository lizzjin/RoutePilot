# Production Readiness

This checklist defines the supported commercial self-hosting baseline. It is
not a certification, warranty, availability SLA, or substitute for an
operator's own threat model and compliance review.

## Supported Topology

- One trusted backend host or a small trusted network.
- PostgreSQL is mandatory for every runtime state path.
- A same-origin HTTPS reverse proxy fronts the dashboard and API.
- Provider credentials remain server-side.
- Dashboard-issued scoped API keys are required for shared use.

Public multi-tenant isolation, active-active multi-region operation,
distributed sessions/rate limits, SCIM, and a maintainer-hosted service are not
part of the current support claim.

## Go-Live Gate

- [ ] Pin a released image digest or verified binary attestation.
- [ ] Use a new PostgreSQL database for the current operational baseline.
- [ ] Configure PostgreSQL TLS `verify-full` for remote/production databases.
- [ ] Set strong unique admin, router, database, and Provider credentials.
- [ ] Set `MODELPORT_ENTERPRISE_MODE=1`; startup then enforces database
      `verify-full`, secure cookies, HTTPS allowed origins, exact trusted proxy
      CIDRs, control API keys, and enabled CSRF protection.
- [ ] Set secure cookies, exact public origin, and exact trusted proxy CIDRs.
- [ ] Set `MODELPORT_REQUIRE_CONTROL_API_KEYS=1`.
- [ ] Keep backend and PostgreSQL ports private.
- [ ] Run `config validate`, `/readyz`, smoke, and acceptance checks.
- [ ] Verify backup creation, restoration drill, encryption, and retention.
- [ ] Define request/audit retention and personal-data deletion procedures.
- [ ] Alert on readiness, rejection phases, request failures, ledger
      finalization/reconciliation, PostgreSQL saturation, Provider cooldown,
      TTFT, full latency, and budget exhaustion.
- [ ] Record the deployed version, commit, image digest, configuration revision,
      and migration set.

## Reliability Objectives

ModelPort does not publish a universal SLO because Provider availability and
local inference capacity dominate end-to-end results. Operators should define:

- data-plane availability and error-budget targets;
- Tool Use workflow success;
- TTFT and full-lifecycle P50/P95 by workload class;
- maximum ledger finalization lag and unreconciled lease count;
- recovery time and recovery point objectives;
- maximum accepted Provider billing reconciliation variance.

Synthetic traffic must use the `synthetic` traffic class and must not be mixed
with business cost/success dashboards.

## Operational Ownership

The operator owns capacity planning, upgrades, incident response, Provider
contracts, legal/privacy requirements, backups, key rotation, database
maintenance, and user support. Community maintenance is best effort unless a
separate written agreement supplies an SLA.

## Release Evidence

Keep these artifacts for each deployed release:

- checksums, SBOM, provenance attestation, and immutable image digest;
- CI, dependency, CodeQL, and migration results;
- configuration validation and production acceptance output;
- dated Provider evidence when a Provider-specific claim matters;
- database backup and restore-drill result;
- rollback decision and incident contact list.
