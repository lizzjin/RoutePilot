## Summary

-

## Validation

- [ ] `scripts/check.sh`
- [ ] `cd dashboard && npm run lint`
- [ ] `cd dashboard && npm run build`
- [ ] `cd dashboard && npm run e2e` when dashboard behavior changes
- [ ] `scripts/doctor.sh`
- [ ] `scripts/doctor.sh --upstream` when provider behavior changes
- [ ] Docker build when deployment files change

## Risk

- [ ] No real API keys, tokens, logs, or `.env` content are included
- [ ] Auth, routing, streaming, and provider compatibility impact has been considered
