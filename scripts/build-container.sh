#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib.sh"

allow_dirty=0
if [[ "${1:-}" == "--allow-dirty" ]]; then
  allow_dirty=1
  shift
fi
if [[ "$#" -ne 0 ]]; then
  die "usage: scripts/build-container.sh [--allow-dirty]"
fi

source_revision="$(git -C "$ROOT_DIR" rev-parse HEAD)"
source_state="clean"
modelport_version="$(
  sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n 1
)"
if [[ -z "$modelport_version" ]]; then
  die "could not read package version from Cargo.toml"
fi
build_date="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [[ -n "$(git -C "$ROOT_DIR" status --porcelain=v1)" ]]; then
  source_state="dirty"
  if [[ "$allow_dirty" != "1" ]]; then
    die "refusing to build a release image from a dirty worktree; commit the reviewed changes or use --allow-dirty for local testing"
  fi
fi

log "building ModelPort images version=$modelport_version revision=$source_revision source_state=$source_state"
common_args=(
  --build-arg "MODELPORT_VERSION=$modelport_version"
  --build-arg "MODELPORT_SOURCE_REVISION=$source_revision"
  --build-arg "MODELPORT_SOURCE_STATE=$source_state"
  --build-arg "MODELPORT_BUILD_DATE=$build_date"
)

docker build \
  "${common_args[@]}" \
  --file "$ROOT_DIR/Dockerfile" \
  --tag modelport:local \
  "$ROOT_DIR"
docker build \
  "${common_args[@]}" \
  --file "$ROOT_DIR/dashboard/Dockerfile" \
  --tag modelport-dashboard:local \
  "$ROOT_DIR"

for image in modelport:local modelport-dashboard:local; do
  image_id="$(docker image inspect "$image" --format '{{.Id}}')"
  image_revision="$(docker image inspect "$image" --format '{{index .Config.Labels "org.opencontainers.image.revision"}}')"
  image_state="$(docker image inspect "$image" --format '{{index .Config.Labels "io.modelport.source-state"}}')"
  image_version="$(docker image inspect "$image" --format '{{index .Config.Labels "org.opencontainers.image.version"}}')"

  if [[ "$image_revision" != "$source_revision" || "$image_state" != "$source_state" || "$image_version" != "$modelport_version" ]]; then
    die "$image provenance labels do not match the requested source state"
  fi
  log "built $image id=$image_id version=$image_version revision=$image_revision source_state=$image_state"
done

log "start these source-built images with: MODELPORT_LOCAL_BUILD=1 scripts/compose-up.sh"
