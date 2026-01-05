# 可靠性、安全与可观测性

## 1. 错误模型（Error Model）

建议定义稳定错误类别，便于任务侧自动处理：

- `BackendNotEnabled`
- `NoCandidateBackend`
- `MissingCapabilities`
- `RateLimited`
- `Timeout`
- `BackendError`
- `InvalidRequest`

错误建议携带：

- `operation`
- `required_caps`（摘要）
- `candidates_checked`
- `rejected_reasons`（可选）
- `retryable`

## 2. 可靠性机制（建议逐步落地）

- 并发限制（per instance / per operation）
- 速率限制（token bucket 或窗口）
- 超时与重试（注意幂等性与成本）
- 熔断与剔除（错误率/连续失败/超时率阈值）

## 3. 安全边界

- secret（API key）必须由 host 管理：通过 `credential_ref` 解析到 `credentials[].api_key_env` 并读取，禁止 WASM 传入
- base_url 与网络策略由 host 配置，禁止 WASM 注入任意 URL
- allowlist/denylist 由 host 定义，请求侧只能进一步收缩
- 记录审计日志时不得输出 secret

## 4. 可观测性

建议最小指标：

- `operation`, `backend_instance`
- `request_count`, `error_count`, `timeout_count`, `rate_limited`
- `latency_ms`（p50/p95/p99）
- `inflight`

对策略相关的指标：

- fallback 次数
- hedged 的 secondary 触发率
- mirror 的采样率

建议在每次路由决策上保留结构化解释信息（可采样）：选择原因、候选集、剔除原因。
