# Reliability, Security, and Observability

## 1. Error model

Define stable error categories so tasks can react programmatically:

- `BackendNotEnabled`
- `NoCandidateBackend`
- `MissingCapabilities`
- `RateLimited`
- `Timeout`
- `BackendError`
- `InvalidRequest`

Errors should include:

- `operation`
- `required_caps` (summary)
- `candidates_checked`
- optional `rejected_reasons`
- `retryable`

## 2. Reliability mechanisms (incremental)

- Concurrency limits (per instance / per operation)
- Rate limits (token bucket or window)
- Timeouts and retries (mind idempotency and cost)
- Circuit breaking and outlier ejection (error-rate/timeout thresholds)

## 3. Security boundaries

- Secrets (API keys) are host-managed via `credential_ref` resolved to `credentials[].api_key_env`; WASM must not provide them
- Base URLs and network policy are host-configured; WASM must not inject arbitrary URLs
- Host allowlist/denylist is authoritative; request hints can only further restrict
- Audit logs must not contain secrets

## 4. Observability

Minimum metrics:

- `operation`, `backend_instance`
- `request_count`, `error_count`, `timeout_count`, `rate_limited`
- `latency_ms` (p50/p95/p99)
- `inflight`

Policy-related metrics:

- fallback counts
- hedged secondary trigger rate
- mirror sampling rate

The router should retain structured decision explanations (sampled): selection reason, candidates, rejection reasons.
