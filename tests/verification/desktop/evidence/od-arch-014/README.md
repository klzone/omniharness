# OD-ARCH-014 证据索引：以主工作区为基线选择性整合 UI 外壳

- 采集时间：2026-10-18（Asia/Shanghai）
- 采集者：BUILDER
- 对应计划：[PLANS.md](../../../../../PLANS.md) 的 **OD-ARCH-014**
- 工作区：`/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`
- 结论范围：**只证明可丢弃 UI 外壳可接入，并保留主工作区既有功能**。不构成正式产品化批准；模型端点继续 no-go。

---

## 0. 执行前状态与基线

| 项 | 事实 |
| --- | --- |
| 主工作区 HEAD | `5ffc710`（分支 `main`） |
| 开工时未提交改动 | 约 59–68 项（含未跟踪），全部保留，未重置/覆盖/清理 |
| UI 参考工作区 | `/mnt/data/Code/omniharness-ui`，**只读**，未修改、未整体合并 |
| `runtime/` | 未触碰；`git diff --exit-code HEAD -- runtime/` 退出码 0 |
| 端点准入 | `ENDPOINT_ISOLATION_CONFIRMED = false`，未改 |
| 数据 | 仅合成样例 |

### 计划偏差（必须记录）

计划第 1 步的停止条件命中：UI 工作区的 `docs/omnidesk-ui-v2-handoff.md` 仍写着「已包含主工作区成果」
「两侧文件基本不重叠」，与事实不符，且该文档无人更正。

处理：依据用户明确授权 OD-ARCH-014，且 UI 工作区只读，**以主工作区为唯一基线选择性继续**，
不修改交接文档。本项不是无瑕疵执行，需 ARCHITECT 复核该判断是否可接受。

---

## 1. 文件来源与改动

### 从 UI 工作区复制（逐个选取，非分支合并）

- `desktop/src/theme.css`
- `desktop/src/workbench-ui.css`
- `desktop/src/engineAdmission.ts`
- `desktop/src/components/Icon.tsx`
- `desktop/src/shell/TopNav.tsx`
- `desktop/src/shell/ThreeWeekCalendar.tsx`
- `desktop/src/shell/EngineTechSlice.tsx`
- `desktop/public/mascot.png`
- `scripts/dev-ui.sh`、`scripts/lib/port-owner.sh`

### 主工作区新建

- `desktop/src/shell/Dock.tsx`：场景 `home / workbench / offline / engine`，只列已实现入口
- `desktop/src/shell/HomeDashboard.tsx`：首页仪表盘；**没有第二套引擎对话入口**

### 主工作区修改

- `desktop/package.json`：+Tailwind、+3 个 `@fontsource` 字体依赖
- `desktop/vite.config.ts`：接入 `@tailwindcss/vite`
- `desktop/src/main.tsx`：先字体与 `theme.css`，再 `styles.css`
- `desktop/src/styles.css`：全部原有规则包进 `@layer legacy { … }`，内容未删
- `desktop/src/App.tsx`：顶栏 + Dock + 场景区；引擎诊断改为只读覆盖层；删除旧自由文本提交面板
- `desktop/src/api.ts`：仅新增准入拒绝（`currentEngineSubmitDecision`）+ 相关导出，离线 API 未丢
- `desktop/src/workbench.tsx`：以 UI 版本覆盖（与 UI 副本 `diff` 完全一致）
- `desktop/src/OfflineFlows.tsx`：根容器加 `ofl-v2` 作用域类（本次新增）
- `desktop/src/offline-flows.css`：**本次新增**，离线流程局部浅色样式
- `desktop/src/shell/EngineTechSlice.tsx`：空事件提示从 `text-inverse-on-surface/50` 提到 `/80`

---

## 2. 本次修复的两个可见缺陷

| 缺陷 | 现象 | 修复 | 证据 |
| --- | --- | --- | --- |
| 离线流程样式破损 | `styles.css` 进入 `@layer legacy` 后，浅色外壳覆盖旧深色规则，出现深底深字、按钮被压成小色块，功能在但读不清 | 新增 `offline-flows.css`，只作用于 `.ofl-v2` 子树，恢复浅色卡片/输入/按钮/结果区 | `offline-flows.png`、`offline-writing.png` |
| 选中标签悬停不可读 | `.tab:hover` 优先级高于 `.tab.active`，悬停已选按钮时文字变暗 | 增加 `.ofl-v2 .tab.active:hover:not(:disabled)` 保持高对比 | `offline-writing.png`（悬停态截图） |
| 引擎诊断空态字太淡 | 深色事件区里的空态提示对比度不足 | 由 50% 提到 80% 不透明度 | `engine-readonly.png` |

未改文案、未改功能、未隐藏任何边界提示。

---

## 3. 静态与构建验证（实际运行）

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| `cd desktop && npx tsc --noEmit` | 退出码 0，无错误输出 | `tsc.log`（空即通过） |
| `cd desktop && npm run build` | 退出码 0；CSS 69.71 kB、JS 278.63 kB；字体本地打包 | `build.log` |
| `cargo test --offline`（先 `setup-desktop-toolchain.sh --print-env`） | lib **72 通过**、host_integration **13 通过**、e2e **3 通过**、doc 0；0 失败 | `cargo-test.log` |
| `cargo run --offline --example evaluate_offline_scenarios` | 退出码 0；三场景通过；准入拒绝生效；日期边界正确 | `offline-example.log` |
| `git diff --check HEAD` | 退出码 0（无空白错误） | 见 STATUS.md |
| `git diff --exit-code HEAD -- runtime/` | 退出码 0（Runtime 无差异） | 见 STATUS.md |

`package-lock.json` 因新增依赖已更新（`npm install --no-audit --no-fund`）。

---

## 4. 真实窗口交互验证（Xvfb :96，合成端点，隔离数据根）

启动：

```
setsid Xvfb :96 -screen 0 1440x1000x24 &
DISPLAY=:96 OMNIDESK_DEV_ROOT=/tmp/od014-dev scripts/dev-ui.sh --no-build
```

运行目录 `/tmp/od014-dev/od-dev-87ca1cd8`；App 通过 `scripts/dev-ui.sh --stop` 回收
（app 1、mock 1、vite 3 含子进程），未误杀其它实例。

| 场景 | 实际操作 | 可观察结果 | 证据等级 | 截图 |
| --- | --- | --- | --- | --- |
| 首页渲染 | 打开窗口 | 顶栏、三周日历、吉祥物、三张入口卡；无第二套对话入口 | 运行（真实窗口） | `home.png` |
| 任务工作台 | 点 Dock 进入 | 五阶段骨架完整，默认任务类型为「脱敏数据分析与生态研判」 | 运行 | `workbench.png` |
| 确定性统计 | 载入合成样例 → 运行统计 | 行数 5、合计 204、平均 40.8、最高 51、指纹 `320dae745102…` | 运行 | `stats.png` |
| 引擎准入拒绝 | 阶段 3 点「用引擎生成解释」 | 前端拒绝，提示输入未发送到任何模型；宿主常量仍为 false | 运行 | `admission.png` |
| 人工确认后保存 | 手写解释 → 勾选确认 → 保存 | 写入 `/tmp/od014-dev/od-dev-87ca1cd8/local-data/20261018-035213-320dae745102.md`，1742 字节；未覆盖同名 | 运行 | `saved.png` |
| 离线周报流程 | 载入合成周记录 → 本地处理 | 4 条记录：完成 2 / 待完成 1 / 受阻 1；明确「模型步骤：受准入阻塞、未验证」 | 运行 | `offline-flows.png` |
| 离线文案结构 | 切换到「一般文案结构」 | 表单可读可填；选中标签悬停仍高对比 | 运行 | `offline-writing.png` |
| 引擎诊断（只读） | 顶栏设置按钮打开 | 三栏只读；无输入框；显示端点未确认、引擎未启动、未创建任务 | 运行 | `engine-readonly.png` |

---

## 5. 未验证 / 已知限制

- **未做模型输出验证**：端点准入保持关闭，全部模型步骤均为 blocked；没有任何真实模型结果。
- **仅本机 Linux + Xvfb 验证**：未验证 Windows/macOS，也未验证真实显示器合成器下的渲染差异。
- **未验证长清单滚动性能**：事件流、大结果文本只做了空态与少量数据。
- **交接文档仍不准确**：UI 工作区 `docs/omnidesk-ui-v2-handoff.md` 未更正（只读，未改）。
- **未提交、未推送、未打标签**：全部改动保留在工作区，等待 ARCHITECT 审查。
- **未写 REVIEW.md**：按角色分工留给 ARCHITECT。

---

## 6. 复现步骤

1. `cd desktop && npx tsc --noEmit`
2. `cd desktop && npm run build`
3. `eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)" && cd desktop/src-tauri && cargo test --offline`
4. `cargo run --offline --example evaluate_offline_scenarios`
5. 启动隔离窗口：`DISPLAY=:96 OMNIDESK_DEV_ROOT=/tmp/od014-dev scripts/dev-ui.sh --no-build`
6. 逐项点击上表路径；结束用 `scripts/dev-ui.sh --stop`，再关闭 Xvfb。
