#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${MODELPORT_COMPOSE_FILE:-$ROOT_DIR/docker-compose.yml}"

usage() {
  cat <<'USAGE'
Usage: scripts/database-preflight.sh

Read-only check that the running Compose PostgreSQL major version and data
volume match docker-compose.yml. It never prints credentials or expands the
Compose environment. A mismatch is a hard stop before a full Compose update.
USAGE
}

die() {
  printf '[modelport-database] ERROR: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required"
}

declared_postgres_image() {
  awk '
    /^  postgres:[[:space:]]*$/ { in_postgres = 1; next }
    in_postgres && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ { exit }
    in_postgres && $1 == "image:" { print $2; exit }
  ' "$COMPOSE_FILE"
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

image_major() {
  printf '%s\n' "$1" | sed -nE 's#^postgres:([0-9]+)([.-].*)?$#\1#p'
}

main() {
  if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
  fi
  [[ $# -eq 0 ]] || die "this command accepts no arguments"

  require_command awk
  require_command docker
  require_command sed
  [[ -f "$COMPOSE_FILE" ]] || die "Compose file not found: $COMPOSE_FILE"

  local container_id configured_image configured_major running_image
  local running_major version_number volume_name volume_destination
  local configured_volume failed_migrations state_rows

  configured_image="$(declared_postgres_image)"
  configured_volume="$(declared_postgres_volume)"
  [[ -n "$configured_image" ]] || die "cannot determine the declared PostgreSQL image"
  [[ -n "$configured_volume" ]] || die "cannot determine the declared PostgreSQL volume"
  configured_major="$(image_major "$configured_image")"
  [[ -n "$configured_major" ]] \
    || die "declared PostgreSQL image is not a pinned major tag: $configured_image"

  container_id="$(docker compose -f "$COMPOSE_FILE" ps -q postgres)"
  [[ -n "$container_id" ]] \
    || die "Compose PostgreSQL is not running; inspect volumes before starting a replacement"

  running_image="$(docker inspect "$container_id" --format '{{.Config.Image}}')"
  version_number="$(docker compose -f "$COMPOSE_FILE" exec -T postgres sh -c \
    'exec psql --username="$POSTGRES_USER" --dbname="$POSTGRES_DB" --tuples-only --no-align --command="show server_version_num"')"
  [[ "$version_number" =~ ^[0-9]+$ ]] || die "cannot determine the running PostgreSQL version"
  running_major="$((version_number / 10000))"

  IFS='|' read -r volume_name volume_destination < <(
    docker inspect "$container_id" --format \
      '{{range .Mounts}}{{if eq .Type "volume"}}{{.Name}}|{{.Destination}}{{println}}{{end}}{{end}}' \
      | awk 'NF { print; exit }'
  )
  [[ -n "$volume_name" && -n "$volume_destination" ]] \
    || die "cannot determine the running PostgreSQL data volume"

  failed_migrations="$(docker compose -f "$COMPOSE_FILE" exec -T postgres sh -c \
    'exec psql --username="$POSTGRES_USER" --dbname="$POSTGRES_DB" --tuples-only --no-align --command="select count(*) from _sqlx_migrations where not success"')"
  state_rows="$(docker compose -f "$COMPOSE_FILE" exec -T postgres sh -c \
    'exec psql --username="$POSTGRES_USER" --dbname="$POSTGRES_DB" --tuples-only --no-align --command="select count(*) from modelport_state"')"

  printf '[modelport-database] running image=%s server_major=%s volume=%s destination=%s\n' \
    "$running_image" "$running_major" "$volume_name" "$volume_destination"
  printf '[modelport-database] declared image=%s server_major=%s volume=%s\n' \
    "$configured_image" "$configured_major" "$configured_volume"

  if [[ "$running_major" != "$configured_major" ]]; then
    die "PostgreSQL major-version drift detected; do not run full 'docker compose up'. Follow docs/POSTGRESQL_MIGRATION.md"
  fi
  if [[ "$volume_name" != "$configured_volume" && "$volume_name" != *"_$configured_volume" ]]; then
    die "PostgreSQL data-volume drift detected; do not run full 'docker compose up'. Follow docs/POSTGRESQL_MIGRATION.md"
  fi
  [[ "$failed_migrations" == "0" ]] || die "$failed_migrations SQLx migration(s) are marked failed"
  [[ "$state_rows" =~ ^[0-9]+$ && "$state_rows" -ge 2 ]] \
    || die "durable auth/control state rows are incomplete"

  printf '[modelport-database] alignment and migration preflight passed\n'
}

main "$@"
