# Smart Routing

ModelPort implements an opt-in, policy-aware router for logical model aliases.
It adds adaptive candidate selection without changing the contract for explicit
`provider:model`, static aliases, exact models, or prefix routing.

## Safety Contract

- `off` is the default. Smart aliases are not advertised, and a direct request
  using one stays on the first configured eligible candidate. This is the
  routing kill switch without turning an established alias into an outage.
- `shadow` scores every eligible candidate but sends the request through the
  first configured eligible candidate. This measures disagreements without
  changing production routing.
- `active` applies the scored order only to the configured percentage of
  requests. The remainder stays on the configured-order control route and is
  recorded as `canary_control`.
- An explicit Provider/model request remains deterministic. A routing-profile
  header never turns it into a smart route.
- Candidate policy and capability checks run before scoring. Attempt-scoped
  quota, budget, credential, transport, and Provider checks still run before
  every actual egress.

## Configuration

Smart groups are defined only in TOML. Environment variables can switch the
mode, default profile, and activation percentage, but cannot create candidates.

```toml
[routing]
mode = "shadow"
default_profile = "balanced"
policy_version = "builtin-v1"
activation_percent = 0

[routing.groups.general]
aliases = ["auto", "auto-general"]
default_profile = "balanced"

[[routing.groups.general.candidates]]
provider = "local_qwen"
model = "qwen3.5-9b-q5km"
quality = 0.72
latency_hint_ms = 1200
enabled = true

[[routing.groups.general.candidates]]
provider = "deepseek"
model = "deepseek-v4-flash"
quality = 0.88
latency_hint_ms = 1800
enabled = true
```

Candidate `quality` is a bounded prior from `0` to `1`, not a measured claim.
`latency_hint_ms` must be between `1` and `600000`. Every candidate model must
be accepted by its Provider inventory/prefix/passthrough policy. Alias names
must be unique and cannot conflict with static aliases. A configuration may
contain at most 128 groups and 256 candidates per group.

The restart/reload-time overrides are:

```env
MODELPORT_SMART_ROUTING_MODE=shadow
MODELPORT_SMART_ROUTING_PROFILE=balanced
MODELPORT_SMART_ROUTING_ACTIVATION_PERCENT=0
```

## Request Contract

Smart aliases work on both inference edges. Clients may optionally send:

```http
X-ModelPort-Routing-Profile: quality
X-ModelPort-Session-Id: application-session-reference
```

Profiles are `quality`, `balanced`, `economy`, and `latency`. The session value
is limited to 128 ASCII non-control bytes. ModelPort hashes it together with
the authenticated principal before routing; raw session values are neither
logged nor stored. Reusing it keeps affinity and canary assignment stable for
that principal. Without it, active-mode bucketing uses a principal-scoped
request ID. Callers that need stable retry assignment should reuse their bounded
request ID; otherwise assignment is intentionally per request and does not
derive from prompt content.

## Eligibility And Scoring

The router first removes disabled, missing, capability-incompatible,
policy-ineligible, and cooling candidates. If every otherwise eligible
candidate is cooling, it keeps them as a last-resort plan so a stale cooldown
cannot make the route permanently unavailable.

The built-in score combines:

| Profile | quality | estimated cost | latency | reliability |
| --- | ---: | ---: | ---: | ---: |
| `quality` | 0.60 | 0.10 | 0.15 | 0.15 |
| `balanced` | 0.35 | 0.30 | 0.20 | 0.15 |
| `economy` | 0.15 | 0.70 | 0.05 | 0.10 |
| `latency` | 0.20 | 0.10 | 0.60 | 0.10 |

Quality is the configured prior. Estimated cost uses the request output limit,
a bounded input approximation, and configured Provider pricing. Latency starts
from the candidate hint and blends toward an in-process EWMA as the sample
count grows, so one fast or slow observation cannot immediately replace the
prior. Reliability uses a conservative prior plus current Provider-health
outcomes. A small deterministic affinity term breaks close scores for a
supplied session.

## Evidence And Operations

Every admitted request receives a `rtd_*` decision ID. PostgreSQL migration
`0007_smart_routing_decisions.sql` stores the selected and recommended route,
mode, profile, policy version, candidate count, selected/recommended scores,
bounded reason codes,
session-affinity flag, and shadow-disagreement flag. It does not store request
content or a raw session identifier. Decision rows follow the parent request's
retention lifecycle, so removing an expired request also removes its decision
and attached feedback instead of leaving orphaned storage.

Administrators can inspect current process state through
`GET /admin/router/status`. Prometheus exposes
`modelport_routing_decisions_total{mode,profile,provider}`. Request logs include
the decision ID and selected/recommended route in structured backend logs.

Recommended rollout:

1. Configure candidate pricing, quality priors, latency hints, and policy.
2. Start with `shadow` and `activation_percent=0`.
3. Compare shadow disagreement, errors, latency, cost, Tool Use success, and
   Provider cooldown by profile for at least one representative traffic cycle.
4. Switch to `active` at a small percentage such as `5`, then increase through
   explicit reviewed changes.
5. Set `MODELPORT_SMART_ROUTING_MODE=shadow` or `off` and reload/restart to stop
   adaptive selection. `off` keeps direct smart-alias calls on their configured
   baseline order; explicit routes remain available throughout.

## Current Boundary

Runtime latency EWMA, route counters, and data-plane Provider-health outcomes
used by reliability scoring are process-local; they are not a distributed
semantic-quality model. Configured prices and input-token estimates are
planning inputs, not an upstream invoice. PostgreSQL includes a
routing-feedback evidence table, but this release has no public
feedback-ingestion or automatic weight-training endpoint. A future learning
service should consume versioned, privacy-reviewed offline evaluations and
publish a signed policy snapshot instead of mutating weights in the request
path.
