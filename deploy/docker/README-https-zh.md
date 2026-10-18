# Docker Compose HTTPS 叠加层

该部署采用“基础 compose + HTTPS overlay”的组织方式：

- `compose.local.yaml`：仅包含核心服务
- `compose.https.yaml`：通过 Caddy 提供外部 TLS 终止

本地基础 compose 现在也包含独立的 `rustfs` 服务，SMS 会通过 `object_storage.rustfs.*` 配置使用 `RustFsBlobStore`。

## 对外暴露的 HTTPS 端点

- `https://<host-or-ip>:18443/admin` -> SMS Web Admin
- `https://<host-or-ip>:18444/console` -> SMS Console
- `https://<host-or-ip>:18445/` -> Debug Server

当还没有域名时，`<host-or-ip>` 可以直接使用 IP 地址。

## 说明

- 应用容器内部仍然通过纯 HTTP 提供服务。
- Caddy 通过 `tls internal` 对外终止 TLS。
- 当前本地 SMS setup 已切换到 RustFS，不再使用旧的本地文件系统兼容对象后端。
- 该 HTTPS overlay 只对以下入口提供 HTTPS：
  - SMS Web Admin
  - SMS Console
  - Debug Server
- 如果不从基础 compose 中移除，现有直接 HTTP 端口仍可用于调试：
  - `18080` -> SMS HTTP
  - `18082` -> SMS Web Admin
  - `18081` -> Spearlet HTTP
  - `19000` -> RustFS S3-compatible API
  - `19001` -> RustFS Console

## RustFS 默认配置

本地 compose 会额外拉起一个独立的 RustFS 容器：

- 镜像：`rustfs/rustfs:latest`
- API 端点：`http://127.0.0.1:19000`
- Console 端点：`http://127.0.0.1:19001`
- 默认 access key：`rustfsadmin`
- 默认 secret key：`rustfsadmin`
- 默认 bucket：`sms-objects`
- 默认对象根前缀：`local`
- 默认本地 credential master key：`local-dev-master-key`
- 本地 compose 默认使用 Cargo `dev` profile 构建 `sms`、`spearlet` 与 `debug-server`，以控制启用 RustFS 后的本地启动成本

SMS 通过以下环境变量连接 RustFS：

- `SMS_OBJECT_STORAGE_BACKEND=rustfs`
- `SMS_OBJECT_STORAGE_RUSTFS_ENDPOINT=http://rustfs:9000`
- `SMS_OBJECT_STORAGE_RUSTFS_BUCKET`
- `SMS_OBJECT_STORAGE_RUSTFS_ROOT`
- `SMS_OBJECT_STORAGE_RUSTFS_REGION`
- `SMS_OBJECT_STORAGE_RUSTFS_ACCESS_KEY`
- `SMS_OBJECT_STORAGE_RUSTFS_SECRET_KEY`
- `SMS_OBJECT_STORAGE_RUSTFS_PATH_STYLE=true`

可选覆盖项：

- `RUSTFS_API_PORT`
- `RUSTFS_CONSOLE_PORT`
- `RUSTFS_ACCESS_KEY`
- `RUSTFS_SECRET_KEY`
- `SMS_OBJECT_STORAGE_RUSTFS_BUCKET`
- `SMS_OBJECT_STORAGE_RUSTFS_ROOT`
- `SMS_OBJECT_STORAGE_RUSTFS_REGION`

## 不启用 HTTPS 的启动方式

```bash
docker compose -f deploy/docker/compose.local.yaml up -d --build
```

启动后可以通过下面的接口确认 SMS 确实在使用 RustFS：

```bash
curl http://127.0.0.1:18082/admin/api/object-storage/status
```

也可以直接运行仓库级别的冒烟脚本：

```bash
make compose-rustfs-smoke
```

如果还要一起验证 `SMS_CREDENTIAL_MASTER_KEY` 下的凭证管理链路，可执行：

```bash
make e2e-compose
```

## 启用 HTTPS overlay 的启动方式

```bash
docker compose \
  -f deploy/docker/compose.local.yaml \
  -f deploy/docker/compose.https.yaml \
  up -d --build
```

## Host / IP 证书主题

默认情况下，Caddy 会为 `127.0.0.1` 颁发本地证书。

如果你希望从其它 IP 访问，请在启动前设置 `SPEAR_HTTPS_HOST`：

```bash
export SPEAR_HTTPS_HOST=192.168.1.10
docker compose \
  -f deploy/docker/compose.local.yaml \
  -f deploy/docker/compose.https.yaml \
  up -d --build
```

随后可访问：

- `https://192.168.1.10:18443/admin`
- `https://192.168.1.10:18444/console`
- `https://192.168.1.10:18445/`

## 可选端口覆盖

- `SMS_WEB_ADMIN_HTTPS_PORT`
- `SMS_CONSOLE_HTTPS_PORT`
- `DEBUG_SERVER_HTTPS_PORT`

如果设置了这些环境变量，实际宿主机端口可能与默认值不同。可以通过以下命令查看实时映射：

```bash
docker compose \
  -f deploy/docker/compose.local.yaml \
  -f deploy/docker/compose.https.yaml \
  ps https-proxy
```

## Debug Server

本地调试服务器也会映射到宿主机，便于直接查看日志：

- `http://127.0.0.1:17777/` - 轻量级日志查看器，包含 session/client/stream 下拉选择器，并保持与 console 一致的深色风格
- `http://127.0.0.1:17777/health`
- `http://127.0.0.1:17777/logs`
- `http://127.0.0.1:17777/clients`
- `http://127.0.0.1:17777/streams`

如果你希望通过 HTTPS overlay 访问，请使用：

- `https://127.0.0.1:18445/`

本地 debug server 现在由 Rust `debug-server` 二进制构建，并打包进本地 Docker 镜像。

日志目录结构如下：

- `session`
  - `client`
    - `stream`

默认值：

- `default-session`
- `default-client`
- `default-stream`

路由规则：

- 如果事件中包含 `clientName`，则使用该 client
- 如果事件中包含 `streamName`，则使用该 stream
- 缺失时回退到上述默认值

在本地 compose setup 中，`spearlet` 会发送：

- `DEBUG_SESSION_ID=${DEBUG_SESSION_ID:-default-session}`
- `DEBUG_CLIENT_NAME=${DEBUG_CLIENT_NAME:-default-client}`
- `DEBUG_STREAM_NAME=${DEBUG_STREAM_NAME:-default-stream}`

当启用 `debug-reporter` 路径时，镜像出的 WASM guest 日志会发送以下字段：

- `streamName=wasm-log`
- `data.taskId`
- `data.executionId`
- `data.instanceId`

另见：

- `../../docs/debug-server-log-mirroring-en.md`
- `../../docs/debug-server-log-mirroring-zh.md`

可选覆盖项：

- `DEBUG_SERVER_PORT`
- `DEBUG_SESSION_ID`
- `DEBUG_CLIENT_NAME`
- `DEBUG_STREAM_NAME`

## 排障

### Console HTTPS 需要 API 透传

Console 页面由 `/console` 提供，但前端也会以同源方式调用 `/api/v1/tasks` 之类的接口。

因此 HTTPS console 代理必须同时转发：

- `/console*`
- `/api/*`

在当前配置中，`18444` 这个 HTTPS 入口会把所有非根路径请求代理到 `sms:8080`，而 `/` 会重定向到 `/console`。

### 重新加载 Caddy 配置

修改 `deploy/docker/caddy/Caddyfile` 之后，建议强制重建 HTTPS proxy 以确保新配置被加载：

```bash
docker compose \
  -f deploy/docker/compose.local.yaml \
  -f deploy/docker/compose.https.yaml \
  up -d --force-recreate https-proxy
```
