# ModelPort Observability Pack

This directory is the official minimum monitoring contract for the
single-instance Small-Team Beta. Prometheus and Grafana remain operator-owned
dependencies and are not added to the default Compose stack.

## Included Files

- `prometheus/modelport-alerts.yml`: executable alert rules for process,
  readiness, persistence, request lifecycle, Provider, stream-admission,
  latency, quota-rejection, and reconciliation signals.
- `prometheus/prometheus.example.yml`: authenticated metrics scrape plus an
  optional authenticated Blackbox Exporter readiness probe.
- `prometheus/blackbox.example.yml`: `/readyz` probe module.
- `grafana/modelport-overview.json`: importable dashboard for the same metrics.
- [`docs/OBSERVABILITY_RUNBOOK.md`](../../docs/OBSERVABILITY_RUNBOOK.md): alert
  meaning, triage, and recovery checks.

Copy the files into the operator's Prometheus/Grafana deployment. Store a
dedicated scoped ModelPort key in the referenced secret files; do not place it
in Git, the rule file, a dashboard variable, or an alert annotation. Prometheus
can authenticate to `/metrics` with `Authorization: Bearer`; Blackbox Exporter
uses its own secret file for `/readyz`.

Run `promtool check rules prometheus/modelport-alerts.yml` before reloading
Prometheus, then import the dashboard and select the Prometheus data source.

## Honest Coverage Boundary

The pack uses only metrics emitted by the current release. The following
important signals are not yet emitted and therefore have no pretend alert:

| Desired signal | Available proxy | Missing dependency |
| --- | --- | --- |
| PostgreSQL acquire latency and waiter pressure | pool open/idle/in-use/max/utilization gauges plus the saturation alert and Grafana panel | acquire-latency histogram and waiter gauge |
| Active downstream streams | available stream permits plus concurrency rejections | configured permit-capacity/active-stream gauges |
| Budget remaining and exhaustion forecast | quota-rejection count and estimated-cost counter | remaining/reserved budget gauges by bounded project scope |
| Reconciled Provider invoice cost | estimated-cost counter | append-only settlement/reconciliation metric |
| Per-Provider p95 latency | global latency histogram plus per-Provider cumulative mean | bounded Provider-labelled histogram |

Provider availability/cooldown, PostgreSQL pool utilization, and local scheduler
running/queue/wait gauges are exported and consumed directly by the included
rules and dashboard. Queue wait is currently a gauge, not a distribution; use
it to find sustained pressure, not to claim a historical queue percentile.

`modelport_message_cost_estimate_usd_total` is an estimate, not an invoice.
Dashboard budget values and PostgreSQL evidence remain the source for operator
review until bounded budget metrics are implemented. Alert thresholds are a
safe starting point for a 20–50 person team and must be calibrated against
measured workload and Provider latency.
