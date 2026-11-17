ARG RUST_VERSION=1.91
ARG DEBIAN_SUITE=bookworm
ARG RUST_IMAGE=rust:${RUST_VERSION}-bookworm
ARG DEBIAN_IMAGE=debian:${DEBIAN_SUITE}-slim
ARG USE_CARGO_MIRROR=1
ARG CARGO_MIRROR_REGISTRY=sparse+https://rsproxy.cn/index/
ARG CARGO_NET_RETRY=10
ARG CARGO_HTTP_TIMEOUT=600
ARG CARGO_BUILD_PROFILE=release
ARG CARGO_OUTPUT_DIR=release

FROM ${RUST_IMAGE} AS builder

ARG USE_CARGO_MIRROR
ARG CARGO_MIRROR_REGISTRY
ARG CARGO_NET_RETRY
ARG CARGO_HTTP_TIMEOUT
ARG CARGO_BUILD_PROFILE

ENV CARGO_NET_RETRY=${CARGO_NET_RETRY}
ENV CARGO_HTTP_TIMEOUT=${CARGO_HTTP_TIMEOUT}

WORKDIR /app

# Reuse the same Cargo mirror strategy as other local Rust images. / 复用其他本地 Rust 镜像的 Cargo 镜像策略。
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

# Install CA certificates in the builder stage before Cargo downloads crates. / 在 Cargo 下载 crates 前，于 builder 阶段安装 CA 证书。
RUN set -e; \
  i=1; \
  while [ "$i" -le 5 ]; do \
    apt-get update -o Acquire::Retries=3 \
      && apt-get install -y --no-install-recommends ca-certificates \
      && rm -rf /var/lib/apt/lists/* \
      && break; \
    i=$((i+1)); \
    sleep 3; \
  done

COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY benches ./benches
COPY assets ./assets
COPY sdk ./sdk
COPY src ./src
COPY proto ./proto

RUN if [ "${CARGO_BUILD_PROFILE}" = "release" ]; then \
    cargo build --release --locked --no-default-features --features debug-server-bin --bin debug-server; \
  else \
    cargo build --profile "${CARGO_BUILD_PROFILE}" --locked --no-default-features --features debug-server-bin --bin debug-server; \
  fi

FROM ${DEBIAN_IMAGE} AS runtime

RUN set -e; \
  i=1; \
  while [ "$i" -le 5 ]; do \
    apt-get update -o Acquire::Retries=3 \
      && apt-get install -y --no-install-recommends ca-certificates \
      && rm -rf /var/lib/apt/lists/* \
      && break; \
    i=$((i+1)); \
    sleep 3; \
  done

WORKDIR /app
ARG CARGO_OUTPUT_DIR=release
COPY --from=builder /app/target/${CARGO_OUTPUT_DIR}/debug-server /usr/local/bin/debug-server

EXPOSE 7777

ENTRYPOINT ["/usr/local/bin/debug-server"]
