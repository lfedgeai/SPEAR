# WASM 运行时使用说明

## 概述
Spearlet 的 WASM 运行时在实例创建阶段需要合法的 WASM 二进制模块字节。若模块字节不合法（例如缺失 WASM 魔数），将返回 `InvalidConfiguration` 并拒绝创建实例。

## 生命周期约定
- `create_instance`：下载模块字节、校验 WASM 魔数、加载模块并创建实例句柄
- `start_instance`：将实例推进到可服务状态；在启用 `wasmedge` feature 时启动 WASM worker
- `stop_instance`：发送 Stop 控制请求并等待确认，确保 worker 退出

## 实例化要求
- 实例配置必须能解析到模块字节：通过 `InstanceConfig.artifact.location` 指向的 URI 下载得到（该字段由任务的 `executable.uri` 在 artifact/materialize 阶段转换而来）
- 模块字节必须以 WASM 魔数开头：`00 61 73 6d`（`\0asm`）
- 非法或空内容将导致 `create_instance` 报错

## 与任务注册集成
- 注册任务时建议使用 `executable` 字段描述可执行：
```json
{
  "name": "hello-wasm",
  "priority": "normal",
  "endpoint": "",
  "version": "1.0.0",
  "capabilities": ["wasm"],
  "metadata": {},
  "config": {},
  "executable": {
    "type": "wasm",
    "uri": "smsfile://<file_id>",
    "name": "hello.wasm",
    "args": [],
    "env": {}
  }
}
```
- 运行时将在 `create_instance` 中根据 artifact 的 `location` 下载内容，并在加载前校验 WASM 魔数；随后创建 WASM 实例句柄。
- 在启用 `wasmedge` feature 时，`start_instance` 会启动 WASM worker（用于接收并执行后续的函数调用请求）。

### SMS 文件协议与配置来源

- 支持 `smsfile` 下载协议：
  - 显式覆盖：`smsfile://<host:port>/<file_id>`
  - 简洁形式：`smsfile://<file_id>`（将使用 `SpearletConfig.sms_http_addr` 作为 HTTP 网关地址）
- 运行时构造路径 `"/api/v1/files/<file_id>"`。
- 下载函数签名：

```rust
pub async fn fetch_sms_file(sms_http_addr: &str, path: &str) -> ExecutionResult<Vec<u8>>
```

* 配置在运行时初始化阶段由 FunctionService 注入：通过 `RuntimeConfig.spearlet_config` 传递完整 `SpearletConfig`，其中 `sms_http_addr` 用于 HTTP 下载，避免在运行时中读取环境变量。

## 错误行为
- 非合法 WASM 字节：实例创建立即返回错误，避免在执行阶段才发现问题
- 下载失败或校验失败：记录具体错误信息并返回 `RuntimeError` 或 `InvalidConfiguration`

## 最佳实践
- 提前在构建环节生成合法 WASM：
  - C：`zig cc -target wasm32-wasi`
  - Rust：`cargo build --release --target wasm32-wasip1`
- 对于通过 SMS 文件服务上传的模块，建议保留校验信息（`checksum_sha256`）
- 在集成测试中显式提供合法模块字节，以验证入口函数选择与执行路径

## 用户流（WebSocket <-> WASM）

Spearlet 支持将外部客户端的双向流式数据桥接到 WASM 实例，WASM 侧使用 fd/epoll 模型进行读写。

- WebSocket 端点：`GET /api/v1/executions/{execution_id}/streams/ws`
- 如果 execution 无法直连，可通过 SMS gateway：
  - `POST /api/v1/executions/{execution_id}/streams/session`
  - `GET /api/v1/executions/{execution_id}/streams/ws?token=...`
- 载荷：SSF v1 二进制帧（`SPST` magic）。详见：[wasm-user-stream-bridge-zh.md](./api/spear-hostcall/wasm-user-stream-bridge-zh.md)

### WASM hostcalls

- `user_stream_open(stream_id: i32, direction: i32) -> i32`
- `user_stream_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`
- `user_stream_write(fd: i32, buf_ptr: i32, buf_len: i32) -> i32`
- `user_stream_close(fd: i32) -> i32`
- `user_stream_ctl_open() -> i32`
- `user_stream_ctl_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`

direction 取值：

- `1`：输入（client -> guest）
- `2`：输出（guest -> client）
- `3`：双向

推荐用法：

- 使用 `spear_epoll_create/spear_epoll_ctl/spear_epoll_wait` 等待 `EPOLLIN/EPOLLOUT`。
- 将 `-EAGAIN` 视为背压，收到 epoll 就绪后再重试。

stream 发现用法：

- 通过 `user_stream_ctl_open()` 创建一个控制 fd，并用 `EPOLLIN` 注册到 epoll。
- 控制 fd 可读时，调用 `user_stream_ctl_read()` 读取一个固定 8 字节事件：
  - `u32 stream_id`（小端）
  - `u32 kind`（小端），当前定义：
    - `1`：stream connected
    - `2`：session closed
- 拿到 `stream_id` 后，再调用 `user_stream_open(stream_id, direction)` 绑定该 stream 的数据 fd。
- 不再需要时，用 `user_stream_close(fd)` 关闭控制 fd。
