#!/usr/bin/env bash
set -euo pipefail

# Run local compose E2E with RustFS and credential master-key validation.
# 运行本地 compose E2E，覆盖 RustFS 与 credential master key 验证。

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

COMPOSE_FILE="${COMPOSE_FILE:-deploy/docker/compose.local.yaml}"
SMS_BASE_URL="${SMS_BASE_URL:-http://127.0.0.1:18082}"
E2E_CLEANUP="${E2E_CLEANUP:-1}"
KEEP_DOCKER_RESOURCES="${KEEP_DOCKER_RESOURCES:-0}"
SMS_CREDENTIAL_MASTER_KEY="${SMS_CREDENTIAL_MASTER_KEY:-compose-e2e-master-key}"
export SMS_BASE_URL SMS_CREDENTIAL_MASTER_KEY

cleanup() {
  set +e
  if [[ "$E2E_CLEANUP" == "1" && "$KEEP_DOCKER_RESOURCES" != "1" ]]; then
    docker compose -f "$COMPOSE_FILE" down -v --remove-orphans >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

command -v docker >/dev/null 2>&1 || {
  echo "docker is required / 需要安装 docker" >&2
  exit 1
}

command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required / 需要安装 python3" >&2
  exit 1
}

docker info >/dev/null 2>&1 || {
  echo "docker daemon not reachable / docker daemon 不可用" >&2
  exit 1
}

COMPOSE_PARALLEL_LIMIT="${COMPOSE_PARALLEL_LIMIT:-1}" docker compose -f "$COMPOSE_FILE" up -d --build
bash scripts/compose-rustfs-smoke.sh

python3 - <<'PY'
import json
import sys
import time
import urllib.error
import urllib.request
from os import environ

base = environ["SMS_BASE_URL"]
credential_name = "compose_e2e_openai"


def request(path: str, method: str = "GET", body: bytes | None = None, headers: dict[str, str] | None = None):
    req = urllib.request.Request(f"{base}{path}", data=body, headers=headers or {}, method=method)
    with urllib.request.urlopen(req, timeout=30) as resp:
        return resp.status, json.loads(resp.read())


def wait_ready():
    last_error = None
    for _ in range(60):
        try:
            status, payload = request("/admin/api/object-storage/status")
            if status == 200 and payload.get("healthy"):
                return
        except Exception as error:  # noqa: BLE001
            last_error = error
        time.sleep(1)
    raise RuntimeError(f"sms did not become ready: {last_error}")


wait_ready()
status, upsert = request(
    "/admin/api/ai/credentials",
    method="POST",
    body=json.dumps(
        {
            "name": credential_name,
            "secret": "sk-compose-e2e",
            "description": "compose e2e credential",
            "disabled": False,
        }
    ).encode("utf-8"),
    headers={"content-type": "application/json"},
)
if status != 200 or not upsert.get("success"):
    raise RuntimeError(f"credential upsert failed: {upsert}")

status, listed = request("/admin/api/ai/credentials")
if status != 200 or not listed.get("success"):
    raise RuntimeError(f"credential list failed: {listed}")
if credential_name not in {item["name"] for item in listed.get("credentials", [])}:
    raise RuntimeError(f"credential missing before restart: {listed}")

print(json.dumps({"credential_upsert": upsert, "credential_list_before_restart": listed}, ensure_ascii=False, indent=2))
PY

docker compose -f "$COMPOSE_FILE" restart sms >/dev/null

python3 - <<'PY'
import json
import sys
import time
import urllib.request
from os import environ

base = environ["SMS_BASE_URL"]
credential_name = "compose_e2e_openai"


def request(path: str, method: str = "GET", body: bytes | None = None, headers: dict[str, str] | None = None):
    req = urllib.request.Request(f"{base}{path}", data=body, headers=headers or {}, method=method)
    with urllib.request.urlopen(req, timeout=30) as resp:
        return resp.status, json.loads(resp.read())


def wait_ready():
    last_error = None
    for _ in range(60):
        try:
            status, payload = request("/admin/api/object-storage/status")
            if status == 200 and payload.get("healthy"):
                return
        except Exception as error:  # noqa: BLE001
            last_error = error
        time.sleep(1)
    raise RuntimeError(f"sms did not recover after restart: {last_error}")


wait_ready()
status, listed = request("/admin/api/ai/credentials")
if status != 200 or not listed.get("success"):
    raise RuntimeError(f"credential list after restart failed: {listed}")
if credential_name not in {item["name"] for item in listed.get("credentials", [])}:
    raise RuntimeError(f"credential missing after restart: {listed}")

status, deleted = request(f"/admin/api/ai/credentials/{credential_name}", method="DELETE")
if status != 200 or not deleted.get("success"):
    raise RuntimeError(f"credential delete failed: {deleted}")

status, listed_after_delete = request("/admin/api/ai/credentials")
if status != 200 or not listed_after_delete.get("success"):
    raise RuntimeError(f"credential list after delete failed: {listed_after_delete}")
if credential_name in {item["name"] for item in listed_after_delete.get("credentials", [])}:
    raise RuntimeError(f"credential still present after delete: {listed_after_delete}")

print(
    json.dumps(
        {
            "credential_list_after_restart": listed,
            "credential_delete": deleted,
            "credential_list_after_delete": listed_after_delete,
        },
        ensure_ascii=False,
        indent=2,
    )
)
PY
