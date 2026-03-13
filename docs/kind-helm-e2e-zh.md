# Kind + Helm 端到端测试（E2E）

该 E2E 会使用 kind 创建 Kubernetes 集群、在本机构建容器镜像、安装 Helm chart，然后做一组运行时断言（Pod 就绪 + HTTP health 检查）。

默认会在执行结束后卸载 Helm release 并删除 namespace（避免污染后续测试）。如需保留资源用于排查，可设置 `E2E_CLEANUP=0`。

部署就绪后，可通过 port-forward 访问 SMS Web Admin：

```bash
kubectl -n spear port-forward svc/spear-spear-sms 18082:8081
```

然后在浏览器打开 `http://127.0.0.1:18082/`。

## 运行

```bash
make e2e
```

或只运行 kind：

```bash
make e2e-kind
```

## 可选参数

通过环境变量控制：

- `E2E_SUITES`（默认：`auto`），可选：`kind`、`docker`、`docker,kind`
- `E2E_CLEANUP`（默认：`1`），设为 `0` 将跳过卸载/删除
- `CLEANUP_ON_FAIL`（默认：`0`），设为 `1` 即使失败也清理资源
- `CLUSTER_NAME`（默认：`spear-e2e`）
- `REUSE_CLUSTER`（默认：`0`）设为 `1` 复用已存在的集群
- `KEEP_CLUSTER`（默认：`0`）设为 `1` 测试结束后不删除集群
- `NAMESPACE`（默认：`spear`）
- `RELEASE_NAME`（默认：`spear`）
- `ENABLE_WEB_ADMIN`（默认：`1`）设为 `0` 不启用 SMS Web Admin
- `ENABLE_ROUTER_FILTER_AGENT`（默认：`1`）设为 `0` 不启用 sidecar
- `TIMEOUT`（默认：`300s`）
- `DEBIAN_SUITE`（默认：`trixie`）设为 `bookworm`，运行时镜像将使用 `debian:bookworm-slim`

示例：

```bash
CLUSTER_NAME=spear KEEP_CLUSTER=1 make e2e-kind
```
