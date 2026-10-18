ARG RUST_VERSION=1.91
ARG DEBIAN_SUITE=bookworm
ARG RUST_IMAGE=rust:${RUST_VERSION}-bookworm
ARG DEBIAN_IMAGE=debian:${DEBIAN_SUITE}-slim
ARG CARGO_NET_RETRY=10
ARG CARGO_HTTP_TIMEOUT=600
ARG CARGO_BUILD_PROFILE=release
ARG CARGO_OUTPUT_DIR=release

FROM ${RUST_IMAGE} AS builder

ARG CARGO_NET_RETRY
ARG CARGO_HTTP_TIMEOUT
ARG CARGO_BUILD_PROFILE

ENV CARGO_NET_RETRY=${CARGO_NET_RETRY}
ENV CARGO_HTTP_TIMEOUT=${CARGO_HTTP_TIMEOUT}

WORKDIR /app
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
