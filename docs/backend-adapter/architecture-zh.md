# 分层架构与模块边界

## 1. 分层职责

建议将“hostcall → 后端请求”的路径拆为四层：

1) **Hostcall 协议层（ABI 稳定）**
- 负责解析 WASM 参数与内存布局
- 把参数写入 host 侧会话状态（例如 chat session）或一次性请求状态
- 不包含任何后端协议/选路逻辑

2) **Normalize 层（Adapter Core）**
- 将 host 侧会话/状态规范化为 `CanonicalRequestEnvelope`
- 做字段校验、默认值填充、语义对齐
- 不做网络调用

3) **Router 层（Control Plane）**
- 根据 `operation + requirements + policy` 选择 backend instance（1 个或多个）
- 维护候选健康状态、并发/限流、权重/优先级等运行时信号

4) **Backend Adapter 层（Data Plane）**
- 将 Canonical IR 转换为具体后端 API schema
- OpenAI-compatible：直通或轻量字段映射
- 非兼容后端：通过 adapter 做语义转换与显式降级

## 2. Hostcall family 的扩展路线

当从 chat 扩展到 image/asr/tts/embeddings/realtime 时，有两条路线：

命名约定建议：

- hostcall family 的前缀应表达“操作语义”，不要求统一使用某个字母前缀。
- `cchat` 的命名含义是 completion chat，因此保留 `cchat_*` 是合理的；其它操作建议使用更贴近语义的前缀。

### 2.1 路线 A：按操作拆分 hostcall family（推荐默认）

- `cchat_*`：completion chat
- `emb_*`：embeddings
- `img_*`：image（image generation/edit/variation 等）
- `asr_*`：speech-to-text（ASR）
- `tts_*`：text-to-speech（TTS）
- `rt_*`：realtime（或复用 stream 子系统）

优点：ABI 清晰、调试容易、每个 family 可以做更适合该操作的参数组织。

### 2.2 路线 B：统一 `ai_call/transform` hostcall（快速扩展）

- 一个 hostcall 传入 `operation + payload`，host 侧按 operation 分发
- 更像 legacy Go 的 `TransformRegistry` 风格

优点：扩展速度快；缺点：ABI 解释复杂，排查时需要更多工具支撑。

## 3. 模块组织建议（Rust）

建议在 `src/spearlet/execution` 下按关注点拆分：

- `hostcall/`：WASM hostcall ABI、内存读写、session 写入
- `adapter/`：normalize（session → Canonical IR）
- `router/`：registry、capabilities、policy、health/limits
- `backends/`：各 backend adapter（按 Cargo feature 编译裁剪）
- `stream/`：realtime/streaming 子系统（与 router 共享 registry 与 capabilities）

## 4. 关键边界与安全约束

- URL 与 API key 必须由 host 配置提供；WASM 只能选择 backend/model 名称或声明能力需求。
- 请求级 allowlist 只能收缩不能扩张；策略覆盖也必须受 host 配置约束。
