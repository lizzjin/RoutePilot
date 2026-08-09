# Deployment

ModelPort supports two maintained single-host deployment paths. Start with
Docker Compose unless host integration requires systemd.

## Choose A Path

| Path | Use it when | Guide |
| --- | --- | --- |
| Docker Compose | You want the backend, dashboard, and PostgreSQL as one reproducible stack. | [Docker Compose](DOCKER.md) |
| systemd | PostgreSQL and the reverse proxy are already operated on the host. | [systemd](SYSTEMD.md) |

The [Getting Started guide](GETTING_STARTED.md) is the canonical first-run
sequence. Deployment-specific documents assume that you already understand the
Provider and client-key boundary.

## Supported Topology

The current supported free Small-Team Beta self-hosting boundary is:

```text
clients
   |
same-origin HTTPS reverse proxy
   |--------------------|
dashboard            ModelPort
                         |
                     PostgreSQL
                         |
             hosted or local Providers
```

An optional CPA instance belongs beside other internal Providers, never in
front of ModelPort:

```text
clients -> HTTPS -> ModelPort -> private CPA -> Codex/Claude accounts
                            \-> other Providers
```

- For systemd, bind CPA to `127.0.0.1:8317`.
- For containers, attach CPA to ModelPort's private network under a
  single-label service name such as `cpa`; do not publish `8317`.
- Keep CPA OAuth/auth files in a separate access-controlled persistent path.
- Disable CPA remote management unless a separately authenticated
  administrative path is required.

- Keep the backend and PostgreSQL ports private.
- Terminate HTTPS at a trusted same-origin proxy.
- Store Provider credentials only in the server environment or secret input.
- Give applications dashboard-issued, scoped ModelPort client API keys.
- Operate one backend instance on one trusted host or small trusted network.

Multi-instance rate limits, sessions, stream permits, and complete Provider
health coordination are not implemented. Do not represent the current topology
as active-active or public multi-tenant.

## Before Shared Use

1. Complete the [Production checklist](PRODUCTION.md).
2. Set `MODELPORT_ENTERPRISE_MODE=1` and resolve every startup failure rather
   than disabling its guardrails.
3. Use PostgreSQL TLS `verify-full` for a remote production database.
4. Configure exact HTTPS origins and trusted proxy CIDRs.
5. Require dashboard-issued client API keys.
6. Create and restore-drill an encrypted, access-controlled backup.
7. Define retention, monitoring, incident ownership, and upgrade rollback.

## Configuration Ownership

- [Configuration](CONFIGURATION.md) is the authority for environment variables,
  TOML fields, precedence, validation, and reload behavior.
- [Providers](PROVIDERS.md) owns Provider/runtime compatibility and evidence.
- [Operations](OPERATIONS.md) owns health, logs, metrics, backup, retention,
  troubleshooting, and upgrades.
- [Production](PRODUCTION.md) owns go-live and release evidence.

Do not duplicate full environment files in deployment documentation. Start from
the maintained examples under `deploy/` and keep secrets outside version
control.
