# GitHub 建设建议

ModelPort 的 GitHub 仓库建议按“可长期维护的开发者工具”来建设，而不是临时脚本仓库。

## 仓库基础信息

建议设置：

- Description: `Local Anthropic-compatible model gateway for Claude Code and VS Code Claude.`
- Website: README 中的快速开始或 release 页面。
- Topics: `claude-code`, `anthropic`, `openai-compatible`, `llm-gateway`, `model-router`, `rust`, `vscode`, `mimo`, `deepseek`
- License: MIT

## Branch Protection

对 `main` 开启：

- Require pull request before merging。
- Require status checks to pass。
- 必选检查：`Rust checks`；如果启用前端 CI，再加入 `Dashboard checks`。
- Require branches to be up to date before merging。
- Require conversation resolution before merging。

个人项目也可以允许管理员绕过，但正式投产后建议所有变更都经 PR。

## Actions

当前 CI 位于 `.github/workflows/ci.yml`，覆盖：

- `cargo fmt --all -- --check`
- `cargo test --all-targets`
- `cargo clippy --all-targets --all-features -- -D warnings`

建议补充一个 dashboard job，覆盖：

- `cd dashboard && npm ci`
- `cd dashboard && npm run lint`
- `cd dashboard && npm run build`
- 必要时运行 Playwright E2E；真实 provider 相关 E2E 仍放在本机或受控内网环境。

不要把真实 provider key 放进 GitHub Actions secrets 里跑上游测试。真实上游测试留在本机或受控内网环境执行。

## Release

建议发布节奏：

- `v0.1.x`：本机投产、Claude Code 稳定接入、Mimo/DeepSeek 路径稳定。
- `v0.2.x`：provider 可观测性、benchmark、更多 provider 实测矩阵。
- `v0.3.x`：可选 Responses API / Images API 独立扩展。

每次 release 附带：

- 变更摘要。
- 升级命令。
- 配置变更。
- 已验证 provider。
- 已知问题。

## Issue 和 PR

仓库已经提供：

- Bug report 模板。
- Feature request 模板。
- Pull request 模板。
- CODEOWNERS。

Issue 中禁止粘贴真实 API key、完整 `.env` 和敏感日志。维护者回复问题时也不要要求用户公开密钥。
