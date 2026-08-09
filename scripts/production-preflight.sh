#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

die() {
  printf '[modelport-production] ERROR: %s\n' "$*" >&2
  exit 1
}

require_file() {
  local label="$1"
  local path="$2"
  [[ -n "$path" ]] || die "$label path is required"
  [[ -f "$path" && ! -L "$path" ]] || die "$label must be a regular non-symlink file"
}

require_digest_image() {
  local label="$1"
  local image="$2"
  [[ "$image" =~ @sha256:[0-9a-f]{64}$ ]] \
    || die "$label must be pinned by an immutable sha256 digest"
}

main() {
  [[ $# -eq 0 ]] || die "this command accepts no arguments"
  command -v python3 >/dev/null 2>&1 || die "python3 is required"
  command -v readlink >/dev/null 2>&1 || die "readlink is required"
  command -v stat >/dev/null 2>&1 || die "stat is required"

  local runtime_env="${MODELPORT_RUNTIME_ENV_FILE:-}"
  local config_file="${MODELPORT_CONFIG_FILE:-}"
  local database_ca="${MODELPORT_DATABASE_CA_FILE:-}"
  local ownership_file="${MODELPORT_OWNERSHIP_FILE:-}"
  local runtime_real mode owner_uid current_uid

  require_file "MODELPORT_RUNTIME_ENV_FILE" "$runtime_env"
  require_file "MODELPORT_CONFIG_FILE" "$config_file"
  require_file "MODELPORT_DATABASE_CA_FILE" "$database_ca"
  require_file "MODELPORT_OWNERSHIP_FILE" "$ownership_file"
  [[ -s "$database_ca" ]] || die "database CA file must not be empty"
  require_digest_image "MODELPORT_IMAGE" "${MODELPORT_IMAGE:-}"
  require_digest_image "MODELPORT_DASHBOARD_IMAGE" "${MODELPORT_DASHBOARD_IMAGE:-}"
  "$ROOT_DIR/scripts/operations-ownership-preflight.sh" "$ownership_file"

  runtime_real="$(readlink -f "$runtime_env")"
  case "$runtime_real" in
    "$ROOT_DIR"|"$ROOT_DIR"/*)
      die "runtime secret env file must be outside the repository"
      ;;
  esac
  mode="$(stat -c '%a' "$runtime_real")"
  [[ "$mode" =~ ^[0-7]{3,4}$ ]] || die "cannot determine runtime secret env permissions"
  (( (8#$mode & 8#077) == 0 )) \
    || die "runtime secret env permissions must not grant group/other access; actual=$mode"
  owner_uid="$(stat -c '%u' "$runtime_real")"
  current_uid="$(id -u)"
  [[ "$owner_uid" == "0" || "$owner_uid" == "$current_uid" ]] \
    || die "runtime secret env must be owned by root or the current operator"

  python3 - "$runtime_real" "$config_file" <<'PY'
import pathlib
import re
import sys
import tomllib
from urllib.parse import parse_qs, urlparse

env_path = pathlib.Path(sys.argv[1])
config_path = pathlib.Path(sys.argv[2])
values: dict[str, str] = {}
for number, raw in enumerate(env_path.read_text(encoding="utf-8").splitlines(), 1):
    line = raw.strip()
    if not line or line.startswith("#"):
        continue
    if line.startswith("export "):
        line = line[7:].lstrip()
    if "=" not in line:
        raise SystemExit(f"runtime env line {number} is not KEY=VALUE")
    key, value = line.split("=", 1)
    key = key.strip()
    value = value.strip()
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key):
        raise SystemExit(f"runtime env line {number} has an invalid key")
    if key in values:
        raise SystemExit(f"runtime env contains duplicate key {key}")
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        value = value[1:-1]
    values[key] = value

required = {
    "MODELPORT_DATABASE_URL",
    "MODELPORT_AUTH_TOKEN",
    "MODELPORT_HEALTHCHECK_API_KEY",
    "MODELPORT_ADMIN_USERNAME",
    "MODELPORT_ADMIN_PASSWORD",
    "MODELPORT_BACKUP_ADMIN_USERNAME",
    "MODELPORT_BACKUP_ADMIN_EMAIL",
    "MODELPORT_BACKUP_ADMIN_PASSWORD",
    "MODELPORT_OIDC_ISSUER",
    "MODELPORT_OIDC_CLIENT_ID",
    "MODELPORT_OIDC_CLIENT_SECRET",
    "MODELPORT_OIDC_REDIRECT_URI",
}
missing = sorted(key for key in required if not values.get(key))
if missing:
    raise SystemExit(f"runtime env is missing required keys: {', '.join(missing)}")
for key in required:
    value = values[key]
    if "placeholder" in value.lower() or value.startswith("replace-with-"):
        raise SystemExit(f"runtime env contains a placeholder for {key}")

database = urlparse(values["MODELPORT_DATABASE_URL"])
if database.scheme not in {"postgres", "postgresql"} or not database.hostname:
    raise SystemExit("MODELPORT_DATABASE_URL must be a PostgreSQL URL with a hostname")
query = parse_qs(database.query)
if query.get("sslmode") != ["verify-full"]:
    raise SystemExit("MODELPORT_DATABASE_URL must set sslmode=verify-full")
if query.get("sslrootcert") != ["/run/modelport/database-ca.pem"]:
    raise SystemExit(
        "MODELPORT_DATABASE_URL must set "
        "sslrootcert=/run/modelport/database-ca.pem"
    )

issuer = urlparse(values["MODELPORT_OIDC_ISSUER"])
redirect = urlparse(values["MODELPORT_OIDC_REDIRECT_URI"])
if issuer.scheme != "https" or not issuer.hostname:
    raise SystemExit("MODELPORT_OIDC_ISSUER must be an absolute HTTPS URL")
if redirect.scheme != "https" or not redirect.hostname:
    raise SystemExit("MODELPORT_OIDC_REDIRECT_URI must be an absolute HTTPS URL")
if redirect.path != "/admin/auth/oidc/callback" or redirect.query or redirect.fragment:
    raise SystemExit(
        "MODELPORT_OIDC_REDIRECT_URI must use /admin/auth/oidc/callback without query or fragment"
    )
if values.get("MODELPORT_OIDC_AUTO_PROVISION", "0") not in {"0", "false", "False"}:
    raise SystemExit("production OIDC auto-provision must remain disabled")

config = tomllib.loads(config_path.read_text(encoding="utf-8"))
credential_envs: set[str] = set()
for provider in config.get("providers", {}).values():
    if not isinstance(provider, dict):
        continue
    for field in ("token_env", "api_key_env"):
        name = provider.get(field)
        if isinstance(name, str) and name:
            credential_envs.add(name)
missing_provider = sorted(name for name in credential_envs if not values.get(name))
if missing_provider:
    raise SystemExit(
        "runtime env is missing Provider credential references: "
        + ", ".join(missing_provider)
    )

print(
    "[modelport-production] preflight passed: "
    f"required_keys={len(required)} provider_credential_refs={len(credential_envs)}"
)
PY
}

main "$@"
