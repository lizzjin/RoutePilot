#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STACK_DIR="${LOCAL_INFERENCE_STACK_DIR:-}"
MODELPORT_CONFIG_PATH=""
RELEASE=0
JSON=0

usage() {
  cat <<'USAGE'
Usage:
  scripts/local-inference-check.sh --stack-dir <path> [options]

Options:
  --stack-dir <path>       local-inference-stack checkout. May also be supplied
                           through LOCAL_INFERENCE_STACK_DIR.
  --config <path>          ModelPort config, relative to this checkout unless
                           absolute (default: config.toml).
  --release                Also check clean worktrees and the pinned gateway
                           source revision.
  --json                   Emit machine-readable JSON.
  -h, --help               Show this help.

This command is read-only. It does not start services, download models, send an
inference request, or change GPU state.
USAGE
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --stack-dir)
      [[ "$#" -ge 2 ]] || { printf '%s\n' '--stack-dir requires a path' >&2; exit 2; }
      STACK_DIR="$2"
      shift 2
      ;;
    --config)
      [[ "$#" -ge 2 ]] || { printf '%s\n' '--config requires a path' >&2; exit 2; }
      MODELPORT_CONFIG_PATH="$2"
      shift 2
      ;;
    --release)
      RELEASE=1
      shift
      ;;
    --json)
      JSON=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
  printf '%s\n' 'local inference integration checks require Linux or WSL2.' >&2
  exit 2
fi
if [[ -z "$STACK_DIR" ]]; then
  printf '%s\n' \
    'set --stack-dir or LOCAL_INFERENCE_STACK_DIR; adjacent checkout paths are not assumed.' >&2
  exit 2
fi
if ! command -v python3 >/dev/null 2>&1; then
  printf '%s\n' 'python3 3.11 or newer is required.' >&2
  exit 2
fi

STACK_DIR="$(cd "$STACK_DIR" 2>/dev/null && pwd)" || {
  printf 'local-inference-stack directory not found: %s\n' "$STACK_DIR" >&2
  exit 2
}
CHECKER="$STACK_DIR/scripts/compatibility-check.py"
CONTRACT="$STACK_DIR/contracts/local-qwen-provider-v1.json"
if [[ ! -f "$CHECKER" || ! -f "$CONTRACT" ]]; then
  printf 'not a compatible local-inference-stack checkout: %s\n' "$STACK_DIR" >&2
  exit 2
fi

arguments=(
  "$CHECKER"
  --modelport-project "$ROOT_DIR"
  --contract "$CONTRACT"
)
if [[ -n "$MODELPORT_CONFIG_PATH" ]]; then
  arguments+=(--modelport-config "$MODELPORT_CONFIG_PATH")
fi
if [[ "$RELEASE" -eq 1 ]]; then
  arguments+=(--release)
fi
if [[ "$JSON" -eq 1 ]]; then
  arguments+=(--json)
fi

exec python3 "${arguments[@]}"
