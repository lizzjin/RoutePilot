# RoutePilot 工作台原型与 P0 基线

改名复拍说明（2026-09-08）：本文关联的部分截图已按全面改名范围从对应版本的页面重新采集。历史对照图保留对应版本布局、视口和演示场景；当前实现截图使用新命名的隔离后端。复拍样本与时间可能变化，不能把新截图视为原日期的验证证据；原截图保留在 Git 历史或改名前工作区备份中。详见改名实施记录及 `docs/assets/routepilot-rename/` 中的复拍清单。
日期：2026-09-08。基线提交：`b847b0b948968b6ddde95611809fc2e9a6099159`，分支：`codex/routepilot-ui-redesign`。

用户已于 2026-09-08 确认当前原型可作为后续全页面改造基准。本文保留 P0/P1 当时的结构、字段清单和原型验证记录；其中“占位”“待接回”均为原型阶段的历史状态。正式页面实施和验收见[全页面改造验收记录](WORKBENCH_REDESIGN_VALIDATION.zh-CN.md)。

## 原型归档与预览

已将获批原型归档到本地分支 `codex/routepilot-workbench-prototype`，提交 `55f507de324fd819488e3a0d856d02415baf2762`。该分支提供 `prototype:workbench` 和 `prototype:baseline` 脚本；正式改造分支已移除原型入口与演示组件，复用真实页面、权限、查询和写入流程。

本文下面的“新”截图为获批原型，“旧”截图为 P0 基线，不代表当前产品截图。当前实现截图位于 `assets/workbench-redesign/implementation/`。正式页面本地演示启动方式：

```bash
VITE_ROUTEPILOT_MOCK=1 npm --prefix dashboard run dev -- --host 127.0.0.1 --port 33012
```

演示登录 `admin / admin`，仅使用 mock 数据。真实后端验证另用隔离测试服务。

## 建议体验路径

1. 运行概览：先看运行判断和右侧异常队列，点击“进入请求排障”或某渠道的排查入口。后者携带现有日志筛选所支持的 Provider 与错误状态。
2. 日志：输入搜索、切换渠道/结果、连续选中两条记录、分页，展开次要字段。桌面列表与详情同屏；390px 下进入详情后用“返回请求列表”返回，筛选与分页保留。
3. 配置：选择 Provider，再选择其模型。编辑 Provider 的名称、端点、Secret 环境变量引用、默认模型和目录；模拟保存或取消。编辑未保存时切换工作区、对象或浏览器后退会提示处理修改。
4. 使用底部工具条检查正常样例、空数据、首次加载失败、刷新失败和拒绝访问；使用顶部主题选择检查浅色、深色及跟随系统。
5. 将演示角色改为普通用户/只读用户：管理员页面不出现在导航，直接进入管理员路由显示拒绝访问；不出现 Provider 写入入口。

## 新旧结构对照

截图视口固定为 1440 × 1000，核心三页另采集 1024 × 1000 和 390 × 844。新旧使用同一仓库 mock 数据，通过固定随机种子 `20260908` 与 `Date.now = 1788854400000` 采集，验证脚本比较三组数据的完整 JSON 相等。该时间只用于固定演示快照，不代表真实后端时间。日常手动打开时 mock 的时间序列仍会重新生成。

| 页面 | 旧版 | 新原型 | 结构变化 |
| --- | --- | --- | --- |
| 概览 | [旧](assets/workbench-redesign/before-dashboard-1440.png) | [新](assets/workbench-redesign/after-dashboard-1440.png) | 移除贯穿全页的左导航；运行判断与异常入口提前；趋势/运行情况为主区，异常集中旁区；低频用量成本折叠 |
| 日志 | [旧](assets/workbench-redesign/before-logs-1440.png) | [新](assets/workbench-redesign/after-logs-1440.png) | 原宽表与临时抽屉改为结果/路由/耗时队列和常驻详情，身份与计费细节后置，连续选择不遮挡列表 |
| 配置 | [旧](assets/workbench-redesign/before-models-providers-1440.png) | [新](assets/workbench-redesign/after-models-1440.png) | 原横向 Tab 和 Provider 卡片改为对象目录、模型子项、上下文事实和内联编辑区 |

[窄屏概览](assets/workbench-redesign/after-dashboard-390.png)、[窄屏日志详情](assets/workbench-redesign/after-logs-390.png)、[窄屏配置详情](assets/workbench-redesign/after-models-390.png)。

## P0 全路由功能去向清单

此表是产品实现阶段的保留清单，不能将原型中的占位解释为已实现。源码字段依据已采集到[基线字段索引](assets/workbench-redesign/source-inventory.json)：覆盖 13 个页面的 JSX 文案/控件及 107 个领域类型声明。动态内容、嵌套对象和实际 API 字段以所链接源码及领域类型为准，P3/P4 每页实施时继续核对。

权限：`A` 为管理员；`U` 为普通用户；`V` 为只读用户。当前访问策略来自 `App.tsx`、`NAV_ITEMS` 和 `ProtectedRoute.tsx`，不扩张既有权限。

| 路由 / 角色 / 基线截图 | 必须保留的数据与操作 | 目标位置 / 当前实现状态 |
| --- | --- | --- |
| `/dashboard` A/U/V · [基线](assets/workbench-redesign/before-dashboard-1440.png) | 时间范围及自定义区间、请求/成功率、估算费用、进程延迟、Token 拆分/吞吐/密钥/缓存、请求与 Token 趋势、Provider 健康、模型分布、最近调用、接入检查和快捷入口；来源、估算和保留上限提示 | 运行摘要/主趋势/异常旁区/辅助统计。原型展示固定 24 小时快照；完整范围选择、Token 趋势、密钥、接入检查在 P3 接回 |
| `/logs` A/U/V · [基线](assets/workbench-redesign/before-logs-1440.png) | 搜索、用户/密钥/模型/Provider/分组/状态/流式/Tool Use/流量分类/时间过滤；完整结果摘要、分页/页大小、自动刷新；请求 ID、路由/协议、身份、缓存、Token/计费拆分、对账证据、延迟、重试、结束原因、Tool Use、重构协议摘要 | 筛选区/结果队列/常驻详情。原型可搜索/按状态和渠道筛选，20 条分页，全部演示字段可展开；其余筛选、汇总、自动刷新、虚拟化和详情 API 在 P3 接回 |
| `/models` A/U/V（管理 A）· [基线](assets/workbench-redesign/before-models-1440.png) | Provider 身份/协议/端点/Secret 引用、模型目录/前缀/透传、Token 字段、能力与兼容配置、流式策略和请求头；Provider 生命周期、模型发现/测试/画像、账号凭证生命周期、别名、全局默认 Provider/解析顺序、配置模板；删除依赖与确认 | 对象目录/事实与编辑/上下文路由和账号。原型支持 Provider 基础字段模拟编辑与模型选择检查；其余操作明确占位；完整高级字段仍可在配置事实中阅读 |
| `/operations` A · [基线](assets/workbench-redesign/before-operations-1440.png) | Agent 启用/本地优先/基础模型/分析开关；事件级别/状态/范围/恢复条件/出现次数/时间、证据、时间线、状态变更依据、规则反馈、Agent 心跳和队列 | 事件队列/处理区域，Agent 配置辅助区。当前导航占位 |
| `/enterprise` A · [基线](assets/workbench-redesign/before-enterprise-1440.png) | 账本过滤、请求身份及生命周期、Attempts、幂等/租约、Token/成本/对账；预算上限/控制、正负调整、证据引用和调整原因、事件 | 账本联动详情/预算与证据局部分区。当前导航占位 |
| `/settings` A · [基线](assets/workbench-redesign/before-settings-1440.png) | 绑定地址、认证、请求体/并发/超时/速率等只读事实；Provider 就绪/测试；运维审计、热加载、诊断快照导出、保留策略执行及确认。诊断快照不能还原完整备份 | 分类目录/事实阅读区/运维操作区。当前导航占位，保持只读语义 |
| `/api-keys` A/U/V · [基线](assets/workbench-redesign/before-api-keys-1440.png) | 名称/分组、用户/团队、主体/用途/有效期、模型/上游/IP 范围、状态/总与周期费用限额、轮换关联/使用；签发/编辑/撤销/删除/恢复/轮换及一次性明文交接；A 全管理，U 仅所属密钥允许自助操作，V 无写入 | 密钥目录/上下文详情与操作，明文交接保留独立受保护步骤。当前导航占位 |
| `/users` A · [基线](assets/workbench-redesign/before-users-1440.png) | 搜索/角色/状态过滤、账号/邮箱/密码/角色/状态、创建/最后登录、密钥和请求数量；创建/编辑/禁用等生命周期、用户密钥列表及签发、一次性保存保护 | 用户目录/当前用户/关联密钥及操作。当前导航占位 |
| `/quotas` A · [基线](assets/workbench-redesign/before-quotas-1440.png) | 用户、类型、周期、限额、用量/重置时间、限制原因；筛选、创建、调整、删除、零限额确认；不同单位不相加 | 配额对象队列/规则编辑与解释。当前导航占位 |
| `/governance` A · [基线](assets/workbench-redesign/before-governance-1440.png) | 项目策略、审批门禁/存储、变更类型/目标/精确 JSON/原因/载荷摘要、申请人/审批人/时间/状态、批准与应用；高风险动作、交互和后台队列/边界 | 审批队列/变更内容与操作，治理状态辅助区。当前导航占位 |
| `/guide` A/U/V · [基线](assets/workbench-redesign/before-guide-1440.png) | API Key 选择、真实可见模型目录、客户端协议/代码示例、日志验证、安全说明、管理员首次接入流程 | 步骤目录/当前接入任务/对应示例。当前导航占位 |
| `/login` 公开 · [基线](assets/workbench-redesign/before-login-1440.png) | 认证能力、密码登录、企业 SSO（启用时）、字段校验与错误反馈、品牌/接入说明 | 独立认证构图，P4 实施。原型入口不模拟真实认证 |
| `*` 公开 · [基线](assets/workbench-redesign/before-not-found-1440.png) | 404 定位与返回入口；与权限拒绝区分 | 工作区返回路径，P4 实施。原型提供未找到位置的说明及返回入口 |

所有真实数据页还需保留：初始加载、刷新、刷新失败后的陈旧数据、首次失败、无数据/无筛选结果、拒绝访问。基线 mock 中账本、治理、事件部分数据为空；这些截图只能证明当前空状态骨架，不能替代真实数据场景基线或后续验收。

## 验证与限制

验证结果另见[机器可读记录](assets/workbench-redesign/verification.json)。浏览器交互检查通过：连续选择、筛选分页、编辑保存/取消、导航与浏览器后退离开保护、viewer 导航/直接访问限制、窄屏返回焦点、场景及主题切换；无页面 JavaScript 异常。

本轮执行前端 `npm run check`（类型、lint、109 个已有测试和生产构建）。原型未添加正式产品测试；浏览器脚本用于本次交互检查与截图，不替代 P2–P5 的 E2E。

本机 Node 为 22.22.3；项目维护基线为 Node 24。本次检查在当前环境通过，但未据此宣称完成 Node 24 CI 验证。

本原型的问题是“顶部工作区、运行摘要与异常旁区、日志列表/详情、Provider 目录/编辑是否适合实际工作”。按已确认实施方案，只实现这一方向，不重新提出三套全局导航方案。

尚未完成：用户结构确认、全页面产品改造、真实后端/上游联调、真实账号切换缓存隔离、一次性密钥交接回归、日志虚拟化和全字段筛选、所有真实加载/错误组合、全路由深浅色响应式验收。原型角色切换仅验证界面，不是授权机制；非管理员 Provider 字段可见性仍需在 P3 严格复用真实 catalog API 的边界。

原型仅作为可抛弃实现保留在当前工作分支；尚未有胜出确认，不将临时代码提升为生产实现。收到结构反馈后记录确认范围，再按 P2/P3/P4 扩展。
