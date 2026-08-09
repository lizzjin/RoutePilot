# Observability Alert Runbook

This runbook corresponds to
[`deploy/observability/prometheus/modelport-alerts.yml`](../deploy/observability/prometheus/modelport-alerts.yml).
It covers the supported single-instance Small-Team Beta. Prometheus and Grafana
are optional operator-owned dependencies; the default Compose stack does not
install them.

## First Response

1. Record the UTC start time, release version, image digest, affected logical
   models, and a small set of `request_id` values.
2. Check process liveness separately from authenticated readiness:

   ```bash
   curl -fsS http://127.0.0.1:38082/livez
   curl -fsS -H "Authorization: Bearer $MODELPORT_HEALTHCHECK_API_KEY" \
     http://127.0.0.1:38082/readyz
   ```

3. Inspect bounded service logs and PostgreSQL/Provider status. Do not paste a
   complete environment, Provider response body, prompt, response, tool input,
   API key, database URL, or backup into an issue or alert channel.
4. Stop sending new traffic before an operation that can invalidate evidence.
   Do not delete ledger rows, disable database constraints, or bill an
   `unreconciled` request to silence an alert.

`/livez` means only that the HTTP process answers. `/readyz` verifies required
persistence and recent ledger operations but does not require every Provider to
be available. A Provider failure can therefore coexist with successful
readiness.

## ModelPortMetricsTargetDown

1. Request `/livez` directly from the ModelPort host. If it succeeds, inspect
   Prometheus network reachability, DNS, the scoped metrics key, and whether
   `/metrics` returns 401.
2. If liveness fails, run
   `docker compose -f "${MODELPORT_COMPOSE_FILE:-docker-compose.yml}" ps` and
   inspect the last 100 backend log lines. Distinguish startup configuration
   rejection from a crash or host failure.
3. Check host memory, disk, container restarts, and PostgreSQL reachability.
4. Restore service with the same image digest and configuration first. Follow
   [Upgrading and Rollback](UPGRADING.md) before changing versions.

The static Dashboard may remain available while the backend is down. In that
state its proxied API routes intentionally return HTTP 502; this does not mean
the Nginx container itself is unhealthy.

## ModelPortNotReady

1. Confirm `/livez` succeeds and run authenticated `/readyz` directly. A 401
   means the probe key is absent, expired, revoked, or outside the allowed
   authentication policy; it is not a database incident.
2. Read the safe readiness category and backend logs. Check PostgreSQL DNS,
   TCP/TLS, certificate hostname, migrations, database permissions, and pool
   acquisition timeout.
3. Inspect `modelport_ledger_operation_degraded` and
   `modelport_ledger_operation_failures_total` by operation.
4. Keep traffic stopped until readiness returns. A Provider test does not repair
   persistence readiness.

`modelport_gateway_ready` is computed during the authenticated metrics scrape.
The optional Blackbox Exporter probe independently confirms that `/readyz` can
be reached with a scoped credential; if `probe_success` is absent, only the
in-process readiness gauge is available.

## ModelPortDirtyBuildRunning

1. Compare `modelport_build_info` with the intended release and deployment
   record.
2. Replace a `dirty` or `unknown` build with the signed release image pinned by
   digest. Preserve the old digest and database backup until acceptance passes.
3. If the build is an intentional development instance, route this alert away
   from production paging instead of weakening production provenance checks.

## ModelPortLedgerOperationDegraded

1. Identify the `operation` label and inspect its failure counter.
2. For finalization or lease operations, stop new inference traffic and check
   PostgreSQL availability before restarting. An immediate restart can turn a
   recoverable in-process finalizer into an expired lease.
3. Check `modelport_ledger_pending_finalizers` and the request log for
   non-terminal or `unreconciled` evidence.
4. Allow the next successful operation to clear the degraded gauge, then verify
   authenticated readiness and run `scripts/smoke-test.sh`.
5. If the operation remains degraded, preserve logs and a database-native
   backup, then follow the database incident path in [Operations](OPERATIONS.md#common-incidents).

`modelport_database_ready` reports the direct readiness query.
`modelport_database_pool_utilization_ratio` and the bounded connection-state
gauges report application-pool pressure. Acquire latency is not yet exported;
use database-native wait/session metrics as the second source of evidence.

## ModelPortDatabasePoolSaturated

1. Compare `in_use`, `idle`, and the configured maximum; do not raise the pool
   limit before checking PostgreSQL `max_connections` and other clients.
2. Inspect slow ledger queries, stuck finalizers, database locks, and storage
   latency. A larger pool can amplify rather than solve database contention.
3. Reduce admission or long-running operational queries first, then change the
   pool limit only with measured headroom and a rollback value.

## ModelPortPendingFinalizersStuck

1. Compare the finalizer count with active traffic and stream completion. A
   brief non-zero value is normal while terminal evidence commits.
2. If the value persists, inspect database latency, finalization failures, and
   stream clients that stopped reading without closing.
3. Stop new requests. Send SIGTERM through Compose and allow the configured
   `stop_grace_period`; do not use `docker kill` unless the incident requires
   accepting unreconciled outcomes.
4. After restart, inspect reconciliation counters and every affected request's
   terminal reason. Never invent cost for a request whose Provider outcome is
   unknown.

## ModelPortExpiredLeasesReconciled

1. Correlate the increase with deploys, host suspension, process crashes,
   PostgreSQL outages, and scheduler stalls.
2. Locate request rows with `lease_expired_unreconciled` and preserve Provider
   evidence needed for later append-only settlement.
3. Verify the lease TTL exceeds the longest expected scheduler pause and that
   the reconciliation interval remains below the TTL.
4. Treat repeated increases without a planned restart as a reliability
   incident even when current readiness is green.

## ModelPortInferenceErrorRateHigh

1. Split failures by Provider, model, stream flag, and traffic class in the
   Grafana dashboard.
2. Compare rejection metrics with completed message failures. Authentication,
   policy, admission, and concurrency rejections can occur before a message
   series exists.
3. Inspect a bounded sample of request IDs. Separate client validation, policy,
   gateway storage, transport, upstream HTTP, and Tool Use protocol categories.
4. Disable or remove a failing Provider from the affected logical route only
   when the configured local/cloud policy permits the alternative. Never
   silently enable cloud fallback during an incident.

## ModelPortInferenceP95LatencyHigh

1. Compare end-to-end latency with Provider generation latency, queue symptoms,
   streaming duration, model size, and client backpressure.
2. Check Provider-specific cumulative mean:

   ```promql
   sum by (provider) (rate(modelport_message_duration_ms_total[10m]))
   /
   clamp_min(sum by (provider) (rate(modelport_message_requests_total[10m])), 0.001)
   ```

3. Review slow request IDs and stream idle behavior before changing timeouts.
   A long, healthy generation is different from stalled first-byte latency.
4. The current histogram is global. Do not claim a Provider-specific p95 until
   a bounded Provider-labelled histogram is shipped.

## ModelPortAdmissionOrStreamPressure

1. Use the `phase` and `reason` labels to separate context/output admission from
   concurrent-stream exhaustion.
2. For `concurrency`, find abandoned or slow-reading stream clients and compare
   the configured stream permit count with measured memory and Provider
   capacity.
3. For `admission`, verify model context/output limits and exact token-counting
   capability. Do not bypass a context safety limit merely to clear the alert.
4. Inspect `modelport_local_scheduler_{running,interactive_queued,batch_queued,
   users_queued,estimated_service_ms,oldest_interactive_wait_ms,
   oldest_batch_wait_ms}` and `modelport_stream_permits_available`. Available
   permits do not by themselves reveal configured capacity; correlate zero
   permits with concurrency rejections before declaring exhaustion.

## ModelPortQuotaRejectionsDetected

1. Identify the affected user, API key, team/project, and UTC or rolling spend
   window in the authenticated Dashboard. Never expose those identities as
   Prometheus labels.
2. Confirm whether cost is estimated or Provider-reported and whether the
   budget mode is hard or informational.
3. Increase a limit only through the normal reviewed governance path. Do not
   mutate historical requests or budget events.
4. This alert covers the bounded pre-ledger `quota_exceeded` metric path only.
   v0.1.x does not expose remaining budget as a bounded Prometheus gauge; the
   relational budget view remains authoritative for remaining amounts.

## ModelPortProviderErrorRateHigh

1. Confirm the Provider has at least ten completed requests in the alert
   window; low-volume one-off failures should be triaged without changing
   routing policy.
2. Run the safe Provider connection/model test, then a paid synthetic request
   only when authorized. Check credentials, account balance, entitlement,
   region, protocol, Tool Use, and rate limits separately.
3. Compare the failing Provider against the logical model policy. Disable a
   route or credential rather than allowing an unapproved cloud destination.
4. Compare completed traffic with `modelport_provider_available` and
   `modelport_provider_cooldown`. Availability means a usable credential and no
   active cooldown; it does not prove model entitlement, Tool Use, balance, or a
   successful live generation.

## Recovery Closeout

Close an incident only after liveness, authenticated readiness, smoke tests,
the relevant Provider/Tool Use acceptance path, ledger finalizers, and a sample
of governed request evidence all pass. Record the release digest, configuration
revision, database state, action timeline, and any `unreconciled` request IDs.
Turn permanent findings into tests, rules, or documentation without adding
content-bearing telemetry.
