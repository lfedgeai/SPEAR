# SPEARlet Reconnect Strategy

## Defaults

- `heartbeat_interval = 30s`
- `sms_connect_retry_ms = 500`
- `sms_connect_timeout_ms = 15000`
- `reconnect_total_timeout_ms = 300000` (5 minutes)

## Flow

1. Heartbeat fails → immediately attempt reconnect
2. Reconnect uses retry window (`timeout_ms` with `retry_ms` interval)
3. On success → immediately re-register (no wait for next tick)
4. While disconnected, track start time; when elapsed ≥ `reconnect_total_timeout_ms` → exit process

## Configuration

- CLI:
  - `--sms-connect-retry-ms` / `--sms-connect-timeout-ms` / `--reconnect-total-timeout-ms`
- ENV:
  - `SPEARLET_SMS_CONNECT_RETRY_MS`, `SPEARLET_SMS_CONNECT_TIMEOUT_MS`, `SPEARLET_RECONNECT_TOTAL_TIMEOUT_MS`

## Notes

- Re-registration after reconnect reduces perceived delay
- For faster responsiveness, consider `heartbeat_interval = 10s` and `sms_connect_timeout_ms = 5000`
