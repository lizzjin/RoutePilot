# syntax=docker/dockerfile:1

ARG ROUTEPILOT_VERSION=0.1.0
ARG ROUTEPILOT_SOURCE_REVISION=unknown
ARG ROUTEPILOT_SOURCE_STATE=unknown
ARG ROUTEPILOT_BUILD_DATE=unknown
FROM node:24-bookworm-slim AS builder

WORKDIR /app
COPY dashboard/package.json dashboard/package-lock.json ./
RUN npm ci --no-audit --no-fund

COPY dashboard ./
ARG VITE_API_BASE_URL=
ENV VITE_API_BASE_URL=${VITE_API_BASE_URL}
RUN npm run build

FROM nginx:1.30.4-alpine AS runtime

ARG ROUTEPILOT_VERSION
ARG ROUTEPILOT_SOURCE_REVISION
ARG ROUTEPILOT_SOURCE_STATE
ARG ROUTEPILOT_BUILD_DATE
LABEL org.opencontainers.image.title="RoutePilot Dashboard" \
      org.opencontainers.image.description="Same-origin administration dashboard for RoutePilot" \
      org.opencontainers.image.source="https://github.com/lizzjin/RoutePilot" \
      org.opencontainers.image.revision="$ROUTEPILOT_SOURCE_REVISION" \
      org.opencontainers.image.version="$ROUTEPILOT_VERSION" \
      org.opencontainers.image.created="$ROUTEPILOT_BUILD_DATE" \
      org.opencontainers.image.licenses="MIT" \
      io.routepilot.source-state="$ROUTEPILOT_SOURCE_STATE"

COPY deploy/docker/dashboard.nginx-main.conf /etc/nginx/nginx.conf
COPY deploy/docker/dashboard.nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

USER nginx

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD wget -qO- http://127.0.0.1:8080/ >/dev/null || exit 1
