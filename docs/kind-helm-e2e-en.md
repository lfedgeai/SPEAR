# Kind + Helm E2E

This E2E uses kind to create a Kubernetes cluster, builds local container images, installs the Helm chart, then runs a few runtime assertions (pod readiness + HTTP health checks).

By default it uninstalls the Helm release and deletes the namespace after the run to keep the environment clean. Set `E2E_CLEANUP=0` to keep resources for debugging.

After the deployment is ready, you can access SMS Web Admin by port-forwarding:

```bash
kubectl -n spear port-forward svc/spear-spear-sms 18082:8081
```

Then open `http://127.0.0.1:18082/`.

## Run

```bash
make e2e
```

Or run kind only:

```bash
make e2e-kind
```

## Options

Environment variables:

- `E2E_SUITES` (default: `auto`) values: `kind`, `docker`, `docker,kind`
- `E2E_CLEANUP` (default: `1`) set to `0` to skip uninstall/delete
- `CLEANUP_ON_FAIL` (default: `0`) set to `1` to cleanup even on failures
- `CLUSTER_NAME` (default: `spear-e2e`)
- `REUSE_CLUSTER` (default: `0`) set to `1` to reuse an existing cluster
- `KEEP_CLUSTER` (default: `0`) set to `1` to keep the cluster after the run
- `NAMESPACE` (default: `spear`)
- `RELEASE_NAME` (default: `spear`)
- `ENABLE_WEB_ADMIN` (default: `1`) set to `0` to disable SMS Web Admin
- `ENABLE_ROUTER_FILTER_AGENT` (default: `1`) set to `0` to disable the sidecar
- `TIMEOUT` (default: `300s`)
- `DEBIAN_SUITE` (default: `trixie`) set to `bookworm` to use `debian:bookworm-slim` for runtime images

Example:

```bash
CLUSTER_NAME=spear KEEP_CLUSTER=1 make e2e-kind
```
