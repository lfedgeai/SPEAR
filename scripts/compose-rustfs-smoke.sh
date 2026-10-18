#!/usr/bin/env bash
set -euo pipefail

# Validate the local Docker Compose RustFS object-storage path.
# 验证本地 Docker Compose 的 RustFS 对象存储链路。

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

COMPOSE_FILE="${COMPOSE_FILE:-deploy/docker/compose.local.yaml}"
SMS_BASE_URL="${SMS_BASE_URL:-http://127.0.0.1:18082}"
RUSTFS_BASE_URL="${RUSTFS_BASE_URL:-http://127.0.0.1:19000}"
export SMS_BASE_URL RUSTFS_BASE_URL

command -v docker >/dev/null 2>&1 || {
  echo "docker is required / 需要安装 docker" >&2
  exit 1
}

command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required / 需要安装 python3" >&2
  exit 1
}

# Fail early if the local compose stack is not already up.
# 如果本地 compose 栈尚未启动，则提前失败。
running_services="$(docker compose -f "$COMPOSE_FILE" ps --services --status running)"
if [[ -z "$running_services" ]]; then
  echo "compose stack is not running; start it first with docker compose up -d --build / compose 栈未运行，请先执行 docker compose up -d --build" >&2
  exit 1
fi

python3 - <<'PY'
import json
import sys
import urllib.error
import urllib.request
from os import environ

sms_base = environ["SMS_BASE_URL"]
rustfs_base = environ["RUSTFS_BASE_URL"]


def request(
    url: str,
    method: str = "GET",
    body: bytes | None = None,
    headers: dict[str, str] | None = None,
    allow_http_error: bool = False,
):
    req = urllib.request.Request(url, data=body, headers=headers or {}, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.status, resp.read(), dict(resp.headers)
    except urllib.error.HTTPError as error:
        if allow_http_error:
            return error.code, error.read(), dict(error.headers)
        raise


def request_json(path: str, method: str = "GET", body: bytes | None = None, headers: dict[str, str] | None = None):
    status, payload, response_headers = request(f"{sms_base}{path}", method=method, body=body, headers=headers)
    return status, json.loads(payload), response_headers


try:
    status_code, status_payload, _ = request_json("/admin/api/object-storage/status")
    if status_code != 200:
        raise AssertionError(f"unexpected object-storage status code: {status_code}")
    if status_payload.get("backend") != "rustfs-blobstore":
        raise AssertionError(f"unexpected backend: {status_payload}")
    if not status_payload.get("healthy"):
        raise AssertionError(f"unhealthy object storage: {status_payload}")

    rustfs_status, _, _ = request(rustfs_base, allow_http_error=True)
    if rustfs_status not in (200, 403):
        raise AssertionError(f"unexpected RustFS HTTP status: {rustfs_status}")

    upload_status, upload_payload, _ = request_json(
        "/admin/api/objects",
        method="POST",
        body=b"hello rustfs smoke from script",
        headers={
            "content-type": "text/plain",
            "x-file-name": "rustfs-smoke.txt",
        },
    )
    if upload_status != 200:
        raise AssertionError(f"unexpected upload status: {upload_status}")

    key = upload_payload["key"]
    _, meta_payload, _ = request_json(f"/admin/api/objects/{key}")
    _, list_payload, _ = request_json(f"/admin/api/objects?prefix={key}&limit=10")
    content_status, content_body, content_headers = request(f"{sms_base}/admin/api/objects/{key}/content")
    content_headers = {key.lower(): value for key, value in content_headers.items()}
    _, retain_payload, _ = request_json(f"/admin/api/objects/{key}/retain", method="POST")
    _, pin_payload, _ = request_json(f"/admin/api/objects/{key}/pin", method="POST")
    _, delete_conflict_payload, _ = request_json(f"/admin/api/objects/{key}", method="DELETE")
    _, unpin_payload, _ = request_json(f"/admin/api/objects/{key}/unpin", method="POST")
    _, release_payload, _ = request_json(f"/admin/api/objects/{key}/release", method="POST")
    _, delete_payload, _ = request_json(f"/admin/api/objects/{key}", method="DELETE")
    _, meta_after_delete_payload, _ = request_json(f"/admin/api/objects/{key}")

    if not meta_payload.get("found"):
        raise AssertionError(f"metadata not found: {meta_payload}")
    if meta_payload.get("size") != len(b"hello rustfs smoke from script"):
        raise AssertionError(f"unexpected object size: {meta_payload}")
    if list_payload.get("total_count", 0) < 1:
        raise AssertionError(f"object not listed: {list_payload}")
    if content_status != 200:
        raise AssertionError(f"unexpected content status: {content_status}")
    if content_body.decode("utf-8") != "hello rustfs smoke from script":
        raise AssertionError("downloaded content mismatch")
    if content_headers.get("content-type") != "text/plain":
        raise AssertionError(f"unexpected content type: {content_headers.get('content-type')}")
    if retain_payload.get("ref_count") != 1:
        raise AssertionError(f"retain did not increment ref_count: {retain_payload}")
    if not pin_payload.get("pinned"):
        raise AssertionError(f"pin did not set pinned=true: {pin_payload}")
    if delete_conflict_payload.get("success") is not False:
        raise AssertionError(f"delete while pinned should fail: {delete_conflict_payload}")
    if unpin_payload.get("pinned") is not False:
        raise AssertionError(f"unpin did not clear pinned flag: {unpin_payload}")
    if release_payload.get("ref_count") != 0:
        raise AssertionError(f"release did not decrement ref_count: {release_payload}")
    if delete_payload.get("success") is not True:
        raise AssertionError(f"delete did not succeed: {delete_payload}")
    if meta_after_delete_payload.get("found") is not False:
        raise AssertionError(f"object still exists after delete: {meta_after_delete_payload}")

    summary = {
        "object_storage_status": status_payload,
        "rustfs_http_status": rustfs_status,
        "uploaded_key": key,
        "downloaded_content": content_body.decode("utf-8"),
        "downloaded_content_type": content_headers.get("content-type"),
        "retain": retain_payload,
        "pin": pin_payload,
        "delete_conflict": delete_conflict_payload,
        "unpin": unpin_payload,
        "release": release_payload,
        "delete": delete_payload,
    }
    print(json.dumps(summary, ensure_ascii=False, indent=2))
except (AssertionError, KeyError, urllib.error.URLError, urllib.error.HTTPError) as error:
    print(f"compose rustfs smoke failed: {error}", file=sys.stderr)
    sys.exit(1)
PY
