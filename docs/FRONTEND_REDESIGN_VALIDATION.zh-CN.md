# RoutePilot 前端重构验收记录

改名复拍说明（2026-09-08）：本文关联的部分截图已按全面改名范围从对应版本的页面重新采集。历史对照图保留对应版本布局、视口和演示场景；当前实现截图使用新命名的隔离后端。复拍样本与时间可能变化，不能把新截图视为原日期的验证证据；原截图保留在 Git 历史或改名前工作区备份中。详见改名实施记录及 `docs/assets/routepilot-rename/` 中的复拍清单。
日期：2026-09-07。对应[实施方案](FRONTEND_REDESIGN.zh-CN.md)，基线 `d8a791fb51d73de36d0a068fa7545e81036a9b3c`，实施分支 `codex/routepilot-ui-redesign`。

## 交付范围

全局导航、11 个登录后页面、登录和 404 已统一采用 RoutePilot 视觉系统：中性实色表面、琥珀黄主操作、紧凑边框、统一明暗主题和本地 IBM Plex Sans。概览采用指标条和优先展示的趋势图；日志保留动态行测量；Provider 编辑器按字段组分段，并使长表单内部滚动、操作区保持可达。原生 SVG 路径标志、网页标题、favicon 和当前用户文档名称同步更新。

业务边界复核：`dashboard/src/hooks/`、`services/`、`stores/`、API client、query client、业务类型、路由定义和权限判定没有修改。`ProtectedRoute.tsx` 仅修改拒绝访问图标颜色。`features/auth/login-auth.ts` 与 `lib/model-catalog.ts` 仅替换展示品牌字面量。`pages/logs/log-utils.ts` 仅替换样式映射，状态分支、阈值与费用计算不变。Provider 参数、清空处理、保存/测试请求和 Secret 引用逻辑保留。

原视觉改造阶段保留技术名称的约束已由全面改名实施记录取代。当前环境变量、存储键、事件、API Header、诊断下载文件名及工程部署路径均已统一命名。截图使用演示数据；一次性密钥和真实请求数据未写入验收资产。

## 验收证据

| 检查 | 结果与覆盖 |
| --- | --- |
| `npm run check` | typecheck、lint、29 个文件 / 109 项 Vitest、生产构建通过 |
| 完整 Playwright E2E | 26/26 通过，含原有 18 项与新增 8 项 |
| 样板专项 E2E | 登录、概览、日志 6/6 通过 |
| 页面与浮层检查 | [134 个状态](assets/routepilot-redesign/visual-audit.json)，页面横向溢出与浏览器运行错误为 0；axe-core 4.13.0 的 WCAG 2/2.1 A/AA 自动检查违规为 0 |
| 对比度 | [46/46 个实际语义颜色组合](assets/routepilot-redesign/contrast-audit.json)达标：文字至少 4.5:1，控件边界和焦点至少 3:1 |
| 原生缩放与减少动效 | [28 个状态](assets/routepilot-redesign/zoom-motion-audit.json)通过；Chromium 原生 tab zoom=2，1440px 视口实际重排为 720 CSS px；全部页面明暗主题无页面级横向溢出，Provider 保存操作可见，减少动效偏好生效 |
| 文档与差异 | 52 个文档文件的链接检查、`git diff --check` 通过；品牌与硬编码样式残留按展示/技术/历史分类复核 |

页面矩阵包含 390×844 和 1440×900 下全部页面的浅色/深色状态；768×1024、1024×768、1920×1080 重点覆盖概览、模型、日志、登录。另检查 1023/1024 导航断点、Models 六个 Tab、Settings 五个 Tab、API Key/用户/配额创建表单、Provider 编辑器、日志抽屉三个 Tab 和概览下方图表。

新增 E2E 使用真实隔离后端完成登录及 admin/user/viewer 角色边界验证；覆盖导航隐藏、直接路径拒绝、写入口限制、主题持久化与系统主题、手机导航焦点循环/Escape/恢复、字体失败回退、未提交表单状态保持、表单操作区可达。原有 E2E 继续覆盖 Provider 管理、用户和 Key 编辑/清理、一次性 Key 揭示、配置重载和写保护等流程。

日志压力测试用 205 条合成记录、每页 200 条，包含长模型名/请求 ID、多状态标记、长错误与大金额/Token。字体加载及动态测量稳定后连续滚动，检查相邻行无重叠/空洞、表头列对齐、最后记录的 Enter/Space 选择、详情绑定、分页与筛选重置。加载/首次失败/重试/空结果/自动刷新/失败保留旧数据由可控网络夹具逐项验证；这些夹具不代表真实上游服务验证。

自动无障碍检查有其覆盖范围；这里同时检查了键盘交互、焦点、原生缩放和截图。验收浏览器为 Chromium；未把未执行的 Safari/Firefox、真实外部 Provider 调用或部署验证计入通过结果。

## 验收中修正的问题

- 方案中的浅色次级文字 `#6C6E63` 在部分背景不足 4.5:1，调整为 `#626459`；成功实色填充以及 warning/info 实色前景也按实际对比度调整。品牌琥珀黄保留，用于按钮和标志，浅底文字使用深色语义。
- 手机导航关闭时设为 inert，开启时约束焦点，Escape 后恢复至入口；表格与代码滚动区域可用键盘访问。
- 指南窄屏网格、日志长数字及行高、表头对齐得到修正，信息提示和完整值访问方式保留。
- Provider/模型适配长表单改为 flex 布局，标题与操作区不收缩，字段区内部滚动；390px 和原生 200% 缩放下可访问保存/取消。
- 账本搜索输入恢复统一控件边界；详情粘性标题采用实色；代码复制按钮和日志时间线移除遗留白色硬编码。

## 前后截图

均为 1440×900、浅色、模拟数据。模拟时间和记录可能不同，对比目标是布局和视觉结构。

| 页面 | 改造前 | 改造后 |
| --- | --- | --- |
| 概览 | [前](assets/routepilot-redesign/before-dashboard-light.png) | [后](assets/routepilot-redesign/after-dashboard-light-1440.png) |
| 模型 | [前](assets/routepilot-redesign/before-models-light.png) | [后](assets/routepilot-redesign/after-models-light-1440.png) |
| 日志 | [前](assets/routepilot-redesign/before-logs-light.png) | [后](assets/routepilot-redesign/after-logs-light-1440.png) |
| 登录 | [前](assets/routepilot-redesign/before-login-light.png) | [后](assets/routepilot-redesign/after-login-light-1440.png) |

补充示例：[深色概览](assets/routepilot-redesign/after-dashboard-dark-1440.png)、[手机 Provider 表单](assets/routepilot-redesign/after-provider-form-dark-390.png)、[原生 200% 缩放表单](assets/routepilot-redesign/after-provider-form-dark-native-zoom-200.png)。完整文件与逐状态结果位于同一资产目录。

## 复现

使用 Node.js 24，依赖按 lockfile 安装。常规验证在 `dashboard/` 执行：

```bash
npm run check
npm run e2e -- --reporter=list
node scripts/check-redesign-contrast.mjs
```

E2E 需要按 [开发文档](DEVELOPMENT.md)准备隔离后端，并设置 `ROUTEPILOT_ENV_FILE`、`ROUTEPILOT_VITE_PROXY_TARGET`、`PLAYWRIGHT_BASE_URL`。本次使用独立 PostgreSQL/后端容器、网络和临时凭据，上游指向容器内关闭的回环端口；验收后已清理这些容器、网络和凭据。既有 Dockerfile 缺少编译所需 `catalog/` 输入；本次临时 Dockerfile 补齐复制步骤，生产 Dockerfile 与后端源码未修改，详见[基线](assets/routepilot-redesign/BASELINE.md)。

视觉验证只连接本地 mock。首先启动：

```bash
VITE_ROUTEPILOT_MOCK=1 npm run dev -- --port 33012
```

另一个终端在 `dashboard/` 运行以下命令。将 axe-core 4.13.0 包内的 `axe.min.js` 放到本地路径，再设置 `AXE_SOURCE`；若省略该变量，只生成截图与布局结果，不能宣称已执行 axe 检查。

```bash
AXE_SOURCE=/absolute/path/to/axe.min.js node scripts/capture-redesign.mjs
node scripts/check-redesign-zoom.mjs
```

脚本默认访问 `http://127.0.0.1:33012`，可用 `REDESIGN_MOCK_URL` 指定其他本地端口；登录后必须出现“演示数据”标记。原生缩放脚本使用一次性浏览器配置和临时扩展调用 Chromium `tabs.setZoom`，结束时自动移除。并行运行 E2E 时使用 list reporter，避免 Vite 监听到 HTML 报告写入而触发页面重载。

仓库根目录检查：

```bash
node scripts/check-doc-links.mjs
git diff --check
git diff d8a791f -- dashboard/src/hooks dashboard/src/services dashboard/src/stores \
  dashboard/src/lib/api-client.ts dashboard/src/lib/query-client.ts
```

最后一条命令应无输出。字体来源和许可证见 [fonts README](../dashboard/public/fonts/README.md)。
