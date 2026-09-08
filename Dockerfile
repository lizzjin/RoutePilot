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
COPY catalog ./catalog
COPY migrations ./migrations
COPY crates ./crates
COPY ops-agent ./ops-agent

RUN cargo build --release --locked -p routepilot

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && rm -rf /var/lib/apt/lists/*

RUN useradd --system --home /nonexistent --shell /usr/sbin/nologin routepilot
RUN mkdir -p /data /config \
  && chown -R routepilot:routepilot /data /config

COPY --from=builder /app/target/release/routepilot /usr/local/bin/routepilot

# Keep source metadata after dependency and binary layers so a new commit label
# does not invalidate the slow apt or Rust build cache.
ARG ROUTEPILOT_SOURCE_REVISION
ARG ROUTEPILOT_SOURCE_STATE
ARG ROUTEPILOT_VERSION
ARG ROUTEPILOT_BUILD_DATE
LABEL org.opencontainers.image.title="RoutePilot" \
      org.opencontainers.image.description="Self-hosted multi-protocol model gateway" \
      org.opencontainers.image.source="https://github.com/lizzjin/RoutePilot" \
      org.opencontainers.image.revision="$ROUTEPILOT_SOURCE_REVISION" \
      org.opencontainers.image.version="$ROUTEPILOT_VERSION" \
      org.opencontainers.image.created="$ROUTEPILOT_BUILD_DATE" \
      org.opencontainers.image.licenses="MIT" \
      io.routepilot.source-state="$ROUTEPILOT_SOURCE_STATE"

USER routepilot

ENV ROUTEPILOT_BIND=0.0.0.0:38082
ENV ROUTEPILOT_STATE_DIR=/data
ENV ROUTEPILOT_CONFIG=/config/config.toml
ENV RUST_LOG=routepilot=info,tower_http=info

EXPOSE 38082
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:38082/livez >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/routepilot"]
