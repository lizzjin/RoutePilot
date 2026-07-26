# Evidence-Led Optimization Plan

Last reviewed: 2026-07-26.

This plan turns one deployment's recent ModelPort evidence into an execution
order. It is deliberately narrower than the enterprise roadmap: observed
correctness and workload bottlenecks take priority over adding protocol or
platform surface area.

## Dated Baseline

The local operational sample covered about 6.8 days. It was read from retained
usage rows, process metrics, and the normalized PostgreSQL ledger; no paid
Provider call was made for this review.

| Signal | Observation | Decision |
| --- | --- | --- |
| Route outcomes | 244 inference route outcomes: 237 successful and 7 failed. Only one of those failures had reached the retained usage lifecycle. | Keep request logs focused on sent/terminal work, but expose bounded pre-ledger rejection counters by phase and reason so the six early failures are explainable. |
| Provider mix | About 95.8% of completed traffic selected `local_qwen`. | Optimize the maintained local Qwen path before broad Provider catalog work. |
| Workload shape | About 92.9% of retained requests declared or continued Tool Use. | Treat Tool Use workflow success, repair rate, TTFT, and full latency as primary service indicators rather than niche compatibility checks. |
| Local stream latency | The sampled local-Qwen/OpenAI stream path had approximately 4.1 s TTFT P95 and 10 s full-lifecycle P95. | Segment performance by traffic class, Tool Use, alias, and context size. Improve the same workload mix before increasing concurrency. |
| Billing provenance | 1,645 normalized request/attempt lifecycle rows were inspected; 729 unambiguous one-attempt rows had request-level Provider usage but an attempt still labelled `local-estimate`. | Start the current schema on a new database and persist Provider usage provenance atomically at request/attempt finalization. Do not import ambiguous historical rows. |
| Previous persistence state | The former control document contained 2,809 retained usage rows and 500 activities and was about 412 KiB. Each write replaced the logical document. | Remove usage, spend, and activity arrays from the control document. PostgreSQL terminal request rows and append-only audit events become the only operational source. |
| Provider claims | No dated real-Provider result was committed in the compatibility matrix. | Generate secret-free, commit-bound evidence artifacts for explicit paid verification. Configuration alone is not verification. |

These numbers are a dated deployment baseline, not a universal product
benchmark or SLO.

## Changes Shipped In This Optimization Slice

- Non-stream Provider attempts now retain `upstream-returned` evidence when the
  adapter parsed Provider usage, including strict Tool Use validation failures
  that carry reported usage.
- The current operational migration intentionally rejects databases containing
  old request/attempt rows. Operators must provision a new database rather than
  receive guessed dimensions or silent historical rewrites.
- Pre-ledger inference failures increment
  `modelport_inference_rejections_total` with bounded `route`, `phase`, and
  `reason` labels. Error bodies, validation paths, credentials, and request
  values never become labels.
- `business`, `synthetic`, and `diagnostic` traffic are distinct in retained
  logs and process message metrics. Main dashboard usage, cost, success, and
  trend calculations use business traffic only. A real synthetic
  upstream failure can still update Provider/credential cooldown state.
- Relational request rows carry the complete identity snapshot, client IP,
  request path, traffic class, Tool Use
  intent/outcome, lifecycle latency, semantic TTFT, repair state, retry count,
  fallback source, final Provider/model/attempt, pricing snapshot, and billing
  provenance.
- Request logs, Dashboard ranges, API-key/team usage, user quotas, spend
  policies, and append-only audit history read the relational ledger only.
  Process metrics no longer backfill historical charts.
- Log requests apply their lower time bound in PostgreSQL, detail reads use the
  request primary key, Dashboard reads only the selected range plus today, and
  management usage uses grouped relational aggregation.
- Request-log summaries calculate full-filter-set latency P95 and semantic TTFT
  P95. The UI displays both alongside workflow success.
- `provider-matrix.sh --evidence <file>` writes a secret-free JSON record tied
  to the commit and clean/dirty source state. Benchmark upstream calls are
  labelled synthetic.

## Next Optimization Loop

### P0 — Prove correctness after rollout

1. Back up the existing deployment, provision a new PostgreSQL database, apply
   all migrations, and verify the old-row guard fails closed against a fixture.
2. Alert on rejection rate by phase. A rise in `validation` is a client
   contract issue; `rate_limit` or `concurrency` is capacity policy;
   `ledger` is a storage/readiness incident.
3. Compare business dashboard totals with business-filtered request logs.
   Synthetic acceptance calls must not change business cost or success cards.
4. Run the local Tool Use acceptance suite and retain its result separately
   from the Provider matrix.

### P1 — Reduce local Qwen Tool Use latency

Use successful business streams only and keep model, context-size bucket,
Tool Use state, output cap, and concurrency constant between samples.

1. Route short tool-selection turns to the non-thinking `qwen3.5-fast` alias.
   Keep `qwen3.5-code`/`qwen3.5-deep` for turns that need reasoning; do not make
   thinking the default for the physical model.
2. Bound tool descriptions and conversation history before tuning generation.
   Prompt ingestion often dominates local TTFT, while larger output limits
   mostly affect full latency.
3. Run one change at a time against at least 30 representative Tool Use
   workflows. Record TTFT P50/P95, full P50/P95, tool workflow success,
   `answered_without_tool`, strict-repair rate, tokens, and runtime queue depth.
4. Accept an experiment only when TTFT or full-lifecycle P95 improves by at
   least 20% without reducing Tool Use workflow success by more than one
   percentage point. This is an experiment gate, not a published SLO.
5. Tune runtime slots, batch size, KV cache, and GPU offload only with the
   companion runtime's queue/GPU evidence. Raising ModelPort concurrency while
   the runtime is saturated increases tail latency.

### P1 — Validate and operate the current relational baseline

1. Measure PostgreSQL plans for the indexed time/API-key/team/Provider queries
   at expected production cardinality.
2. Define an explicit database retention and archival policy; never reintroduce
   an in-document row cap.
3. Alert on ledger finalization, quota aggregation, and audit append failures.
4. Add cursor-based audit export and server-side SQL aggregation for unusually
   large ad-hoc log ranges if production evidence shows the current bounded
   window queries are insufficient.

### P1 — Establish Provider evidence

Run paid verification only in an explicit budget-capped workflow:

```bash
scripts/provider-matrix.sh \
  --models provider:model-a,provider:model-b \
  --evidence artifacts/provider-matrix.json
scripts/tool-use-acceptance.sh --upstream
```

Commit or retain the secret-free artifacts with the tested release. Tool Use
evidence is independent from non-stream/stream text evidence.

### P2 — Defer breadth until demanded by traffic

OpenAI Responses, SCIM, multi-instance HA, advanced routing, and a larger
Provider catalog remain valid roadmap items. They should not displace the
local-Qwen Tool Use, persistence, accounting, and verification work above
unless a concrete client, compliance requirement, or availability target
changes the workload evidence.

## Release Evidence

An optimization release is complete when it has:

- Rust, dashboard, configuration, migration, and shell-syntax checks;
- an isolated clean PostgreSQL migration plus an old-row rejection fixture;
- before/after business-only latency and Tool Use workflow statistics;
- rejection counters covering every pre-ledger return phase;
- zero request content, Provider bodies, keys, or validation paths in metrics
  and generated evidence;
- a dated rollback point and database backup for production rollout.
