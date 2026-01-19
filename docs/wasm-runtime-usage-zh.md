# WASM 运行时使用说明

## 概述
Spearlet 的 WASM 运行时在实例创建阶段需要合法的 WASM 二进制模块字节。若传入的字节不是合法的 WASM，将返回 `InvalidConfiguration: "Invalid WASM module format"`。

## 实例化要求
- 实例配置必须能解析到模块字节：通过 `InstanceConfig.runtime_config["module_bytes"]` 或通过任务可执行 `executable.uri` 下载得到
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
    "uri": "sms+file://<file_id>",
    "name": "hello.wasm",
    "args": [],
    "env": {}
  }
}
```
- 运行时将根据 `executable.uri` 下载内容，并在 `create_wasm_instance` 中严格校验模块格式。

### SMS 文件协议与配置来源

- 支持 `sms+file` 下载协议：
  - 显式覆盖：`sms+file://<host:port>/<file_id>`
  - 简洁形式：`sms+file://<file_id>`（将使用 `SpearletConfig.sms_http_addr` 作为 HTTP 网关地址）
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
