# Performance And Efficiency

ModelPort is designed for single-host personal and small-team traffic. The
gateway adds one local HTTP hop, authentication/policy work, JSON or SSE
conversion, metrics, and a synchronous control-state write per completed
request. Upstream queueing and generation will often dominate latency, but the
repository does not claim a universal throughput or latency target without a
dated benchmark.

## Work Per Request

- Axum/Tokio handles ingress asynchronously with a process-wide concurrency
  limit.
- reqwest/rustls reuses upstream connections and disables redirects.
- Non-stream responses are bounded and parsed before response mapping.
- Normal streams are converted frame by frame with idle and total-byte limits.
  The general request timeout covers only their response-header handshake; an
  established stream has no fixed wall-clock lifetime while chunks continue
  arriving within the idle and byte limits.
  A separate process-local stream semaphore is acquired before the upstream
  attempt and remains attached to the downstream response body until completion
  or drop. This prevents the handler-return boundary from hiding long-lived
  stream occupancy; immediate exhaustion returns 429 rather than queueing.
  A provider configured with `buffer_stream_text` intentionally waits for a
  complete non-stream generation and conversion, captures reported usage, and
  only then emits local SSE. This removes upstream generation from the
  post-header phase but makes time to first byte equal to full upstream
  generation plus conversion.
- Authentication, model routing, rate policy, quota checks, credential health,
  pricing estimates, metrics, and usage records add local work.
- Auth and low-frequency control mutations replace logical JSON documents.
  Request usage and audit activity are not stored in those documents, so normal
  inference traffic does not amplify them.
- Gateway-request, Provider-attempt, budget, usage, and audit writes use the
  async pooled row-oriented PostgreSQL ledger.
- Log queries apply the lower time bound in PostgreSQL and retrieve details by
  primary key. The selected rows are then filtered and summarized before
  pagination.
- Dashboard trend aggregation reads only the selected window plus the current
  UTC day. Custom windows remain bounded to 90 days.

## Benchmark

Local endpoints, default 30 iterations:

```bash
scripts/bench.sh
```

Real upstream, default 3 paid calls:

```bash
scripts/bench.sh --upstream
scripts/bench.sh --upstream -n 5
```

Upstream benchmark calls carry `x-modelport-traffic-class: synthetic`, so they
remain available in request logs without changing business dashboard totals.

Record at least:

- date, commit, build profile, CPU/RAM and OS;
- storage backend and retained usage-record count;
- provider/model, stream mode, context/output size;
- local endpoint p50/p95 and end-to-end client latency;
- first content delta and complete generation separately;
- failure/SSE-error count, not only initial HTTP status.

`/livez` measures HTTP/process overhead. `/v1/models` adds authentication and
catalog generation. `/v1/messages` is not a gateway-only benchmark because it
includes provider behavior and a persistence write.

## Metrics

```bash
curl -sS \
  -H "x-api-key: $MODELPORT_AUTH_TOKEN" \
  http://127.0.0.1:38082/metrics
```

Current process-local series:

- `modelport_uptime_seconds`
- `modelport_route_{requests,successes,failures,duration_ms}_total`
- `modelport_inference_rejections_total`
- `modelport_message_{requests,successes,failures,duration_ms}_total`
- `modelport_message_{input,output,cache_write,cache_read}_tokens_total`
- `modelport_message_cost_estimate_usd_total`

Message metrics are labelled by provider, model, traffic class, and stream.
Arbitrary model passthrough can create high cardinality; bounded overflow
series preserve business/synthetic/diagnostic separation. Pre-ledger rejection
metrics use fixed phase and error-category labels. Metrics reset on restart.

For streams, request logs are finalized when the downstream body completes,
fails, times out, or is dropped. `firstByteLatencyMs` is recorded only for a
stream's first deliverable text delta or tool-call event; non-stream requests
leave it null instead of substituting full-response latency. Final tokens/cost
still fall back to a request estimate when the Provider does not emit usage, so
do not treat local estimates as invoices. Request logs expose `billingMode` to
distinguish Provider-returned usage from a local estimate. Attempt-level
preflight rows record zero usage; earlier ingress failures may return before
persistence and are counted by the bounded rejection metric. Neither updates
quota/spend, though both still incur local validation/metrics work. Filtered log
summaries expose lifecycle P95 and semantic TTFT P95 over the full result set,
not only the current page.

## Tuning

```env
MODELPORT_MAX_CONCURRENT_REQUESTS=64
MODELPORT_MAX_CONCURRENT_STREAMS=64
MODELPORT_HTTP_CONNECT_TIMEOUT_SECS=10
MODELPORT_HTTP_REQUEST_TIMEOUT_SECS=600
MODELPORT_HTTP_STREAM_IDLE_TIMEOUT_SECS=300
MODELPORT_HTTP_MAX_RESPONSE_BYTES=33554432
MODELPORT_HTTP_SSE_MAX_LINE_BYTES=1048576
MODELPORT_HTTP_SSE_MAX_EVENT_BYTES=8388608
MODELPORT_HTTP_SSE_MAX_STREAM_BYTES=67108864
```

- Lower concurrency before raising it when provider rate limits or storage
  latency are the bottleneck.
- `MODELPORT_MAX_CONCURRENT_STREAMS` defaults to the effective general
  concurrency cap. Size it for simultaneously open bodies, not request-start
  throughput: slow readers hold permits until completion/drop. A 429 includes
  `Retry-After: 1`, and raising the cap increases open sockets and upstream
  work.
- Keep request/response/SSE limits finite; larger values increase memory and
  connection exposure.
- Do not use `MODELPORT_HTTP_REQUEST_TIMEOUT_SECS` as a live-generation cap. It
  covers the full non-stream exchange but only the SSE handshake; tune the
  stream idle and byte limits for the established phase.
- Apply explicit PostgreSQL retention/partitioning only when evidence shows it
  is required.
- Prefer PostgreSQL for durable operational state; low-frequency control
  document writes remain synchronous.
- Diagnose provider/network latency before changing gateway timeouts.
- Measure Tool Use and large-context workloads separately from tiny text calls.

Multi-instance rate limiting and exact invoice reconciliation are not
implemented. PostgreSQL usage/audit rows, tenant-budget reservation, and
in-process live-stream terminal accounting are implemented, but user/key/team
preflight limits and process-loss evidence remain explicit boundaries. These
are architectural triggers, not settings that can be tuned away.
