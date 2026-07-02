# ModelPort Dashboard

The dashboard is the web control plane for ModelPort. It is built with React, TypeScript, Vite, Tailwind CSS, shadcn-style local UI primitives, TanStack Query/Table/Virtual, Recharts, and Playwright.

It is designed for personal and small-team operations:

- Dashboard overview: requests, success rate, tokens, latency, cost estimates, provider breakdown, model distribution, token trends, recent usage, and quick actions.
- Request logs: provider/channel, identity, model, cache/token details, cost, latency, retry, network, and raw context fields.
- API keys: create/edit/delete, restore/disable, user/team binding, IP restrictions, spend limits, rate windows, and model/provider policy.
- Users: role, status, email, and password management.
- Quotas, model/provider configuration, provider lifecycle management, aliases, setup diagnostics, runtime checks, and backup/export entry points.

## Development

Install dependencies:

```bash
npm install
```

Start the dashboard dev server:

```bash
npm run dev
```

Default development URL:

```text
http://127.0.0.1:5173
```

The dev server proxies dashboard API requests to the backend when configured through Vite. In split deployments, set:

```bash
VITE_API_BASE_URL=http://127.0.0.1:17878
```

For mock-only UI development:

```bash
VITE_MODELPORT_MOCK=1 npm run dev
```

Do not use mock mode in production builds.

## Backend Requirements

The real dashboard expects ModelPort to be running and reachable:

```bash
cd ..
scripts/start.sh
```

Login uses the account credentials from `.env`:

```env
MODELPORT_ADMIN_USERNAME=admin
MODELPORT_ADMIN_PASSWORD=replace-with-a-long-random-admin-password
```

The router token (`MODELPORT_AUTH_TOKEN`) is for `/v1` clients and `/metrics`; it is not the normal dashboard login credential.

## Checks

Run before committing dashboard changes:

```bash
npm run lint
npm run build
```

Run browser E2E tests:

```bash
npm run e2e
```

On machines where Playwright Chromium cannot find system libraries, use the local dependency path:

```bash
LD_LIBRARY_PATH=../.modelport/playwright-deps/root/usr/lib/x86_64-linux-gnu npm run e2e
```

The E2E suite covers dashboard trend filters, model catalog visibility, DeepSeek standard model availability, provider lifecycle/model inventory workflows, and admin user/API-key workflows.

## UI Guidelines

- Keep operational pages dense but calm; ModelPort is a control plane, not a marketing site.
- Prefer real controls over explanatory text: segmented controls for ranges, selects for filters, toggles for booleans, icon buttons for compact commands.
- Avoid nested cards and decorative noise.
- Ensure long model names, token counts, costs, and request IDs cannot break card/table layout.
- Keep mobile usable for login and quick checks, but optimize dashboards and logs for desktop operations.

## Build Output

Production build:

```bash
npm run build
```

The Docker dashboard image serves the generated static assets through Nginx and proxies `/admin`, `/v1`, `/livez`, `/readyz`, `/health`, and `/metrics` to the backend.
