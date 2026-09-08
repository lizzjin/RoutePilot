# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.96.0
ARG ROUTEPILOT_VERSION=0.1.0
ARG ROUTEPILOT_SOURCE_REVISION=unknown
ARG ROUTEPILOT_SOURCE_STATE=unknown
ARG ROUTEPILOT_BUILD_DATE=unknown
FROM rust:${RUST_VERSION}-bookworm AS builder

ARG ROUTEPILOT_SOURCE_REVISION
ARG ROUTEPILOT_SOURCE_STATE
ENV ROUTEPILOT_BUILD_REVISION=${ROUTEPILOT_SOURCE_REVISION}
ENV ROUTEPILOT_BUILD_SOURCE_STATE=${ROUTEPILOT_SOURCE_STATE}

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY crates ./crates
COPY ops-agent ./ops-agent

RUN cargo build --release --locked -p routepilot-ops-agent

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*

RUN useradd --system --home /nonexistent --shell /usr/sbin/nologin routepilot-ops \
  && mkdir -p /var/lib/routepilot-ops \
  && chown -R routepilot-ops:routepilot-ops /var/lib/routepilot-ops

COPY --from=builder /app/target/release/routepilot-ops-agent /usr/local/bin/routepilot-ops-agent

ARG ROUTEPILOT_SOURCE_REVISION
ARG ROUTEPILOT_SOURCE_STATE
ARG ROUTEPILOT_VERSION
ARG ROUTEPILOT_BUILD_DATE
LABEL org.opencontainers.image.title="RoutePilot Operations Agent" \
      org.opencontainers.image.description="Deterministic read-only operations agent for RoutePilot" \
      org.opencontainers.image.source="https://github.com/lizzjin/RoutePilot" \
      org.opencontainers.image.revision="$ROUTEPILOT_SOURCE_REVISION" \
      org.opencontainers.image.version="$ROUTEPILOT_VERSION" \
      org.opencontainers.image.created="$ROUTEPILOT_BUILD_DATE" \
      org.opencontainers.image.licenses="MIT" \
      io.routepilot.source-state="$ROUTEPILOT_SOURCE_STATE"

USER routepilot-ops

ENV ROUTEPILOT_OPS_BIND=0.0.0.0:38083
ENV ROUTEPILOT_OPS_BASE_URL=http://routepilot:38082
ENV ROUTEPILOT_OPS_MODE=disabled
ENV ROUTEPILOT_OPS_SPOOL_PATH=/var/lib/routepilot-ops/spool.sqlite
ENV RUST_LOG=routepilot_ops_agent=info

EXPOSE 38083
VOLUME ["/var/lib/routepilot-ops"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD curl -fsS http://127.0.0.1:38083/readyz >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/routepilot-ops-agent"]
