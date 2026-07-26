## Summary

-

## Validation

- [ ] `scripts/check-all.sh`
- [ ] `cd dashboard && npm run e2e` when dashboard behavior changes
- [ ] `scripts/doctor.sh` when runtime or deployment behavior changes
- [ ] `scripts/doctor.sh --upstream` when provider behavior changes
- [ ] Docker Compose render/build when deployment files change

## Risk

- [ ] No real API keys, tokens, logs, or `.env` content are included
- [ ] Auth, routing, streaming, and provider compatibility impact has been considered
- [ ] User-visible changes and breaking migrations are documented in `CHANGELOG.md`
- [ ] New dependencies and generated artifacts have acceptable licenses
- [ ] Backup, rollback, and data-retention impact has been considered
