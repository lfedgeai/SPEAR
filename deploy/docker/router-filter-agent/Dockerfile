# Router filter agent container image / Router filter agent 容器镜像

ARG RUST_VERSION=1.91
ARG DEBIAN_SUITE=trixie
FROM rust:${RUST_VERSION}-bookworm AS builder

WORKDIR /work

ARG USE_CARGO_MIRROR=1
ARG CARGO_MIRROR_REGISTRY="sparse+https://rsproxy.cn/index/"

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

RUN apt-get update -o Acquire::Retries=3 \
  && apt-get install -y --no-install-recommends clang libclang-dev \
  && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY benches/ benches/
COPY static/ static/
COPY proto/ proto/
COPY assets/ assets/
COPY src/ src/

RUN cargo build --release --locked --bin keyword-filter-agent

FROM debian:${DEBIAN_SUITE}-slim AS runtime

RUN apt-get update -o Acquire::Retries=3 \
  && apt-get install -y --no-install-recommends ca-certificates libssl3 \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /work/target/release/keyword-filter-agent /usr/local/bin/keyword-filter-agent
COPY --from=builder /work/target/release/build/wasmedge-sys-*/out/standalone/WasmEdge-*/lib64/ /usr/local/lib/
COPY deploy/docker/entrypoints/router-filter-agent.sh /entrypoint.sh

ENV LD_LIBRARY_PATH=/usr/local/lib

RUN chmod +x /entrypoint.sh

USER 10001:10001

ENTRYPOINT ["/entrypoint.sh"]
