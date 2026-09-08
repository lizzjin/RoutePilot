# RoutePilot P0 基线

改名复拍说明（2026-09-08）：本文关联的部分截图已按全面改名范围从对应版本的页面重新采集。历史对照图保留对应版本布局、视口和演示场景；当前实现截图使用新命名的隔离后端。复拍样本与时间可能变化，不能把新截图视为原日期的验证证据；原截图保留在 Git 历史或改名前工作区备份中。详见改名实施记录及 `docs/assets/routepilot-rename/` 中的复拍清单。
日期：2026-09-07。基线提交：`d8a791fb51d73de36d0a068fa7545e81036a9b3c`。

开始时在 main，只有此前方案交付的未提交文档变更（文档索引、实施方案、参考归档）。这些内容均保留。

- `before-dashboard-light.png`、`before-models-light.png`、`before-logs-light.png`、`before-login-light.png`：1440×900，浅色，现有 mock 数据，Google Chrome 152。
- mock 使用 admin 演示身份；日期、金额、模型和用户均为模拟展示，不是后端验收结果。现有 mock 的随机数据与时间会变化，前后截图用于对比同页面、同尺寸的视觉结构。
- Node.js 24.20.0；原版 `npm run check` 通过：typecheck、lint、29 个测试文件 / 109 项 Vitest、Vite build。
- 页面清单：概览/个人工作台、模型、日志、API Key、用户、配额、账本、治理、运维、设置、指南、登录、404；角色：admin / user / viewer。
- 状态验收范围：加载、首次失败、重试、空结果、后台刷新、刷新失败保留旧数据、角色拒绝；定向覆盖估算/来源、Secret 引用、只读配置、一次性密钥。

环境备注：现有 Dockerfile 未复制 `catalog/`，导致后端编译的 include_str! 找不到 provider-adaptations-v1.json。隔离验收使用临时 Dockerfile 补齐这一输入；没有更改生产 Dockerfile、后端源码或业务配置。验收后端与数据库使用独立容器和网络，网关上游指向容器内关闭的回环端口。
