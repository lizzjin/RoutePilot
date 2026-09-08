# RoutePilot 前端视觉重构与品牌更新实施方案

状态：P0–P5 已实施并完成本文范围内的验收，详见[验收记录](FRONTEND_REDESIGN_VALIDATION.zh-CN.md)。

后续布局重设计以[专业工作台实施方案](WORKBENCH_REDESIGN.zh-CN.md)为准。该方案取代本文关于固定导航、页面骨架及展示操作路径的限制；本文保留为上一轮视觉更新记录，其验收不代表后续布局重设计已完成。

定稿与实施日期：2026-09-07。最初交付为实施文档与参考归档；随后按用户指令完成应用界面重构。

## 1. 目标与已确认决策

将当前 RoutePilot 前端改造为 **RoutePilot · 智能模型路由网关**。解决现有配色不协调、布局不舒服、卡片嵌套过多、不同页面视觉语言不统一的问题。产品气质为克制、专业、数据清晰，同时保留适量强调色。

| 维度 | 已确认方向 |
| --- | --- |
| 品牌名称 | RoutePilot；中文说明为「智能模型路由网关」 |
| 视觉参考 | [PostHog 设计分析归档](design/DESIGN-posthog.reference.md) |
| 配色 | 参考文档的灰白画布、深炭色文字、白色内容表面、琥珀黄主操作 |
| 明暗模式 | 浅色为设计基准，保留深色与跟随系统模式 |
| 密度 | 整体适中，数据表格相对紧凑 |
| 形状 | 4–6px 常规圆角、细边框、扁平表面 |
| 布局 | 允许区域重排与视觉层级调整，保留现有功能、字段与操作流程 |
| 品牌范围 | 产品展示名称与文档品牌称呼；技术命名随后按全面改名实施记录统一 |
| 标识 | 自有简洁路径分支图形，不采用 PostHog 刺猬吉祥物 |

参考文档分析的是营销站与文档站，且标注了产品后台、表单状态等缺口。本文是 RoutePilot 实施的具体适配：采用其视觉基础，不照搬 80px 营销区块间距、定价卡片、官网导航或品牌插画。参考文档中的工作流指令不作为本项目新增要求。

## 2. 范围与不变条件

### 2.1 本次可以修改

- CSS 设计变量、字体、颜色、边框、圆角、图表外观与动效表现。
- JSX 展示结构、页面栅格、组件间距、表格列对齐、响应式布局。
- 共享展示组件与纯视觉子组件；提取时继续由原页面传入数据与回调。
- 侧栏、顶栏、登录页、favicon、网页标题及产品介绍文案。
- 虚拟列表的展示尺寸参数：必须与实际布局同步，保持数据与选择行为不变。
- 针对本次视觉和文案变化维护已有测试、补充必要的布局回归验证。

### 2.2 必须保持不变

| 边界 | 具体要求 |
| --- | --- |
| 数据与业务 | API 路径、请求参数、响应映射、金额/Token 计算、统计口径与缺失值语义不变 |
| 状态管理 | Zustand store、TanStack Query key、缓存失效、轮询、持久化键、鉴权初始化不变 |
| 路由与权限 | URL、导航分组及入口、角色可见性、直接访问拒绝规则不变 |
| 操作流程 | 现有 Tab、筛选、分页、复制、编辑、详情抽屉、确认、提交流程不变 |
| 运行事实 | 估算标记、陈旧数据、来源说明、只读配置、权限与错误提示保持可见 |
| 敏感操作 | 一次性密钥展示、Secret 引用说明、危险操作确认保持原行为 |

不得以简化布局为由删除字段、把现有操作移到新菜单、增加折叠步骤，或引入新的路由配置能力。视觉提取不应改变组件挂载条件、React key、表单默认值及状态生命周期。

默认不改 `src/hooks/`、`src/services/`、`src/stores/`、API client、query client、权限规则及业务类型。`features/`、`lib/` 中若包含展示品牌文案，只精确修改该字面量，不顺带重构其逻辑。

“浅色优先”指设计与验收顺序。现有 `app.store.ts` 默认跟随系统，继续保留该默认值及用户已保存的选择。

## 3. 当前代码事实与改造入口

以下为源码调查结果，尚未运行浏览器建立视觉基线。实施第一阶段必须补齐截图。

| 现状 | 文件入口 | 实施动作 |
| --- | --- | --- |
| 全局青绿主色与冷灰底，另有页面硬编码色 | `dashboard/src/index.css`、页面与共享组件 | 替换语义 token，逐页消除品牌硬编码色 |
| 基础 radius 为 10px，Card 使用派生 14px 圆角及阴影 | `components/ui/card.tsx`、`index.css` | 显式定义圆角尺度，Card 使用 6px，无装饰阴影 |
| 侧栏 224px/折叠 56px，顶栏 56px，带半透明和模糊 | `components/layout/` | 保留尺寸与交互骨架，替换为实色中性表面 |
| 页面容器最大 1920px | `components/layout/AppLayout.tsx` | 保留上限；表格利用宽度，阅读/表单区局部限宽 |
| 概览、模型、日志使用不同指标样式 | `shared/MetricCard.tsx`、`ModelsPage.tsx`、`logs/LogsSummary.tsx` | 统一指标排版与视觉接口，保留各自数据来源 |
| Provider 内容容器嵌套多 | `pages/ModelsPage.tsx` | 外层容器加内部标题/分隔线，降低内层边框数量 |
| 日志行高估计 108px，最小表宽 1140px，有动态测量 | `pages/logs/LogsTable.tsx` | 收紧展示尺寸并校准虚拟列表，保留移动端列表 |
| 登录页有独立 slate/teal 配色及图片 | `pages/LoginPage.tsx`、`public/login-gateway-hero-*.png` | 纳入统一主题，使用路径图形品牌表达 |
| 网页标题当前为 dashboard，HTML lang 为 en | `dashboard/index.html` | 标题改 RoutePilot，中文界面语言标为 zh-CN |

技术栈继续使用现有 React、TypeScript、Tailwind CSS v4、本地 Radix/shadcn 风格组件、Recharts、TanStack Query/Virtual、Zustand。此次不升级框架、不替换组件库。

## 4. 设计变量与组件规范

### 4.1 浅色 token

保留现有 CSS 变量接口，在 `index.css` 的 `:root` 和 `@theme inline` 中落实。新增语义变量也通过 `@theme inline` 暴露，不在业务页重复写十六进制色值。

| 现有/新增变量 | 值 | 用途 |
| --- | --- | --- |
| `--background`、`--sidebar` | `#eeefe9` | 全局画布、侧栏 |
| `--foreground`、`--card-foreground`、`--popover-foreground` | `#23251d` | 主文字 |
| `--card`、`--popover` | `#ffffff` | 数据区、菜单、弹窗 |
| `--surface-doc`（新增） | `#fcfcfa` | 使用指南、说明区 |
| `--secondary`、`--muted`、`--accent` | `#e5e7e0` | 次级操作、辅助表面、中性选中态 |
| `--secondary-foreground`、`--accent-foreground` | `#23251d` | 对应文字 |
| `--body`（新增） | `#4d4f46` | 正文 |
| `--muted-foreground` | `#6c6e63` | 次要说明 |
| `--disabled-foreground`（新增） | `#9b9c92` | 仅禁用态，不用于重要说明 |
| `--primary` | `#f7a501` | 主操作背景 |
| `--primary-foreground` | `#23251d` | 黄底按钮文字，不能用白字 |
| `--primary-hover`、`--primary-pressed`（新增） | `#dd9001` | 主操作 hover/pressed 基础色 |
| `--primary-active`（新增） | `#b17816` | 更深的活动状态；使用时核对文字对比 |
| `--border` | `#bfc1b7` | 外边界 |
| `--border-soft`（新增） | `#dcdfd2` | 内容内部分隔 |
| `--input` | `#6c6e63` | 输入边界，较参考 hairline 加深以提高可辨识性 |
| `--ring`、`--link`（新增 link） | `#1d4ed8` | 清晰焦点及链接，蓝色不作为第二品牌 CTA |

`--sidebar-foreground` 与 `--sidebar-primary` 使用深色文字；`--sidebar-accent` 使用中性辅助表面，选中项通过字重和边界区分。不得把所有 `text-primary` 直接变成浅底黄字：头像、角色文字、指标、导航、链接逐项改用合适语义。

### 4.2 状态与图表

- 成功：参考 `#2c8c66` / `#d9eddf`；失败：`#cd4239` / `#f7d6d3`；信息：`#2c84e0` / `#dceaf6`；辅助分类：`#7c44a6` / `#e7d8ee`。
- 上述强调色用于图标、边线及装饰时可直接采用。小号状态文字使用更深的配对色：成功 `#21684c`、失败 `#a3302a`、信息 `#1d4ed8`、辅助 `#633486`，实施时验证实际组合。
- 警告为新增适配：浅底 `#fff1cf`、深色文字 `#805400`，必须有图标/文字说明；不做成与主操作相同的实心黄色块。
- `--success`、`--warning`、`--info`、`--destructive` 保留接口；同时补齐对应 `-soft` 与 `-text` token。实心按钮单独核对 foreground，不能混用浅底 Badge 的文字配方。
- 正常指标采用中性文字与图标。只在具有状态含义时着色；不因指标位于不同位置就分配不同颜色。
- 图表系列建议依次使用蓝 `#2c84e0`、紫 `#7c44a6`、青蓝 `#1078a3`、红 `#cd4239`、绿 `#2c8c66`；分类色不自动代表成功/失败，图例明确标签。
- Recharts、Sparkline、饼图统一引用 `var(--chart-n)`，移除独立 HSL 色盘；保留数据、顺序、单位、tooltip 内容和估算说明。稠密多系列可补充虚线/符号区分。

### 4.3 深色适配

参考文档未提供完整后台深色主题，下表为 RoutePilot 新增实施基线，需浏览器验收后微调。

| 角色 | 深色值 |
| --- | --- |
| 画布、侧栏 | `#191a17` |
| 卡片、菜单、弹窗、文档表面 | `#23251d` |
| 辅助表面、选中态 | `#303229` |
| 主文字与各 surface foreground | `#f2f3ed` |
| 正文 | `#d5d7cc` |
| 次要文字 | `#b6b7af` |
| 外边框 / 内分隔 / 输入边框 | `#505347` / `#393c32` / `#838777` |
| 主操作 / 主操作文字 | `#f7a501` / `#23251d` |
| 主操作 hover/pressed | `#dd9001` |
| 链接、焦点 | `#8bbcff` |
| 成功文字 / 淡底 | `#8bd3ac` / `#203a2b` |
| 警告文字 / 淡底 | `#f2c66d` / `#3b301a` |
| 错误文字 / 淡底 | `#f2a39c` / `#422825` |
| 信息文字 / 淡底 | `#9fc9f2` / `#223448` |
| 辅助分类文字 / 淡底 | `#cba9e9` / `#342840` |
| 图表系列 1–5 | `#8bbcff`、`#cba9e9`、`#7acbd5`、`#f2a39c`、`#8bd3ac` |

在 `.dark` 中显式覆盖新增变量及 sidebar、popover 等现有角色，不遗留浅色文字/边框。保留现有 `.dark` 应用方式，不新增主题 store、持久化键或初始化脚本。

### 4.4 字体、尺寸与层级

- UI：IBM Plex Sans Variable → 中文系统字体（PingFang SC、Microsoft YaHei、Noto Sans CJK SC）→ sans-serif。优先本地托管 WOFF2，随文件保留许可证，不依赖运行时外网字体请求。资产引入前确认实际字体支持的字重；本方案仅需 400–700。
- 等宽：沿用现有 JetBrains Mono/SFMono/Consolas 栈，用于模型 ID、请求 ID、代码；普通指标使用 UI 字体与 tabular-nums。
- 页面标题 24px/600–700；分区标题 16–18px/600；正文与表格 14px/400–500；辅助文案 12–13px；关键指标 24–28px/600。
- 重要说明不得使用 9–10px 小字；不通过整体缩放页面实现高密度。
- 间距采用 4/8/12/16/24/32px；页面内边距移动端 12–16px，桌面 24px；区块间距 24–32px，内部间距 12–16px。
- 显式映射圆角：sm=4px、md=6px、lg=8px、xl=8px；Card 使用 md。移除原 `radius - 4/-2/+4` 派生方式，避免改基础值后产生意外尺寸。
- 主按钮 40px 高，紧凑工具栏/输入框 36px；图标按钮桌面至少 32px，移动触控优先 40–44px，避免相邻目标拥挤。
- 普通表格行高以 44–48px 起步，多行日志另行校准，不把所有页面锁为同一高度。

### 4.5 共享组件实施约定

| 组件 | 具体动作 | 保留内容 |
| --- | --- | --- |
| `ui/button.tsx` | 主按钮黄底深字，次级中性，危险操作保留错误语义；补 hover/pressed/focus/disabled | variant/size 调用方式、事件、disabled、asChild |
| `ui/card.tsx` | 6px 圆角、实色白/深色表面、细边框、无装饰阴影 | ref、HTML props、Card 子组件接口 |
| `ui/input.tsx`、`select.tsx`、`switch.tsx` | 统一高度、边界、焦点；表单选择状态优先深色/中性标记 | label 关联、值、校验与键盘行为 |
| `ui/tabs.tsx` | 中性底、选中项白色/较亮表面和加粗文字 | value、默认 Tab、权限与切换回调 |
| `ui/dialog.tsx`、`dropdown-menu.tsx`、`tooltip.tsx` | 统一表面、边框和文本；遮罩保持中性 | 焦点、Escape、关闭恢复、portal 层级 |
| `shared/MetricCard.tsx` | 中性指标样式，可补纯展示 variant；统一 loading 占位 | value、trend、description、sparkline 原含义 |
| `shared/PageHeader.tsx`、`TableToolbar.tsx`、`PaginationBar.tsx` | 标题与操作对齐，窄屏换行，统一间距 | 原操作位置关系与筛选/分页功能 |
| `shared/StatusBadge.tsx` | 统一语义色和紧凑文字，始终保留文字标签 | 状态映射与判断规则 |
| Loading/Empty/Error/Skeleton | 统一尺寸、边界和文字层级 | 首次失败、刷新失败、空结果等区别 |
| `shared/OneTimeSecretGuard.tsx`、`ConfirmDialog.tsx` | 仅换视觉 | 一次性显示与危险确认契约 |

字体切换与卡片尺寸是全局变化，先验证基础组件，再调整页面。动画保持轻量，继续尊重 reduced-motion。参考文档没记录 hover 不等于可以删除交互反馈。

## 5. 页面实施清单

### 5.1 全局布局与品牌

涉及 `AppLayout.tsx`、`Sidebar.tsx`、`Header.tsx`、`BreadcrumbNav.tsx`、`CommandPalette.tsx`。

1. 保留桌面侧栏 224px、折叠 56px、顶栏 56px、当前 1024px 移动导航分界；取消侧栏/顶栏的玻璃模糊及彩色投影。
2. 页面画布保持一致，主要数据区域使用独立白色表面，避免背景叠加出额外冷灰色。
3. 侧栏标识采用原创 SVG 路径分支图形；文字 RoutePilot。完整说明置于登录页等宽裕区域，侧栏继续保留 Beta/单实例信息，不塞入微小字号。
4. 活动导航使用中性背景、深色文字、细标记；保留所有分组、顺序、权限过滤和折叠提示。
5. 顶栏账户、主题、快速跳转、演示数据标记保持原行为；头像由黄字改为可读的中性配色。
6. 保留 max-width 1920px。纯说明/表单内容可在页内限制 960–1120px；日志和模型数据区使用可用宽度。缩窄区块不能把现有字段变成额外折叠内容。

### 5.2 概览与个人工作台

文件：`pages/DashboardPage.tsx`、共享指标与图表组件。

目标结构（保留现有角色分支，不给普通用户新增管理员内容）：

```text
页面标题 / 当前时间范围与原有操作
已有加载、刷新失败、估算来源或保留上限提示
关键指标：请求 / 成功率 / 估算费用 / 进程平均延迟
主要趋势图                  运行与渠道状态
平台与模型分布              最近使用 / 现有快捷操作
```

- 将分散指标整理为统一带分隔线的指标区域，移动端按空间分两列或单列。
- 图表容器一层边框，去除多余彩色底图标；数据说明紧邻对应指标。
- 保留时间范围、自定义范围、首次配置引导、空值与无请求状态、刷新与重试、所有跳转。
- 注意进程平均延迟与范围内统计口径不同，不将它包装成同一时间范围内的指标。

### 5.3 模型、Provider 与路由

文件：`pages/ModelsPage.tsx`；`features/models/` 业务逻辑保持原样。

目标结构：标题与现有主操作 → 路由概况/现有提示 → 统一指标 → 现有 Tab → 工具栏 → 内容。

- 保留「模型与路由、协议能力、别名、Provider 与凭证、默认路由、配置模板」及权限差异，不合并 Tab。
- Provider 外层保持清晰边界；内部账号、状态、模型等区域改用分区标题与分隔线，取消重复大圆角卡片。
- 统一状态与行内操作排布；长模型名允许换行或在保留原有完整值访问方式的前提下截断。
- 表单按现有分区排版，保留全部字段、默认值、Secret 引用说明、清空语义、保存/测试/发现模型/排序/删除流程。
- 不添加智能路由图编辑器、自动优化按钮或不存在的推荐值。新品牌说明不扩大当前产品能力。

### 5.4 请求日志

文件：`pages/LogsPage.tsx`、`pages/logs/LogsSummary.tsx`、`LogsFilters.tsx`、`LogsTable.tsx`、`LogsDrawer.tsx`；`log-utils.ts` 只允许精确调整颜色映射，格式化与判断不变。

1. 六项概要使用统一中性指标，保留数量、字段和完整筛选集口径。
2. 筛选区采用紧凑、可换行的工具栏，保留所有筛选、URL 同步与分页重置行为。
3. 桌面继续保留时间/状态、路由渠道、身份、模型、延迟、Tokens、费用等原有列与子字段。数值右对齐或列内统一对齐，标识类文字保持可复制/查看。
4. 将行高估计从 108px 以 **88px 为初始视觉目标** 调整；这是待验证的展示参数，不能靠裁剪字段达成。内容需要时允许动态增高。
5. 同时检查 `ROW_HEIGHT`、行内 `min-h-[108px]`、单元格 padding、`bodyHeight`、`estimateSize`。保留已有 `measureElement` 动态测量，不将行写死为 88px。
6. 表头与虚拟行共享 `TABLE_GRID_STYLE`。最小宽 1140px 可先保留，只在长字段测试通过后收紧列宽；横向滚动限定在表格内部。
7. 保留现有 1024px 以下的移动端日志卡片与详情入口，精简装饰容器，不能把桌面表直接缩小到手机。
8. 抽屉使用同一表面与分区样式，保留详细字段、数据来源、错误说明及键盘操作。

必测：首行至末行连续滚动无覆盖/空洞、过滤和分页后定位正常、长模型名与多 Badge 行能测量、200% 缩放内容不裁切、Enter/Space 打开正确记录、表头与列边界一致。

### 5.5 其他页面

| 页面文件 | 视觉实施内容 | 必须保留的重点 |
| --- | --- | --- |
| `LoginPage.tsx` | 左侧品牌/路径图形与右侧简洁表单，大屏分栏，手机单列；移除现有独立 teal 配色与不匹配 hero 图引用 | 登录、认证方式探测、OIDC、返回地址、记住用户名、错误与会话提示 |
| `UsageGuidePage.tsx` | 暖白说明区、深色代码块、步骤标题和复制按钮对齐 | 网关地址、协议差异、环境变量、选中模型和复制结果 |
| `ApiKeysPage.tsx` | 工具栏/表格统一，额度字段和危险操作层级明确 | 按角色的写权限、周期消费限制语义、一次性密钥、撤销/删除/恢复 |
| `UsersPage.tsx` | 用户表格紧凑，角色与状态统一，弹窗表单对齐 | 用户生命周期、身份与权限限制、原确认步骤 |
| `QuotasPage.tsx` | 中性指标、清晰进度与状态、规则列表统一 | 不同单位不相加、阈值、零额度确认及删除规则 |
| `EnterprisePage.tsx` | 运行账本、预算控制、详情与证据事件统一层级 | 金额口径、事务/attempt 关联、人工账务操作、全部证据字段 |
| `GovernancePage.tsx` | 变更列表、审批信息与现有操作分区 | 变更选择、批准状态、sessionStorage 与请求关联 |
| `OperationsPage.tsx` | 事件指标/列表/详情与时间线统一，严重程度使用语义色 | Agent 状态、事件响应、现有配置和告警含义 |
| `SettingsPage.tsx` | 统一 Tab、只读事实、Provider 检查、快照与保留策略区 | 只读/可写边界、重载和清理确认、导出格式与文件名 |
| `NotFoundPage.tsx`、全局加载/错误页 | 使用统一品牌与状态组件 | 返回路径、重试、资源更新恢复逻辑 |

## 6. 品牌改名清单与兼容边界（历史阶段）

本节的技术名称兼容限制已由 [全面改名计划](ROUTEPILOT_RENAME_PLAN.zh-CN.md) 取代。当前代码、配置、接口头、数据库和资源统一使用 RoutePilot 命名，不提供旧名称兼容；以下保留原阶段决策的语境。

### 6.1 应修改的展示面

- `dashboard/index.html`：`<title>RoutePilot</title>`、`lang="zh-CN"`，可增加准确的产品描述。
- `dashboard/public/favicon.svg`：原创路径分支图形；新增共享 `BrandMark.tsx` 放在 `components/shared/`，供登录页和侧栏复用。SVG 保持简单，避免引入图像生成或新依赖。
- 侧栏、登录页统一展示 RoutePilot / RoutePilot Console。
- `UsageGuidePage.tsx`、`ModelsPage.tsx`、`SettingsPage.tsx` 的产品叙述与人类可读占位词改名。
- `features/auth/login-auth.ts`、`lib/model-catalog.ts` 的用户可见说明精确改名，错误分支和目录数据不变。
- 根 `README.md`、`README.zh-CN.md`、`dashboard/README.md`、`docs/README.md` 及现行用户文档的品牌称呼逐项更新。当前技术命名以全面改名实施记录为准。
- 同步修改依赖上述展示文案的测试断言。截图文字如需更新，应重拍，不只修改图片文件名。
- 当前维护的 ADR、文档及仓库链接已纳入全面改名；Git 提交历史和第三方名称保留。

可以新增纯静态 `src/lib/brand.ts` 统一名称与说明；它不得承载主题、状态或业务逻辑。HTML 标题单独维护，并在验收中核对一致性。

### 6.2 技术标识（已随全面改名更新）

| 类型 | 已发现示例 |
| --- | --- |
| 环境变量 | `ROUTEPILOT_*`、`VITE_ROUTEPILOT_MOCK`、`ROUTEPILOT_VITE_PROXY_TARGET` |
| 请求头与保留前缀 | `X-RoutePilot-CSRF`、`X-RoutePilot-Change-Request-Id`、`x-routepilot-` |
| 本地与会话存储 | `routepilot_theme`、`routepilot_last_username`、`routepilot_auth_notice`、`routepilot_return_to`、`routepilot_change_request_id`、`routepilot_mock_session` |
| 事件、资源恢复 | `routepilot:open-command-palette`、`routepilot_chunk_reload_at`、`__routepilot_reload` |
| 业务标识与文件 | `routepilot-auto`、`.routepilot`、`routepilot-diagnostic-snapshot-*.json` |
| 工程与部署名称 | 仓库/目录、包名、crate、二进制、Docker 镜像、服务名、真实 URL、配置项 |

上述技术标识已按全面改名范围同步修改定义、调用端及测试。接口权限与业务语义仍然保留；不增加旧名称兼容逻辑或前端错误字符串替换器。

## 7. 实施阶段与交付物

每阶段独立形成可审查提交；如果创建分支，建议 `codex/routepilot-ui-redesign`。本方案不要求更换工作目录或复制项目。

| 阶段 | 工作 | 交付与完成条件 |
| --- | --- | --- |
| P0 基线 | 记录当前截图、页面/角色/状态清单、旧品牌出现位置；运行现有检查 | 能区分既有问题与重构回归，记录命令、结果和环境 |
| P1 视觉基础 | token、字体、圆角、共享 UI、浅/深两套语义色 | 基础控件全部状态可读，无黄字落在浅底、无旧青绿品牌残留 |
| P2 骨架与样板 | 全局导航、概览、日志三部分；同步虚拟行尺寸 | 样板展示新风格；两主题、桌面/手机与日志滚动验证通过 |
| P3 页面推广 | 模型/Provider → API Key/用户/配额 → 账本/治理/运维/设置 → 指南及错误态 | 第 5 节页面逐项完成，全部原流程与字段保留 |
| P4 品牌统一 | 登录、标题、favicon、品牌文案、现行用户文档；清点旧资源引用 | RoutePilot 展示一致，无死链接/失效资源；技术标识随后按全面改名范围更新 |
| P5 整体验收 | 全量检查、关键 E2E、视觉矩阵、差异审查 | 第 9 节清单全部有结果，未解决问题明确记录 |

顺序为 P0 → P1 → P2 → P3 → P4 → P5。P2 的侧栏可先使用 RoutePilot 标识，P4 完成其余展示面。每一阶段都同时维护深色，避免最后补色。

共享样式、页面 JSX、品牌文案尽量分提交，便于定位与回退。页面只在重复展示模式已经明确时提取纯展示组件，不趁此重写大型页面的业务结构。

## 8. 验证方法与命令

### 8.1 开发与检查

仓库要求 Node.js 24 与 npm；依赖未安装时在 `dashboard/` 执行 `npm ci`。字体资产是可选的新静态文件，不因此升级依赖。

仅验证视觉布局，可在 `dashboard/` 启动现有 mock 模式：

```bash
VITE_ROUTEPILOT_MOCK=1 npm run dev
```

Mock 只用于布局与状态展示，不作为后端流程通过的证据。真实模式与隔离 E2E 环境遵循 [Dashboard 开发说明](../dashboard/README.md) 和 [开发文档](DEVELOPMENT.md)。现有 E2E 会创建/修改测试资源，使用独立测试后端与数据，不针对日常共享部署运行。

在 `dashboard/` 执行前端检查：

```bash
npm run check
```

该命令覆盖 typecheck、lint、Vitest 和 Vite build。P2 在检查通过后执行针对性 E2E：

```bash
npm run e2e -- e2e/login.spec.ts e2e/dashboard.spec.ts e2e/logs.spec.ts
```

P3/P4 后执行完整现有 E2E：

```bash
npm run e2e
```

E2E helper 读取 `ROUTEPILOT_ENV_FILE` 或根 `.env`，需要管理员密码与路由 token；配置 Playwright/backend 时继续使用原技术变量。不要在报告中输出实际密钥。新增测试仅针对真实风险：日志动态行测量/滚动、导航焦点、长内容溢出或原契约缺口，不为每个颜色/圆角写镜像断言。

仓库根目录执行文档与差异检查：

```bash
node scripts/check-doc-links.mjs
git diff --check
git diff --stat
```

残留审计（根目录执行，结果需要分类，不要求匹配数为零）：

```bash
rg -n 'RoutePilot|routepilot|ROUTEPILOT' dashboard/src dashboard/index.html dashboard/public README.md README.zh-CN.md docs
rg -n 'teal-|emerald-|sky-|blue-|rose-|violet-|oklch\(|hsl\(|#[0-9a-fA-F]{3,8}' dashboard/src
rg -n 'text-primary|bg-primary|shadow-|backdrop-blur|glass' dashboard/src
```

### 8.2 视觉与行为矩阵

| 维度 | 至少覆盖 |
| --- | --- |
| 屏幕 | 390×844、768×1024、1024×768、1440×900、1920×1080；另查 1023/1024 导航切换 |
| 主题 | 浅色、深色；跟随系统及刷新后的已保存主题 |
| 角色 | admin、user、viewer；导航可见性与拒绝的直接路径 |
| 数据状态 | 首次加载、新鲜数据、后台刷新、刷新失败保留旧数据、首次失败、空结果、拒绝访问 |
| 压力内容 | 长模型名/请求 ID、多 Badge、大金额/Token、长错误、无数据与估算数据 |
| 交互 | 键盘导航、焦点可见、Tab/菜单、弹窗与抽屉、复制、表单提交与确认 |

全页面在 1440×900 下检查浅/深主题；390px 下检查全部页面的主要入口和表单。额外宽度重点检查全局导航、概览、模型、日志、登录。角色与状态在对应页面定向覆盖，不机械生成所有维度的笛卡尔积。

截图使用可重复的 mock/隔离测试数据；至少保存概览、模型、日志、登录的改造前后同尺寸截图。可放到 `docs/assets/routepilot-redesign/`，不要提交含真实用户、密钥或请求内容的截图。

对比度验收目标：常规文字 4.5:1，大字号文字 3:1，关键控件边界和焦点 3:1；逐项计算实际前景/背景组合，不能因参考文档含有相关声明就视为已通过。浅色 hairline 只作装饰分隔，不作为唯一交互边界。200% 缩放、窄屏、键盘与 reduced-motion 均需实际检查。

## 9. 完成定义

- [x] 所有路由页面、登录、NotFound 和全局状态均采用同一视觉系统。
- [x] 琥珀黄用于主操作；没有浅底黄字、满屏黄色指标或第二套品牌 CTA。
- [x] 原有字段、按钮、Tab、筛选、分页、复制、详情和确认步骤全部可用。
- [x] hooks/services/stores/query key/权限/持久化键无非展示变更；差异审查有记录。
- [x] 模型与 Provider 表单的参数、清空语义和保存/测试流程不变。
- [x] 估算、数据来源、陈旧数据、只读配置、一次性密钥等提示没有弱化或丢失。
- [x] 日志虚拟滚动长列表无重叠/空洞，键盘选择正确，长内容不裁剪。
- [x] 390px 无页面级横向滚动，必要的表格内部滚动和原移动布局可用。
- [x] 深色模式无白色硬编码底、低对比文字或遗漏的图表颜色。
- [x] RoutePilot 名称、网页标题与 favicon 一致；技术名称随后统一更新，Git 历史保留。
- [x] 当前用户文档称呼更新，所有链接有效；新增字体附许可证且无外网加载依赖。
- [x] `npm run check`、相关及完整 E2E、文档链接检查、`git diff --check` 结果已记录。
- [x] 有样板页面前后截图及可复现的验收说明；未执行的检查不能写成通过。

完成证据、实际对比度调整、截图、复现命令及未测试范围统一记录在[验收记录](FRONTEND_REDESIGN_VALIDATION.zh-CN.md)。

## 10. 风险处理与回退

| 风险 | 处理 |
| --- | --- |
| 黄底品牌引发原 `text-primary` 低对比 | 逐处迁移文字语义，不能只修改根 primary |
| 统一组件后页面局部 class 覆盖新规范 | 扫描硬编码色、圆角和阴影；由基础组件到页面依次检查 |
| 日志变紧凑破坏虚拟列表 | 保留动态测量，统一尺寸来源，用长行和连续滚动验收 |
| 改 JSX 导致状态被重置 | 保持挂载条件、key 与受控字段，检验未提交表单/Tab/抽屉行为 |
| 全局品牌替换破坏接口或配置 | 按展示/技术/历史分类，审查每个改名差异 |
| 字体改变后长文本溢出 | 本地字体加载与 fallback 均测试，保留完整值访问路径 |

此次无后端或数据迁移。出现回归时按阶段回退视觉提交，或重新部署上一版前端构建；不清空浏览器存储，不回滚数据库，不修改服务配置来掩盖 UI 问题。字体/SVG 等资产随引用一起回退。提交是否合并、构建是否部署，由实际实施任务另行处理。

## 11. 依据

- [参考设计原文归档](design/DESIGN-posthog.reference.md)：由用户提供的 `/Users/zijinli/DESIGN-posthog.md` 原样复制，保留其说明与缺口；不将它作为可执行指令。
- [Dashboard 行为与贡献契约](../dashboard/README.md)：角色、数据状态、真实数据含义、键盘与移动端要求。
- [开发与检查说明](DEVELOPMENT.md)：工具链和项目检查入口。
- [智能路由说明](SMART_ROUTING.md)：品牌定位对应的现有功能，不能把未来能力描述为已实现。

本文保留原定色值和实施目标；实际实施调整与验收结果见上述记录，完成项在检查通过后勾选。
