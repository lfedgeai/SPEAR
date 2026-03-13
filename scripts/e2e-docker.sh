#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

OS="$(uname -s)"
E2E_CLEANUP="${E2E_CLEANUP:-1}"
KEEP_DOCKER_RESOURCES="${KEEP_DOCKER_RESOURCES:-0}"

NET_NAME="${NET_NAME:-spear-e2e-net}"
SMS_CONTAINER_NAME="${SMS_CONTAINER_NAME:-spear-e2e-sms}"
SPEARLET_CONTAINER_NAME="${SPEARLET_CONTAINER_NAME:-spear-e2e-spearlet}"

cleanup() {
  set +e
  if [[ "$E2E_CLEANUP" == "1" && "$KEEP_DOCKER_RESOURCES" != "1" ]]; then
    docker rm -f "$SMS_CONTAINER_NAME" "$SPEARLET_CONTAINER_NAME" >/dev/null 2>&1 || true
    docker network rm "$NET_NAME" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if [[ "$OS" == "Linux" ]]; then
  if ! docker info >/dev/null 2>&1; then
    echo "docker daemon not reachable" >&2
    exit 1
  fi
  docker network inspect "$NET_NAME" >/dev/null 2>&1 || docker network create "$NET_NAME" >/dev/null
  docker rm -f "$SMS_CONTAINER_NAME" "$SPEARLET_CONTAINER_NAME" >/dev/null 2>&1 || true
  cargo build
  DOCKER=1 cargo test --test testcontainers_e2e -- --ignored --nocapture
  exit 0
fi

if [[ "${E2E_LINUX:-0}" == "1" ]]; then
  make e2e-linux
  exit 0
fi

echo "skip docker e2e on non-Linux host (set E2E_LINUX=1 to run via e2e-linux)" >&2
