# Contributing to ModelPort

感谢参与 ModelPort。这个项目的目标是保持轻量、可靠、容易本机投产：优先服务 Claude Code / VS Code Claude 的 Anthropic-compatible 文本链路，再逐步扩展常用 provider。

## 开发环境

```bash
git clone git@github.com:tiammomo/ModelPort.git
cd ModelPort
cp .env.example .env
scripts/install-deps-ubuntu.sh
scripts/check.sh
```

`.env` 只放本机真实密钥，已经被 `.gitignore` 忽略。不要把真实 API key、token、日志、`.modelport/` 或 `target/` 提交进仓库。

## 提交前检查

每个改动至少跑：

```bash
scripts/check.sh
scripts/doctor.sh
```

涉及真实 provider、流式转换、鉴权或路由行为时，再跑：

```bash
scripts/doctor.sh --upstream
scripts/smoke-test.sh --upstream
```

涉及 Docker 时跑：

```bash
docker build -t modelport:local .
```

## 代码约定

- 优先沿用现有模块边界：`config` 负责配置与路由，`routes` 负责 HTTP 入口，`providers` 负责协议转换，`http` 负责上游传输。
- provider 扩展优先走 Anthropic-compatible 或 OpenAI-compatible，不急着引入 provider native API。
- 流式行为要加测试，尤其是 SSE 分片、错误事件、重复 chunk 和 tool call 参数。
- 不在日志中输出 API key、完整 Authorization header、完整 base64 图片或大请求体。
- README 面向使用者，`docs/` 面向维护者和长期建设。

## Pull Request

PR 描述请说明：

- 改了什么行为。
- 对 Claude Code / VS Code Claude 的影响。
- 运行过哪些验证命令。
- 是否涉及密钥、鉴权、日志、上游兼容或费用风险。
