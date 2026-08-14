#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${MODELPORT_COMPOSE_FILE:-$ROOT_DIR/docker-compose.yml}"

die() {
  printf '[modelport-compose] ERROR: %s\n' "$*" >&2
  exit 1
}

declared_project_name() {
  awk '$1 == "name:" { print $2; exit }' "$COMPOSE_FILE"
}

declared_postgres_volume() {
  awk '
    /^  postgres:[[:space:]]*$/ { in_postgres = 1; next }
    in_postgres && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ { exit }
    in_postgres && $1 == "-" && $2 ~ /^modelport-postgres/ {
      split($2, parts, ":")
      print parts[1]
      exit
    }
  ' "$COMPOSE_FILE"
}

compose_has_service() {
  docker compose -f "$COMPOSE_FILE" config --services \
    | grep -Fxq "$1"
}

resolved_default_network() {
  docker compose -f "$COMPOSE_FILE" config \
    | awk '
        /^networks:[[:space:]]*$/ { in_networks = 1; next }
        in_networks && /^[^[:space:]]/ { exit }
        in_networks && /^  default:[[:space:]]*$/ { in_default = 1; next }
        in_default && /^  [^[:space:]][^:]*:[[:space:]]*$/ { exit }
        in_default && $1 == "name:" { print $2; exit }
      '
}

ensure_default_network() {
  local network_name
  network_name="$(resolved_default_network)"
  [[ -n "$network_name" ]] || die "cannot determine the declared default network"

  if docker network inspect "$network_name" >/dev/null 2>&1; then
    return
  fi

  docker network create "$network_name" >/dev/null
  printf '[modelport-compose] created external network: %s\n' "$network_name"
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/compose-up.sh [SERVICE ...]

Starts or updates the selected Compose deployment. For a profile with the
bundled PostgreSQL service, a read-only major-version, volume, migration, and
state check must pass before Compose may recreate any service. An external-
database profile skips that local-volume check; run its documented production
preflight first.

Set MODELPORT_COMPOSE_FILE to select a manifest; it defaults to the root
source-build profile.

Set MODELPORT_LOCAL_BUILD=1 after scripts/build-container.sh to use the local
backend, dashboard, and optional operations-agent images without pulling GHCR.
USAGE
  exit 0
fi

if [[ "${MODELPORT_LOCAL_BUILD:-0}" == "1" ]]; then
  local_images=(modelport:local modelport-dashboard:local)
  if [[ ",${COMPOSE_PROFILES:-}," == *,ops-agent,* ]]; then
    local_images+=(modelport-ops-agent:local)
  else
    for requested_service in "$@"; do
      if [[ "$requested_service" == "ops-agent" ]]; then
        local_images+=(modelport-ops-agent:local)
        break
      fi
    done
  fi
  for local_image in "${local_images[@]}"; do
    docker image inspect "$local_image" >/dev/null 2>&1 \
      || die "missing $local_image; run scripts/build-container.sh first"
  done
  export MODELPORT_IMAGE=modelport:local
  export MODELPORT_DASHBOARD_IMAGE=modelport-dashboard:local
  export MODELPORT_OPS_AGENT_IMAGE=modelport-ops-agent:local
  export MODELPORT_PULL_POLICY=never
fi

if compose_has_service postgres; then
  if [[ -n "$(docker compose -f "$COMPOSE_FILE" ps -q postgres)" ]]; then
    "$ROOT_DIR/scripts/database-preflight.sh"
  else
    project_name="$(declared_project_name)"
    target_volume="$(declared_postgres_volume)"
    [[ -n "$project_name" && -n "$target_volume" ]] \
      || die "cannot determine the declared project/PostgreSQL volume"
    expected_volume="${project_name}_${target_volume}"
    legacy_volumes="$(
      docker volume ls --format '{{.Name}}' \
        | awk -v prefix="${project_name}_modelport-postgres" -v expected="$expected_volume" \
            'index($0, prefix) == 1 && $0 != expected { print }'
    )"
    if [[ -n "$legacy_volumes" ]]; then
      printf '[modelport-compose] legacy PostgreSQL volume(s) detected:\n%s\n' \
        "$legacy_volumes" >&2
      die "refusing a stopped-database major-version cutover; follow docs/POSTGRESQL_MIGRATION.md"
    fi
  fi
else
  printf '[modelport-compose] external PostgreSQL profile: local database-volume preflight skipped\n'
fi

ensure_default_network
exec docker compose -f "$COMPOSE_FILE" up -d "$@"
