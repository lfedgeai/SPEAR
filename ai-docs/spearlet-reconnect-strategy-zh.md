# SPEARlet 重连策略

## 默认值

- `heartbeat_interval = 30s`
- `sms_connect_retry_ms = 500`
- `sms_connect_timeout_ms = 15000`
- `reconnect_total_timeout_ms = 300000`（5 分钟）

## 流程

1. 心跳失败 → 立即尝试重连
2. 重连使用重试窗口（`timeout_ms` 内按 `retry_ms` 间隔重试）
3. 重连成功 → 立即重新注册（无需等待下一次 tick）
4. 断线期间记录起始时间；累计时长 ≥ `reconnect_total_timeout_ms` 时退出进程

## 配置

- CLI：
  - `--sms-connect-retry-ms` / `--sms-connect-timeout-ms` / `--reconnect-total-timeout-ms`
- 环境变量：
  - `SPEARLET_SMS_CONNECT_RETRY_MS`、`SPEARLET_SMS_CONNECT_TIMEOUT_MS`、`SPEARLET_RECONNECT_TOTAL_TIMEOUT_MS`

## 说明

- 重连后立即重新注册可降低感知延迟
- 若希望更快响应，建议将 `heartbeat_interval` 调小到 10s、将 `sms_connect_timeout_ms` 调小到 5000ms
