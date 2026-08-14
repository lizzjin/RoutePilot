# Getting Started

This is the shortest Tier 1 path from a published Small-Team Beta to a governed
request. It uses prebuilt Linux x86_64 images, Docker Compose, PostgreSQL, the
Dashboard, and the DeepSeek example; it does not compile source. Use
[Providers](PROVIDERS.md) for a local runtime or another hosted Provider.

## 1. Prerequisites

- Linux x86_64 and Git.
- Docker Engine or Docker Desktop with Compose v2.
- A Provider account and API key.
- Free local ports `33002` and `38082`.

ModelPort stores all runtime state in PostgreSQL. The Compose stack supplies it;
you do not need to install PostgreSQL on the host.

## 2. Create Local Configuration

```bash
git clone --branch v0.1.0 --depth 1 https://github.com/tiammomo/ModelPort.git
cd ModelPort
cp deploy/docker/modelport.env.example .env
cp config.example.toml config.toml
```

Both copies are required. Compose mounts `.env` and `config.toml` read-only into
the backend container.

Edit `.env` and replace every required `replace-with-...` value:

```env
MODELPORT_AUTH_TOKEN=<long-random-router-token>
MODELPORT_ADMIN_USERNAME=admin
MODELPORT_ADMIN_PASSWORD=<different-strong-admin-password>
MODELPORT_POSTGRES_PASSWORD=<long-url-safe-database-password>

DEEPSEEK_ANTHROPIC_AUTH_TOKEN=<real-provider-key>
ANTHROPIC_AUTH_TOKEN=<same-value-as-MODELPORT_AUTH_TOKEN>
```

Do not commit `.env` or `config.toml`. Provider credentials remain in ModelPort;
client applications receive a ModelPort token or a scoped client API key.

The sample model is `deepseek-v4-flash`. If the Provider account exposes a
different ID, update `DEEPSEEK_MODEL`, the `config.toml` model list/default, and
the request examples together.

## 3. Check The Setup

Run the read-only Linux and Compose preflight before pulling images:

```bash
export MODELPORT_COMPOSE_FILE="$PWD/deploy/release/compose.yml"
scripts/doctor.sh --setup
```

Fix the first `[fail]` and rerun it. This mode does not start services or make a
Provider request.

## 4. Pull And Start

```bash
export MODELPORT_COMPOSE_FILE="$PWD/deploy/release/compose.yml"
docker compose -f "$MODELPORT_COMPOSE_FILE" config --quiet
docker compose -f "$MODELPORT_COMPOSE_FILE" pull
scripts/compose-up.sh
docker compose -f "$MODELPORT_COMPOSE_FILE" ps
```

The images are published by the tagged release. Initial evaluation may use the
version tag in Compose; shared/production deployments must use the two main
image digests (plus the optional Agent digest when enabled) listed in the
GitHub Release. Verify checksums, signatures,
attestations, and SBOMs through
[Upgrading and Rollback](UPGRADING.md#release-inputs).

Do not run this release path until the `v0.1.0` tag and required GHCR images
actually exist. A repository change cannot publish them. Before the first
Release, current-main testing is a contributor path:

```bash
git clone https://github.com/tiammomo/ModelPort.git
cd ModelPort
cp deploy/docker/modelport.env.example .env
cp config.example.toml config.toml
# replace required placeholders
export MODELPORT_COMPOSE_FILE="$PWD/docker-compose.yml"
scripts/build-container.sh
MODELPORT_LOCAL_BUILD=1 scripts/compose-up.sh
```

Expected services:

| Service | Expected state |
| --- | --- |
| `postgres` | healthy |
| `modelport` | healthy |
| `dashboard` | running |

The optional operations Agent is not started by the default profile. After the
gateway is configured, follow [Operations Agent](OPS_AGENT.md) to create its
dedicated, non-inference service account. Starting the optional container and
enabling it in **运维事件** are separate confirmations; both default off. Local
models are recommended first for optional diagnosis. Roll out `shadow` before
`read_only` mode.

The backend's Compose healthcheck uses authenticated `/readyz`, so `healthy`
means required persistence is usable. The image-level `/livez` check proves only
that the process answers. Dashboard has no backend startup dependency: its
static page remains available and API routes return 502 when the backend is
unavailable.

If a service does not start:

```bash
docker compose -f "$MODELPORT_COMPOSE_FILE" logs --tail=100 postgres
docker compose -f "$MODELPORT_COMPOSE_FILE" logs --tail=100 modelport
docker compose -f "$MODELPORT_COMPOSE_FILE" logs --tail=100 dashboard
```

## 5. Verify The Gateway

```bash
scripts/smoke-test.sh
```

This checks process liveness, authenticated storage readiness, and the model
catalog without generating model output.

Open `http://127.0.0.1:33002` and sign in with the configured administrator
username and password. The backend API remains at
`http://127.0.0.1:38082`.

## 6. Authorize The First Governed Request

ModelPort fails closed for cloud egress when a project has no policy. Before
calling the DeepSeek example, open **Governance (治理与变更审批)** in the
Dashboard, choose `project_policy.upsert`, and use:

- Target: `org_local/prj_default/env_default`
- Reason: a concrete explanation such as `Allow the documented public synthetic DeepSeek test`
- Payload:

```json
{
  "organizationId": "org_local",
  "projectId": "prj_default",
  "environmentId": "env_default",
  "maximumMode": "cloud_first",
  "defaultClassification": "unknown",
  "allowedProviders": ["deepseek"],
  "allowedModels": ["deepseek-v4-flash"],
  "allowedRegions": ["global"],
  "allowedApiVersions": ["anthropic-v1"],
  "cloudEnabled": true
}
```

Submit the recorded change. In default Small-Team mode, choose
**Direct apply (直接应用)**; the write still requires CSRF protection and is
audited. Enterprise mode or `MODELPORT_REQUIRE_DUAL_APPROVAL=1` requires a
different administrator to approve the change before **Apply change (应用变更)**
becomes available.

This example authorizes only the exact DeepSeek Provider, model, global region,
and Anthropic v1 path for `org_local/prj_default/env_default`. Its default
classification stays `unknown`, so unclassified or sensitive input remains
`local_strict`. Do not paste private source code or other sensitive data into
the public synthetic test.

## 7. Send The First Request

This request can consume Provider quota:

```bash
source .env

curl -fsS \
  -H "x-api-key: $MODELPORT_AUTH_TOKEN" \
  -H 'content-type: application/json' \
  -H 'x-modelport-data-classification: public' \
  -H 'x-modelport-hybrid-mode: cloud_first' \
  http://127.0.0.1:38082/v1/messages \
  -d '{
    "model":"deepseek-v4-flash",
    "max_tokens":96,
    "messages":[{"role":"user","content":"Reply exactly: OK"}]
  }'
```

Or run the synthetic upstream smoke path:

```bash
scripts/smoke-test.sh --upstream
```

Open **Request Logs** in the dashboard and confirm the selected Provider/model,
status, latency, token provenance, and estimated cost. A Provider-returned usage
value is still not an authoritative invoice.

## 8. Connect A Client

Claude Code or another Anthropic-compatible client:

```env
ANTHROPIC_BASE_URL=http://127.0.0.1:38082
ANTHROPIC_AUTH_TOKEN=<MODELPORT_AUTH_TOKEN>
ANTHROPIC_MODEL=deepseek-v4-flash
```

OpenAI-compatible SDK:

```env
OPENAI_BASE_URL=http://127.0.0.1:38082/v1
OPENAI_API_KEY=<MODELPORT_AUTH_TOKEN>
OPENAI_MODEL=deepseek-v4-flash
```

For shared use, create a real user and a scoped client API key in the dashboard,
then set `MODELPORT_REQUIRE_CONTROL_API_KEYS=1` during production hardening.
Never give a client the upstream Provider key.

## 9. Stop, Restart, Or Upgrade

```bash
docker compose -f "$MODELPORT_COMPOSE_FILE" stop
docker compose -f "$MODELPORT_COMPOSE_FILE" start
docker compose -f "$MODELPORT_COMPOSE_FILE" logs -f modelport
docker compose -f "$MODELPORT_COMPOSE_FILE" down
```

`docker compose -f "$MODELPORT_COMPOSE_FILE" down` preserves named volumes. Do
not add `-v` unless permanent database deletion is intentional and a verified
backup exists.

Before an upgrade, follow [Upgrading and Rollback](UPGRADING.md),
[Operations: Backup And Restore](OPERATIONS.md#backup-and-restore), and
[Production](PRODUCTION.md).

## Common First-Run Problems

| Symptom | Resolution |
| --- | --- |
| Compose reports missing `config.toml` | Copy `config.example.toml` to `config.toml`. |
| Startup rejects a placeholder | Replace every required `replace-with-...` value in `.env`. |
| Dashboard opens but login fails | Use the admin username/password, not the router token. |
| Dashboard opens but API calls return 502 | Static Nginx is healthy but the backend is absent/unreachable; check `/livez`, `/readyz`, and backend logs. |
| `/v1/*` returns 401 | Send `x-api-key: <MODELPORT_AUTH_TOKEN>` or `Authorization: Bearer <key>`. |
| Model is not listed | Align the Provider's configured model ID with the account/runtime catalog. |
| Local runtime is unreachable from Docker | Use `host.docker.internal`, not container loopback. |
| Stream starts with HTTP 200 then fails | Inspect the SSE `event: error` and matching request log. |
| Port is already allocated | Change `MODELPORT_API_PUBLISH` or `MODELPORT_DASHBOARD_PUBLISH` in `.env`. |

Continue with [Configuration](CONFIGURATION.md), [Providers](PROVIDERS.md),
[Deployment](DEPLOYMENT.md), or [Operations](OPERATIONS.md) only when your task
requires the additional detail.
