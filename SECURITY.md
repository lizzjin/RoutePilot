# Security Policy

ModelPort 默认用于本机或内网环境。它会持有上游 provider API key，因此安全边界非常清楚：不要把服务直接暴露到公网，不要提交真实 `.env`。

## 支持范围

当前维护 `main` 分支。安全修复会优先进入 `main`。

## 报告安全问题

请优先通过 GitHub 私有安全通道报告。如果仓库没有开启 Security Advisories，可以先开一个不包含密钥和攻击细节的 issue，请维护者开启私下沟通渠道。

报告时请提供：

- 影响版本或 commit。
- 复现路径。
- 可能泄露或绕过的内容类型。
- 已确认不会包含真实 API key 的日志片段。

## 本项目的安全原则

- 默认要求 `MODELPORT_AUTH_TOKEN`，投产不要设置 `MODELPORT_ALLOW_NO_AUTH=1`。
- `.env`、`.modelport/`、`target/` 不进入 git。
- 日志只记录路由、状态和 request id，不记录真实 key。
- 建议绑定 `127.0.0.1:17878`，团队使用时放在反向代理和内网鉴权后面。
- 对大请求和大响应设置上限，避免意外内存压力。
