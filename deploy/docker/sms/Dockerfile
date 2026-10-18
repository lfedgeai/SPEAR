# SMS container image / SMS 容器镜像

ARG RUST_VERSION=1.91
ARG DEBIAN_SUITE=trixie
ARG NODE_IMAGE=node:20-bookworm-slim
ARG RUST_IMAGE=rust:${RUST_VERSION}-bookworm
ARG DEBIAN_IMAGE=debian:${DEBIAN_SUITE}-slim

FROM ${NODE_IMAGE} AS web_builder
WORKDIR /work
COPY web-admin/ web-admin/
COPY web-console/ web-console/
COPY assets/ assets/
RUN cd /work/web-admin \
  && (if [ -f package-lock.json ]; then npm ci --silent; else npm install --silent; fi) \
  && npm run build
RUN cd /work/web-console \
  && (if [ -f package-lock.json ]; then npm ci --silent; else npm install --silent; fi) \
  && npm run build

FROM ${RUST_IMAGE} AS builder

WORKDIR /work

ARG USE_CARGO_MIRROR
ARG CARGO_MIRROR_REGISTRY
ARG CARGO_BUILD_JOBS
ARG CARGO_NET_RETRY
ARG CARGO_HTTP_TIMEOUT
ARG CARGO_BUILD_PROFILE

ARG USE_CARGO_MIRROR=1
ARG CARGO_MIRROR_REGISTRY="sparse+https://rsproxy.cn/index/"
ARG CARGO_BUILD_JOBS=1
ARG CARGO_NET_RETRY=10
ARG CARGO_HTTP_TIMEOUT=600
ARG CARGO_BUILD_PROFILE=release
ARG CARGO_OUTPUT_DIR=release
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}
ENV CARGO_NET_RETRY=${CARGO_NET_RETRY}
ENV CARGO_HTTP_TIMEOUT=${CARGO_HTTP_TIMEOUT}

RUN mkdir -p /usr/local/cargo \
  && if [ "${USE_CARGO_MIRROR}" = "1" ]; then \
    printf '%s\n' \
      '[source.crates-io]' \
      'replace-with = "mirror"' \
      '' \
      '[source.mirror]' \
      "registry = \"${CARGO_MIRROR_REGISTRY}\"" \
      '' \
      '[http]' \
      'multiplexing = false' \
      > /usr/local/cargo/config.toml; \
  else \
    printf '%s\n' \
      '[http]' \
      'multiplexing = false' \
      > /usr/local/cargo/config.toml; \
  fi

RUN set -e; \
  i=1; \
  while [ "$i" -le 5 ]; do \
    apt-get update -o Acquire::Retries=3 \
      && apt-get install -y --no-install-recommends ca-certificates clang libclang-dev \
      && rm -rf /var/lib/apt/lists/* \
      && break; \
    i=$((i+1)); \
    sleep 3; \
  done

COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY benches/ benches/
COPY proto/ proto/
COPY assets/ assets/
COPY src/ src/

# Local path dependencies (SDK crates) / 本地 path 依赖（SDK crates）
COPY sdk/rust/crates/spear-ssf/ sdk/rust/crates/spear-ssf/

COPY --from=web_builder /work/assets/admin assets/admin
COPY --from=web_builder /work/assets/console assets/console

RUN if [ "${CARGO_BUILD_PROFILE}" = "release" ]; then \
    cargo build --release --locked --no-default-features --features rocksdb,sms-rustfs --bin sms; \
  else \
    cargo build --profile "${CARGO_BUILD_PROFILE}" --locked --no-default-features --features rocksdb,sms-rustfs --bin sms; \
  fi

FROM ${DEBIAN_IMAGE} AS runtime

RUN set -e; \
  i=1; \
  while [ "$i" -le 5 ]; do \
    apt-get update -o Acquire::Retries=3 \
      && apt-get install -y --no-install-recommends ca-certificates libssl3 \
      && rm -rf /var/lib/apt/lists/* \
      && break; \
    i=$((i+1)); \
    sleep 3; \
  done

ARG SMS_WITH_NODE=0
RUN if [ "${SMS_WITH_NODE}" = "1" ]; then \
    set -e; \
    i=1; \
    while [ "$i" -le 5 ]; do \
      apt-get update -o Acquire::Retries=3 \
        && apt-get install -y --no-install-recommends nodejs npm \
        && rm -rf /var/lib/apt/lists/* \
        && break; \
      i=$((i+1)); \
      sleep 3; \
    done; \
    command -v node >/dev/null 2>&1; \
    command -v npx >/dev/null 2>&1; \
  fi

ARG CARGO_OUTPUT_DIR=release
COPY --from=builder /work/target/${CARGO_OUTPUT_DIR}/sms /usr/local/bin/sms
COPY deploy/docker/entrypoints/sms.sh /entrypoint.sh

ENV LD_LIBRARY_PATH=/usr/local/lib

RUN mkdir -p /var/lib/sms /var/lib/sms/files /var/lib/sms/execution_logs /var/log/sms \
  && chown -R 10001:10001 /var/lib/sms /var/log/sms

RUN chmod +x /entrypoint.sh

USER 10001:10001

EXPOSE 50051 8080 8081

ENTRYPOINT ["/entrypoint.sh"]
