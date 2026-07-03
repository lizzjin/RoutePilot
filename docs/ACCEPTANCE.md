# Production Acceptance

This checklist is for the main ModelPort audience: personal use and small teams running a lightweight self-hosted gateway.

## Run The Script

Start ModelPort first, then run:

```bash
scripts/acceptance.sh
```

Default mode verifies the control plane and safety policies without making a real upstream model call.

Tool Use has a dedicated acceptance script:

```bash
scripts/tool-use-acceptance.sh
```

Default Tool Use acceptance starts a temporary local OpenAI-compatible mock upstream, creates a temporary local provider through the dashboard API, validates non-streaming Tool Use, streaming `input_json_delta`, `tool_result` continuation, and malformed Tool Use rejection, then cleans the provider up. It does not consume upstream quota.

To include one real `/v1/messages` call through the created API key:

```bash
scripts/acceptance.sh --upstream
```

To certify the configured real upstream provider's Tool Use behavior:

```bash
scripts/tool-use-acceptance.sh --upstream
```

Before a release or push that touches the dashboard, pricing, routing, auth, quotas, or request logs, also run the code checks:

```bash
cargo fmt --all -- --check
CC_x86_64_unknown_linux_gnu=./tools/zig-cc-wrapper.sh cargo test --all-targets
cd dashboard
npm run lint
npm run build
LD_LIBRARY_PATH=../.modelport/playwright-deps/root/usr/lib/x86_64-linux-gnu npm run e2e
```

The `LD_LIBRARY_PATH` line is only needed on machines where Playwright's Chromium cannot find system libraries such as `libnspr4.so`.

## What It Checks

The script verifies:

- `/livez` is reachable.
- Authenticated `/readyz` returns detailed readiness.
- Dashboard URL is reachable when `MODELPORT_DASHBOARD_URL` points to it.
- Admin login works.
- Authenticated `/v1/models` works.
- A temporary user can be created.
- A temporary team/project can be created.
- A temporary API key can be created and bound to that team/project.
- API key IP restriction rejects a disallowed client IP.
- API key spend limit rejects an over-limit request before upstream routing.
- Audit events are recorded.
- Full local backup export and validation work.
- Temporary user, team/project, and key are cleaned up.
- Dedicated Tool Use acceptance can validate the protocol adapter with a local mock provider.

`--upstream` additionally verifies:

- The same temporary API key can make a successful real model request when IP policy allows it.
- `scripts/tool-use-acceptance.sh --upstream` verifies that the configured provider can return a real Tool Use response.

## Environment

The script reads `.env` through the shared script loader. Required values:

```env
MODELPORT_BIND=127.0.0.1:17878
MODELPORT_AUTH_TOKEN=...
MODELPORT_ADMIN_USERNAME=admin
MODELPORT_ADMIN_PASSWORD=...
```

Optional:

```env
MODELPORT_DASHBOARD_URL=http://127.0.0.1:5173
MODELPORT_TOOL_USE_MOCK_HOST=host.docker.internal
```

The script requires `curl` and `node`. `node` is already part of the dashboard toolchain.

`MODELPORT_TOOL_USE_MOCK_HOST` is optional. The Tool Use script automatically uses `host.docker.internal` when Docker Compose is running and `127.0.0.1` otherwise.

## Docker Compose

For Docker Compose:

```bash
cp deploy/docker/modelport.env.example .env
nano .env
docker compose up -d --build
MODELPORT_DASHBOARD_URL=http://127.0.0.1:5173 scripts/acceptance.sh
```

If you published the dashboard or API on different host ports, adjust `MODELPORT_BIND` and `MODELPORT_DASHBOARD_URL` in `.env` or in the command environment.

## Interpreting Results

Passing default acceptance means ModelPort is ready for personal or small-team trial production.

Passing `--upstream` means the full path is working:

```text
Claude-compatible client -> ModelPort auth/policy -> provider route -> upstream model
```

If default acceptance passes but `--upstream` fails, investigate provider credentials, model names, base URL, or upstream quota.

## Dashboard Acceptance

The dashboard E2E suite currently covers:

- Admin login through the account-based dashboard auth flow.
- Dashboard range filters: last 1 day, 3 days, 7 days, and custom range.
- No `Invalid Date` labels in trend charts.
- Public model catalog visibility for active configured providers.
- DeepSeek standard model visibility when the provider is usable.
- Provider lifecycle and model inventory workflows: create, disable/restore, change default model, enable/disable a model, and delete.
- System settings configuration reload, including the returned restart-required scope.
- User and API key create/edit/cleanup flows.

Manual visual checks before a user-facing release should include:

- `/login`: desktop and mobile layout, button/input sizing, and readable color contrast.
- `/dashboard`: KPI cards, request trend, provider breakdown, model distribution, token trend, recent usage, and no clipped long numbers.
- `/logs`: filter panel, virtualized table, token/cost columns, retry/network details, and horizontal overflow behavior.

## Pricing Acceptance

Cost display is an estimate, but the estimate must be internally consistent:

- `src/pricing.rs` has regression tests for provider-specific cache input/output pricing.
- Request logs expose `modelPricing`, `costBreakdown`, token totals, and cache hit rate.
- Dashboard summaries recalculate historical records with token details from the current pricing table.
- Legacy records without token details keep their stored estimate so old quota tests and imports remain compatible.
