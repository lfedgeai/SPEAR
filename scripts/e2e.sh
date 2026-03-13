#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OS="$(uname -s)"

E2E_SUITES="${E2E_SUITES:-auto}"

if [[ "$E2E_SUITES" == "auto" ]]; then
  if [[ "$OS" == "Linux" ]]; then
    E2E_SUITES="docker,kind"
  else
    E2E_SUITES="kind"
  fi
fi

echo "e2e suites: ${E2E_SUITES} (os=${OS})"
if [[ "$OS" != "Linux" && "$E2E_SUITES" == *"docker"* && "${E2E_LINUX:-0}" != "1" ]]; then
  echo "note: docker suite is skipped on non-Linux hosts (set E2E_LINUX=1 to run via e2e-linux)" >&2
fi

IFS=',' read -r -a suites <<<"$E2E_SUITES"

for suite in "${suites[@]}"; do
  suite="$(echo "$suite" | tr -d '[:space:]')"
  if [[ -z "$suite" ]]; then
    continue
  fi

  case "$suite" in
    kind)
      bash scripts/e2e-kind.sh
      ;;
    docker)
      bash scripts/e2e-docker.sh
      ;;
    *)
      echo "unknown suite: $suite" >&2
      exit 1
      ;;
  esac
done
