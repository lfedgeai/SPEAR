# Qwen 统一接入架构改造路线图

## 1. 目标

本文给出一条**不要求兼容旧配置和旧代码语义**的 Qwen 接入改造路线，目标是在不引入 provider-specific 意面逻辑的前提下，把 Qwen / DashScope / Model Studio 的多类模型统一纳入现有 AI backend 架构。

目标包括：

- 让 `provider` 表达厂商与控制面归属，而不是承载协议细节。
- 让 `kind` 表达 adapter / 协议家族，而不是把 provider 与实现强行绑定。
- 让 `operation` 表达能力类型，`model` 表达具体模型名。
- 在**能共享协议族**时复用现有 adapter，在**协议明显不同**时单独落独立 kind。
- 为后续接入 Qwen 文本生成、多模态理解、Embedding、Rerank、TTS、图像生成与更完整 Realtime 能力预留稳定扩展面。

相关现状参考：

- [backend-adapter/backends-zh.md](backend-adapter/backends-zh.md)
- [implementation/realtime-asr-implementation-zh.md](implementation/realtime-asr-implementation-zh.md)
- [web-admin-ui-guide-zh.md](web-admin-ui-guide-zh.md)

## 2. 背景判断

根据 Qwen 官方模型选择与子页面说明，Qwen 平台不是“一个模型 + 一套接口”，而是：

- 文本生成 / 多模态理解：一大批模型共享聊天式调用心智，支持长上下文、工具调用、思考模式等。
- 语音识别：分实时与非实时，实时链路基于专门的实时协议。
- Realtime API：还存在 `AOQ`、`WebRTC`、`WebSocket` 三种传输层形态。
- 向量与重排：是独立任务接口，不应硬塞进 chat adapter。
- 图像 / 视频生成、TTS：也各自有独立 API 形态与参数集合。

因此，Qwen 的统一接入不能设计成：

- `provider = qwen`
- 再堆出一批 `qwen_chat` / `qwen_vision_chat` / `qwen_embedding` / `qwen_tts` / `qwen_image` 的散乱分支

更合理的方向是：

- **同协议族共享 adapter**
- **同能力族共享 operation**
- **同厂商共享 provider / credential / preflight / 文档与 Web Admin 入口**

## 3. 现有架构中的问题

### 3.1 `kind` 与 `provider` 绑定过紧

当前 `CanonicalBackendKind` 会推导默认 `provider`，配置层还会要求二者严格匹配。这意味着：

- `openai_chat_completion` 天然只能配 `provider = openai`
- 即使 Qwen 某类 LLM 能通过兼容接口复用同一 adapter，也无法自然表达成
  - `provider = qwen`
  - `kind = openai-compatible chat family`

这会阻止“同协议族复用 adapter”的统一思路。

### 3.2 `provider` 被误用成“模型家族”

从官方页面看，Qwen 平台不仅承载 Qwen 自家模型，也承载三方模型，且 API 形态一致。也就是说：

- `provider = qwen`

更准确的含义应是：

- 该 backend 走 DashScope / Model Studio 控制面
- 其 credential、preflight、Web Admin 展示与官方文档归属属于 Qwen 平台

而不是：

- 该 backend 只允许调用“Qwen 自家模型”

### 3.3 operation 覆盖面已经不错，但 payload 尚偏窄

当前 IR 已有：

- `chat_completions`
- `embeddings`
- `image_generation`
- `speech_to_text`
- `text_to_speech`

这是对的，但 payload 还没有覆盖 Qwen 各能力面常见的 richer params，例如：

- embedding 维度、融合/独立向量模式
- image generation 的编辑/参考图/分辨率
- TTS 的更丰富语音参数
- realtime multimodal 的更高层 session 参数

## 4. 设计原则

### 4.1 明确分层

- `provider`
  - 表达厂商、控制面、凭据来源、预检逻辑、Web Admin 展示归属
- `kind`
  - 表达 adapter / 协议家族
- `operation`
  - 表达能力类型
- `transport`
  - 表达 HTTP / WebSocket / WebRTC / AOQ 等传输
- `model`
  - 表达具体模型名
- `features`
  - 表达能力声明，如 tools、thinking、vision input、audio input 等

### 4.2 兼容协议共用 adapter

如果不同 provider 在请求/响应/鉴权/endpoint 结构上足够一致，应优先共享 adapter，而不是按 provider 拆分。

### 4.3 协议明显不同才独立建 kind

只有在以下条件之一成立时，才新增 provider-specific kind：

- endpoint 结构明显不同
- 请求体结构明显不同
- streaming / realtime 协议明显不同
- 返回结构或错误模型差异足够大
- 控制面预检需要独立处理

### 4.4 不做旧名兼容包袱

本路线图默认允许：

- 重命名 backend kind
- 调整 config typed view 语义
- 调整 Web Admin 枚举与展示逻辑
- 直接迁移文档与测试，不保留旧 alias

## 5. 目标架构

## 5.1 provider 语义

建议保留并强化如下 provider：

- `openai`
- `qwen`
- `ollama`
- `llamacpp`
- `vllm`
- `internal`

其中：

- `provider = qwen` 表示走 DashScope / Model Studio 控制面
- 不表示 model 必须是 Qwen 自家模型

## 5.2 kind 语义

建议把 `kind` 改造成真正的“adapter family”。

推荐的目标 kind 列表：

- `chat_http`
  - 用于聊天式 HTTP JSON adapter
  - 首批支持 provider：
    - `openai`
    - `qwen`
- `rt_asr_openai_ws`
  - OpenAI 风格 realtime ASR websocket
- `rt_asr_qwen_ws`
  - Qwen realtime ASR websocket
- `embeddings_dashscope_http`
  - DashScope embedding 系列
- `rerank_dashscope_http`
  - DashScope rerank 系列
- `image_generation_dashscope_http`
  - DashScope image/video generation 系列的首期文本到图像能力
- `text_to_speech_dashscope_http`
  - DashScope TTS 系列
- `realtime_multimodal_qwen_ws`
  - 后续 Qwen WebSocket realtime multimodal family
- `realtime_multimodal_qwen_webrtc`
  - 后续 Qwen WebRTC realtime family
- `realtime_multimodal_qwen_aoq`
  - 后续 Qwen AOQ realtime family
- `ollama_chat`
- `llamacpp`
- `vllm`
- `stub`

其中最关键的一点是：

- 把原先 `openai_chat_completion` 升级为 provider-neutral 的 `chat_http`

这样 Qwen 文本生成、多模态理解、工具调用能力可以优先统一到同一 adapter family，而不是额外做一个 `qwen_llm`。

## 5.3 operation 扩展

当前 operation 中最值得补的一项是：

- `rerank`

建议目标 operation 为：

- `chat_completions`
- `embeddings`
- `rerank`
- `image_generation`
- `speech_to_text`
- `text_to_speech`
- 后续视需要新增：
  - `realtime_multimodal`
  - `speech_to_speech`

## 5.4 features 建议

建议把 provider 差异尽可能下沉为 `features` 声明，而不是继续做 provider 分叉：

- `supports_stream`
- `supports_tools`
- `supports_thinking`
- `supports_structured_output`
- `supports_vision_input`
- `supports_video_input`
- `supports_audio_input`
- `supports_audio_output`
- `supports_builtin_tools`
- `supports_fusion_embedding`
- `supports_separate_embedding`

## 6. 分阶段改造路线

## Phase 0：命名和分类语义清理

目标：先把“provider 是谁、kind 是什么”彻底说清楚。

改造项：

- 重写 `CanonicalBackendKind` 的命名语义与枚举值。
- 移除“一个 kind 只能对应一个 provider”的设计假设。
- 让 `kind` 只表达 adapter family。
- 让 `provider` 只表达厂商/控制面。
- 将 canonical backend family id 收敛到 Rust 类型层常量与 Web Admin registry，避免表单、运行时与测试各自维护重复字符串。
- 将 canonical backend provider id 同步收敛到共享常量，避免 `openai/qwen/ollama/llamacpp/vllm` 在运行时、控制面与前端逻辑层散落硬编码。
- Web Admin 测试 fixture 也统一复用 `AI_BACKEND_PROVIDERS.*`，避免 `provider_family` 字面量在测试层重新漂移。

涉及文件：

- `src/ai_backend_types.rs`
- `src/spearlet/config.rs`
- `src/sms/ai_backends/validator.rs`
- `src/sms/web_admin/ai_backend_admin.rs`
- `web-admin/src/features/ai-backends/*`

验收标准：

- 可以合法表达：
  - `provider = qwen`
  - `kind = chat_http`
- typed view、validator、Web Admin 都不再将其视为非法组合。
- Rust 运行时与 Web Admin 表单默认值共享同一份 family id 定义来源。
- Rust 与 Web Admin 逻辑层共享同一份 provider id 定义来源。

## Phase 1：统一 Chat / 多模态理解能力

目标：先让 Qwen 大量文本生成与视觉理解模型统一进现有 chat-completions 架构。

改造项：

- 将现有 `OpenAIChatCompletionBackendAdapter` 重命名或提炼为更中性的 adapter family，例如：
  - `ChatHttpBackendAdapter`
- 保留对 OpenAI-compatible 风格的：
  - `messages`
  - `tools`
  - `params`
  透传能力
- 允许 `provider = qwen` 使用该 kind
- 在 remote preflight 中为该 kind 允许：
  - `openai`
  - `qwen`

为何先做这步：

- Qwen 官方文本生成、视觉理解、大量工具调用能力都适合落在统一的 chat-completions family
- 当前 IR `ChatMessage.content = Value`、`tools`、`params` 已经给了足够宽的上层接口

验收标准：

- 可以创建如下 backend：

```toml
[[spearlet.ai.backends]]
name = "qwen-chat"
provider = "qwen"
kind = "chat_http"
hosting = "remote"
base_url = "https://dashscope.aliyuncs.com/compatible-mode/v1"
model = "qwen3.7-plus"
credential_ref = "qwen_prod"
ops = ["chat_completions"]
features = ["supports_tools", "supports_thinking"]
transports = ["http"]
```

- 同一 kind 也能用于 `openai`。
- Web Admin 能创建/编辑/展示这类 backend。

## Phase 2：保留并重命名现有 Qwen Realtime ASR

目标：把当前已有的 Qwen realtime ASR 支持迁移到新的命名与分类体系里。

改造项：

- 将 `qwen_asr_realtime_ws` 迁移为更统一的命名：
  - `rt_asr_qwen_ws`
- 保持独立 adapter，不与 OpenAI realtime ASR 混用
- 保持独立 remote preflight 分支
- 更新相关 live test、sample 与 Web Admin

为何要保留独立 kind：

- Qwen realtime ASR 的 websocket endpoint、`session.update` 结构、实时 transcript 事件语义与 OpenAI 明显不同
- 它不属于“兼容协议族可直接复用”的范围

验收标准：

- 现有 `live_caption` / `voice_chat`
- 现有 `qwen_realtime_asr_live_tests`
- 现有 `realtime_asr_live_tests`

全部迁移到新 kind 名称并通过。

## Phase 3：新增 Qwen Embedding 与 Rerank

目标：让 Qwen 的向量和重排正式成为一等能力。

改造项：

- 为 `Operation` 增加 `Rerank`
- 扩展 payload：
  - `EmbeddingsPayload`
    - 增加可选 `dimensions`
    - 增加可选 `encoding_mode`
    - 为多模态 embedding 预留扩展字段
  - 新增 `RerankPayload`
    - `query`
    - `documents`
    - `top_n`
    - 可选 provider params
- 新增：
  - `embeddings_dashscope_http`
  - `rerank_dashscope_http`

为何不强行走 chat：

- embedding / rerank 是独立能力面
- 官方接口与返回结构、调参与约束明显不同

验收标准：

- runtime adapter matrix 能清晰声明 embeddings / rerank 能力
- router 能按 operation 正确选择 backend
- Web Admin 能为 Qwen 创建 embeddings / rerank backend

## Phase 4：新增 Qwen TTS 与图像生成

目标：补齐 Qwen 平台的常见非聊天能力面。

改造项：

- 扩展 `TextToSpeechPayload`
  - 允许 provider params
- 扩展 `ImageGenerationPayload`
  - 允许 provider params
  - 视需要后续再拆更细 payload
- 新增：
  - `text_to_speech_dashscope_http`
  - `image_generation_dashscope_http`

注意：

- 首期不强求一次把视频生成、图片编辑、参考图等所有能力做完
- 先以“最小可用的一等能力接入”落地

## Phase 5：设计 Qwen 更完整的 Realtime 体系

目标：为 Qwen 的语音转语音、全模态实时交互、不同传输协议预留长期架构。

改造项：

- 新增 operation：
  - `realtime_multimodal`
  - 或 `speech_to_speech`
- 新增 transport：
  - `webrtc`
  - `aoq`
- 新增 kind：
  - `realtime_multimodal_qwen_ws`
  - `realtime_multimodal_qwen_webrtc`
  - `realtime_multimodal_qwen_aoq`

注意：

- 不要把这类能力继续塞进现在的 `speech_to_text` 语义下
- 不要让 `rt_asr_qwen_ws` 承担未来全模态/语音对话的职责

## 7. 文件级改造清单

### 7.1 类型与配置层

- `src/ai_backend_types.rs`
  - 重写 kind 枚举与 provider 推导规则
- `src/spearlet/config.rs`
  - 放宽 provider/kind 匹配关系
  - 引入“kind 支持的 provider family 集合”校验
- `src/sms/ai_backends/validator.rs`
  - 从“一对一匹配”转为“kind 允许的 provider 集合”校验

### 7.2 runtime assembly 层

- `src/spearlet/ai/backend_assembly.rs`
  - 按新的 kind family 重新映射 adapter
- `src/spearlet/execution/ai/backends/chat_http.rs`
  - 提炼为 provider-neutral 的 chat HTTP JSON adapter
- `src/spearlet/execution/ai/backends/rt_asr_qwen_ws.rs`
  - 迁移命名但保留独立实现
- 后续新增：
  - embeddings / rerank / tts / image generation adapter

### 7.3 preflight 层

- `src/spearlet/ai/remote_preflight.rs`
  - 按 provider family 做控制面检查
  - 不再把 preflight family 与 runtime kind 一对一绑定
- `src/spearlet/http_gateway/backend_preflight_handlers.rs`
  - 更新 provider family 解析与前端调用返回结构

### 7.4 Web Admin 层

- `web-admin/src/features/ai-backends/AiBackendEditorForm.ts`
- `web-admin/src/features/ai-backends/AiBackendsPage.tsx`
- `web-admin/src/features/ai-backends/AiBackendDetailPage.utils.ts`
- `web-admin/src/api/ai-backends.ts`

目标：

- provider 选择与 kind 选择分离但保持约束
- Qwen chat / embeddings / rerank / tts / image generation 能按 operation 正确建模
- model 仍保持 freeform，不做硬编码枚举

## 8. 测试路线

### 8.1 单测

- kind/provider 组合校验
- config typed view
- preflight family 识别
- runtime assembly 映射
- Web Admin form / payload / detail

### 8.2 集成测试

- `chat_http` 在 `provider=openai` 下通过
- `chat_http` 在 `provider=qwen` 下通过
- `rt_asr_qwen_ws` live test 通过
- embeddings / rerank 的最小 real API 测试

### 8.3 样例验证

- Rust `live_caption`
- Rust `voice_chat`
- Chat 示例在 `provider=qwen` 下调用 `qwen3.7-plus`

## 9. 文档路线

需要同步更新：

- [backend-adapter/backends-zh.md](backend-adapter/backends-zh.md)
- [backend-support-matrix-zh.md](backend-support-matrix-zh.md)
- [implementation/realtime-asr-implementation-zh.md](implementation/realtime-asr-implementation-zh.md)
- [web-admin-ui-guide-zh.md](web-admin-ui-guide-zh.md)

并新增与本文件对应的英文版本。

## 10. 推荐实施顺序

推荐严格按下面顺序推进：

1. 重构 `provider / kind` 语义
2. 统一 chat-completions family，让 Qwen LLM 先跑起来
3. 迁移 Qwen realtime ASR 到新命名体系
4. 增加 rerank
5. 增加 embeddings
6. 增加 TTS
7. 增加 image generation
8. 最后再做更完整的 realtime multimodal

原因：

- 这样能以最小架构代价先覆盖 Qwen 平台里价值最高、模型最多的文本/多模态理解能力
- 同时不污染现有 realtime ASR 的独立实现
- 还可以把 embedding / rerank / tts / image generation 作为后续独立 PR 分步落地

## 11. 最终建议

如果只保留一条最关键建议，那就是：

- **不要为 Qwen LLM 新造一个 provider-specific chat adapter；先把现有 chat HTTP adapter 提升为 provider-neutral 的协议 family，并允许 `provider=qwen` 复用。**

这样做：

- 最符合 Qwen 官方“同平台多模型、多能力、多供应商、同类 API 家族共享”的现实
- 最符合你们现有 `operation + transport + model` 的架构方向
- 最能避免未来继续长出 `qwen_*` 专用分支树
