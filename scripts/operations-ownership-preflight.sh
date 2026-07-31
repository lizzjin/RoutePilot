#!/usr/bin/env bash
set -euo pipefail

die() {
  printf '[modelport-ownership] ERROR: %s\n' "$*" >&2
  exit 1
}

main() {
  [[ $# -eq 1 ]] || die "usage: $0 OWNERSHIP.toml"
  local ownership_file="$1"
  [[ -f "$ownership_file" && ! -L "$ownership_file" ]] \
    || die "ownership file must be a regular non-symlink TOML file"
  command -v python3 >/dev/null 2>&1 || die "python3 is required"

  python3 - "$ownership_file" <<'PY'
import pathlib
import sys
import tomllib

path = pathlib.Path(sys.argv[1])
document = tomllib.loads(path.read_text(encoding="utf-8"))
if document.get("schema_version") != 1:
    raise SystemExit("ownership schema_version must be 1")

platform = document.get("platform", {})
owner = str(platform.get("owner", "")).strip()
backups = [str(value).strip() for value in platform.get("backups", [])]
if not owner or "replace" in owner.lower() or "todo" in owner.lower():
    raise SystemExit("a named platform Owner is required")
if not backups or any(not value for value in backups):
    raise SystemExit("at least one named platform Backup is required")
if owner in backups:
    raise SystemExit("platform Owner and Backup must be different people")

coverage = document.get("coverage", {})
required_coverage = (
    "timezone",
    "business_hours",
    "primary_channel",
    "backup_channel",
)
missing = [key for key in required_coverage if not str(coverage.get(key, "")).strip()]
if missing:
    raise SystemExit("ownership coverage is missing: " + ", ".join(missing))
ack = coverage.get("acknowledgement_minutes")
if not isinstance(ack, int) or not 1 <= ack <= 60:
    raise SystemExit("acknowledgement_minutes must be an integer from 1 to 60")

escalation = document.get("escalation", {})
required_escalation = ("database", "identity", "security", "cloud_billing")
missing = [key for key in required_escalation if not str(escalation.get(key, "")).strip()]
if missing:
    raise SystemExit("ownership escalation is missing: " + ", ".join(missing))

print(
    "[modelport-ownership] preflight passed: "
    f"owner={owner} backups={len(backups)} timezone={coverage['timezone']} "
    f"acknowledgement_minutes={ack}"
)
PY
}

main "$@"
