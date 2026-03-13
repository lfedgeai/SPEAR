#!/usr/bin/env bash
set -euo pipefail

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing dependency: $1" >&2
    exit 1
  fi
}

require_cmd docker
require_cmd kind
require_cmd kubectl
require_cmd helm
require_cmd curl

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

CLUSTER_NAME="${CLUSTER_NAME:-spear-e2e}"
REUSE_CLUSTER="${REUSE_CLUSTER:-0}"
KEEP_CLUSTER="${KEEP_CLUSTER:-0}"
E2E_CLEANUP="${E2E_CLEANUP:-1}"
CLEANUP_ON_FAIL="${CLEANUP_ON_FAIL:-0}"

NAMESPACE="${NAMESPACE:-spear}"
RELEASE_NAME="${RELEASE_NAME:-spear}"

SMS_IMAGE_REPO="${SMS_IMAGE_REPO:-spear-sms}"
SPEARLET_IMAGE_REPO="${SPEARLET_IMAGE_REPO:-spear-spearlet}"
ROUTER_FILTER_AGENT_IMAGE_REPO="${ROUTER_FILTER_AGENT_IMAGE_REPO:-spear-router-filter-agent}"
IMAGE_TAG="${IMAGE_TAG:-local}"

ENABLE_ROUTER_FILTER_AGENT="${ENABLE_ROUTER_FILTER_AGENT:-1}"

ENABLE_WEB_ADMIN="${ENABLE_WEB_ADMIN:-1}"

DEBIAN_SUITE="${DEBIAN_SUITE:-trixie}"

TIMEOUT="${TIMEOUT:-300s}"

PORT_FORWARD_WAIT_S="${PORT_FORWARD_WAIT_S:-2}"
HTTP_RETRY_TOTAL_S="${HTTP_RETRY_TOTAL_S:-60}"
HTTP_RETRY_INTERVAL_S="${HTTP_RETRY_INTERVAL_S:-2}"

SMS_STS="${RELEASE_NAME}-spear-sms"
SMS_POD="${SMS_STS}-0"
SPEARLET_DS="${RELEASE_NAME}-spear-spearlet"

PREV_CONTEXT="$(kubectl config current-context 2>/dev/null || true)"

cleanup() {
  local rc="$?"
  set +e

  if [[ -n "${SMS_PF_PID:-}" ]]; then
    kill "${SMS_PF_PID}" >/dev/null 2>&1 || true
    wait "${SMS_PF_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${SPEARLET_PF_PID:-}" ]]; then
    kill "${SPEARLET_PF_PID}" >/dev/null 2>&1 || true
    wait "${SPEARLET_PF_PID}" >/dev/null 2>&1 || true
  fi

  if [[ "$rc" -ne 0 ]]; then
    kubectl -n "$NAMESPACE" get pods -o wide >/dev/null 2>&1 || true
    kubectl -n "$NAMESPACE" get pods -o wide || true
    kubectl -n "$NAMESPACE" get events --sort-by=.metadata.creationTimestamp | tail -n 80 || true
    kubectl -n "$NAMESPACE" describe pod -l app.kubernetes.io/instance="$RELEASE_NAME" | tail -n 200 || true
  fi

  if [[ "$E2E_CLEANUP" == "1" && ( "$rc" -eq 0 || "$CLEANUP_ON_FAIL" == "1" ) ]]; then
    kubectl config use-context "kind-$CLUSTER_NAME" >/dev/null 2>&1 || true
    helm -n "$NAMESPACE" uninstall "$RELEASE_NAME" >/dev/null 2>&1 || true
    kubectl delete namespace "$NAMESPACE" --ignore-not-found --timeout=120s >/dev/null 2>&1 || true
  fi

  if [[ -n "$PREV_CONTEXT" ]]; then
    kubectl config use-context "$PREV_CONTEXT" >/dev/null 2>&1 || true
  fi

  if [[ "$KEEP_CLUSTER" != "1" && "$REUSE_CLUSTER" != "1" ]]; then
    kind delete cluster --name "$CLUSTER_NAME" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if kind get clusters | grep -qx "$CLUSTER_NAME"; then
  if [[ "$REUSE_CLUSTER" != "1" ]]; then
    kind delete cluster --name "$CLUSTER_NAME"
    kind create cluster --name "$CLUSTER_NAME"
  fi
else
  kind create cluster --name "$CLUSTER_NAME"
fi

kubectl config use-context "kind-$CLUSTER_NAME"

docker build -f deploy/docker/sms/Dockerfile --build-arg "DEBIAN_SUITE=${DEBIAN_SUITE}" -t "${SMS_IMAGE_REPO}:${IMAGE_TAG}" .
docker build -f deploy/docker/spearlet/Dockerfile --build-arg "DEBIAN_SUITE=${DEBIAN_SUITE}" -t "${SPEARLET_IMAGE_REPO}:${IMAGE_TAG}" .
docker build -f deploy/docker/router-filter-agent/Dockerfile --build-arg "DEBIAN_SUITE=${DEBIAN_SUITE}" -t "${ROUTER_FILTER_AGENT_IMAGE_REPO}:${IMAGE_TAG}" .

kind load docker-image --name "$CLUSTER_NAME" \
  "${SMS_IMAGE_REPO}:${IMAGE_TAG}" \
  "${SPEARLET_IMAGE_REPO}:${IMAGE_TAG}" \
  "${ROUTER_FILTER_AGENT_IMAGE_REPO}:${IMAGE_TAG}"

HELM_ARGS=(
  upgrade
  --install
  "$RELEASE_NAME"
  deploy/helm/spear
  -n
  "$NAMESPACE"
  --create-namespace
  --set
  "sms.image.repository=${SMS_IMAGE_REPO}"
  --set
  "sms.image.tag=${IMAGE_TAG}"
  --set
  "spearlet.image.repository=${SPEARLET_IMAGE_REPO}"
  --set
  "spearlet.image.tag=${IMAGE_TAG}"
  --set
  "e2e.enabled=true"
)

if [[ "$ENABLE_WEB_ADMIN" == "1" ]]; then
  HELM_ARGS+=(
    --set
    "sms.config.enableWebAdmin=true"
  )
fi

if [[ "$ENABLE_ROUTER_FILTER_AGENT" == "1" ]]; then
  HELM_ARGS+=(
    --set
    "routerFilterAgent.enabled=true"
    --set
    "routerFilterAgent.image.repository=${ROUTER_FILTER_AGENT_IMAGE_REPO}"
    --set
    "routerFilterAgent.image.tag=${IMAGE_TAG}"
  )
fi

helm "${HELM_ARGS[@]}"

kubectl -n "$NAMESPACE" rollout status "statefulset/${SMS_STS}" --timeout="$TIMEOUT"
kubectl -n "$NAMESPACE" wait --for=condition=Ready "pod/${SMS_POD}" --timeout="$TIMEOUT"

kubectl -n "$NAMESPACE" rollout status "daemonset/${SPEARLET_DS}" --timeout="$TIMEOUT"
kubectl -n "$NAMESPACE" wait --for=condition=Ready pod -l app.kubernetes.io/component=spearlet --timeout="$TIMEOUT"

SPEARLET_POD="$(kubectl -n "$NAMESPACE" get pod -l app.kubernetes.io/component=spearlet -o jsonpath='{.items[0].metadata.name}')"

if [[ "$ENABLE_ROUTER_FILTER_AGENT" == "1" ]]; then
  ROUTER_READY="$(kubectl -n "$NAMESPACE" get pod "$SPEARLET_POD" -o jsonpath='{.status.containerStatuses[?(@.name=="keyword-filter-agent")].ready}')"
  if [[ "$ROUTER_READY" != "true" ]]; then
    echo "keyword-filter-agent not ready" >&2
    exit 1
  fi
fi

kubectl -n "$NAMESPACE" port-forward "pod/${SMS_POD}" 18080:8080 >/dev/null 2>&1 &
SMS_PF_PID="$!"
sleep "$PORT_FORWARD_WAIT_S"

elapsed=0
until curl -fsS "http://127.0.0.1:18080/health" >/dev/null 2>&1; do
  sleep "$HTTP_RETRY_INTERVAL_S"
  elapsed=$((elapsed + HTTP_RETRY_INTERVAL_S))
  if [[ "$elapsed" -ge "$HTTP_RETRY_TOTAL_S" ]]; then
    echo "sms health check timeout" >&2
    exit 1
  fi
done

kubectl -n "$NAMESPACE" port-forward "pod/${SPEARLET_POD}" 18081:8081 >/dev/null 2>&1 &
SPEARLET_PF_PID="$!"
sleep "$PORT_FORWARD_WAIT_S"

elapsed=0
until curl -fsS "http://127.0.0.1:18081/health" >/dev/null 2>&1; do
  sleep "$HTTP_RETRY_INTERVAL_S"
  elapsed=$((elapsed + HTTP_RETRY_INTERVAL_S))
  if [[ "$elapsed" -ge "$HTTP_RETRY_TOTAL_S" ]]; then
    echo "spearlet health check timeout" >&2
    exit 1
  fi
done

if [[ "$ENABLE_ROUTER_FILTER_AGENT" == "1" ]]; then
  resp="$(curl -fsS "http://127.0.0.1:18081/__e2e/llm/router-filter?content=my%20secret%20is%20123")"
  echo "$resp" | grep -q '"selected_backend":"stub_local"' || (echo "filter e2e failed: $resp" >&2; exit 1)
  echo "$resp" | grep -q '"stub_remote"' || (echo "filter e2e missing dropped stub_remote: $resp" >&2; exit 1)
fi

echo "kind e2e ok"
