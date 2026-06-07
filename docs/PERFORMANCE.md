# 性能与效率

结论：ModelPort 目前的中转效率足够支撑 Claude Code / VS Code Claude 的本机长期使用，也足够支撑内网小团队试生产。它不是重型代理，不做数据库写入、复杂策略引擎或二次推理；主要开销来自一次本机 HTTP hop、JSON 协议转换和上游网络请求。

## 为什么足够快

- HTTP 入口使用 Axum/Tokio，异步处理请求。
- 上游使用 reqwest/rustls 原生客户端，不依赖系统 `curl` 子进程。
- 上游连接池复用 TCP/TLS 连接，减少重复握手。
- 非流式响应只做必要 JSON 映射。
- 流式响应按 SSE chunk 转换并转发，不等待完整回答结束。
- 请求体、响应体和并发都有上限，避免异常请求拖垮本机服务。

## 真正的瓶颈在哪里

通常不在 ModelPort，而在：

- 上游模型排队和生成速度。
- 第三方中转服务的网络质量。
- Claude Code 一次任务发起的上下文大小。
- 流式 provider 是否重放文本片段。
- 本机网络代理、防火墙或 DNS。

## 如何测

本机网关基准：

```bash
scripts/bench.sh
```

真实上游基准会产生模型调用成本，默认只跑 3 次：

```bash
scripts/bench.sh --upstream
```

调整次数：

```bash
scripts/bench.sh -n 100
scripts/bench.sh --upstream -n 5
```

建议关注：

- `/health`：本机服务和进程调度开销。
- `/v1/models`：本机鉴权、路由配置和 JSON 响应开销。
- `/v1/messages`：真实上游耗时，主要反映 provider 质量。

## 投产调优

常用参数：

```bash
MODELPORT_MAX_CONCURRENT_REQUESTS=64
MODELPORT_HTTP_CONNECT_TIMEOUT_SECS=10
MODELPORT_HTTP_REQUEST_TIMEOUT_SECS=600
MODELPORT_HTTP_STREAM_IDLE_TIMEOUT_SECS=300
MODELPORT_HTTP_MAX_RESPONSE_BYTES=33554432
```

建议：

- 本机 Claude Code 使用保持 `MODELPORT_BIND=127.0.0.1:17878`。
- 团队使用放到内网反向代理后面，不要直接公网暴露。
- 上游响应慢时先换 provider 或线路，再考虑调大超时。
- 大上下文任务优先观察上游 token 限制和价格。
- 图像生成类能力不要混入 `/v1/messages` 主路径，因为 base64 图片会显著放大响应体。
