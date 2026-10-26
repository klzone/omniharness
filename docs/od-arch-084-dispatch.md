# OD-ARCH-084 派工指令：会话内搜索的命中定位、逐处切换与焦点保持

> 依据 `docs/BUILDER-Task-Instruction-Template.md` 撰写。计划正文以 `PLANS.md` 的 OD-ARCH-084 小节为唯一实施口径；本文只负责把范围、顺序、证据与禁止项说清楚，不放宽任何一条。

## 获批依据

- 计划位置：`PLANS.md` → OD-ARCH-084
- 批准时间与批准人：2026-10-25，用户明确批复「批准执行084」
- 开工基线：分支 main，HEAD `0d17b1f`，工作区干净，`git status --porcelain runtime/` 为 0
- 允许修改：`desktop/src/shell/MarkdownText.tsx`、`desktop/src/shell/HomeChat.tsx`、`desktop/src/shell/TopNav.tsx`、`desktop/src/App.tsx`、`desktop/src/shell/HomeCanvas.tsx`（仅透传）、`desktop/src/markdown.css`、`desktop/src/theme.css`、新增 `tests/verification/desktop/verify_ui_084.py`、`tests/verification/desktop/evidence/od-arch-084/`、`STATUS.md`、`PLANS.md`（仅状态与执行偏差）
- 明确不做：不改 `runtime/`；不动 Profile、凭证、模型路由、DSH 数据根；不改 `verify_ui_082.py` / `verify_ui_083.py`；不覆盖 081/082/083 证据；不新增依赖；不重做顶栏布局与配色；不引入查找面板/侧栏；不提交、不推送、不打标签

## 开工前检查（只读）

1. 记录 `git status --porcelain` 与 `git rev-parse HEAD`，确认与 `0d17b1f` 一致。
2. 只读核对 `HomeChat.tsx:317-344`（searchMatches / 滚动副作用）、`HomeChat.tsx:1654-1690`（整条消息 ring）、`MarkdownText.tsx:63-100`（highlightTextChildren）、`TopNav.tsx:512-540`（箭头按钮）、`App.tsx:197-205`（索引推进）。
3. 复读 `PLANS.md` OD-ARCH-084 的「定案方案」6 条与「验收标准」8 条，逐条对应实现与断言。

## UI 参考（涉及界面，必填）

- 检索关键词：`developer tool text editor find bar`、`document reader in-page search highlight`
- 参考产品：Warp（https://screenshots.designknowledgebase.com/Web/Warp/）、Claude Design（https://screenshots.designknowledgebase.com/Web/Claude%20Design/）、Condens（https://screenshots.designknowledgebase.com/Web/Condens/）
- 借鉴要点（≤2 句）：查找状态收进顶栏同一个控件里，计数用等宽小字、当前项只比其余命中深一档，不额外加浮层；滚动先把命中词带到视口安全区，停稳后只做一次短强调，用户一滚动就让位。
- 上述检索已由 ARCHITECT 在批准前执行并记入 PLANS.md，可直接引用，无需重复检索。

## 实施要求

按以下顺序，每步可独立判断：

**S1 命中项实体化（MarkdownText.tsx）**
- 高亮改为递归遍历文本子节点，覆盖标题、段落、列表、引用、表格单元格、行内代码。
- 每个 `<mark>` 输出 `data-search-hit` 与 `data-search-ordinal`（单次渲染内从 0 递增的局部计数），统一 class `md-search-hit`。
- 围栏代码块（`pre`）不参与高亮，且不产生命中项。

**S2 计数以 DOM 为准（HomeChat.tsx）**
- 删掉按 `m.text` 原始字符串统计的 `searchMatches` 逻辑。
- 用 `useLayoutEffect` 在 `.chat-scroll` 内按 DOM 顺序查询 `[data-search-hit]`，以其为全局命中序列，长度上报 `onSearchMatchesChange`。
- `App.tsx` 在 `searchMatchCount` 变化时夹取 `searchCurrentIndex`，避免越界显示。

**S3 当前项两级视觉（MarkdownText.tsx / markdown.css / HomeChat.tsx）**
- 当前命中独占 `data-search-active`，在 `useLayoutEffect` 中标注（先于绘制）。
- 当前项琥珀 300/400 系实底（蓝通道 < 120），其余命中浅琥珀 200 系；深色问句气泡（`md-on-primary`）单独给一套可辨配色。
- 删除整条消息的 `ring-2 ring-amber-400`（用户问句与助手回复两处），长消息不再出现竖线。

**S4 按命中词滚动（HomeChat.tsx）**
- 以 `.chat-scroll` 为基准计算目标位置，把命中词放到视口约 32% 高度，上下留 ≥24px 安全边；已在安全区内不动。
- 切换时先 `behavior:"instant"` 落位取消上一次动画，再按需 `smooth`；`prefers-reduced-motion` 下全程 instant。

**S5 一次短强调（HomeChat.tsx / markdown.css）**
- 滚动停稳（`scrollend` 或 150ms 无滚动）后加一次约 700ms 的单次强调，不改布局宽度；`wheel`/`touchstart`/`pointerdown`/方向键任一发生即取消。

**S6 焦点保持（TopNav.tsx）**
- 上一处 / 下一处 / 清空三个按钮加 `onMouseDown={(e) => e.preventDefault()}`。
- 移除这三个按钮的原生 `title`，改用 `aria-label`；保留既有 `data-testid`。
- Enter = 下一处、Shift+Enter = 上一处，循环方向与按钮一致；0 命中时按钮 `disabled` 且键盘为空操作。

## 验证

**必须新建 `tests/verification/desktop/verify_ui_084.py`**，用现有 `window_harness.AppSession` + `XSession` + `accessibility` 在一次性 Xvfb 显示上跑真实 Tauri 窗口，合成会话数据（不接真实端点）。断言至少覆盖 PLANS.md 验收标准 1–7：

1. 长消息唯一命中：当前命中块矩形完整在视口内，上下留 ≥24px，中心 y 在可视区 25%–45%。
2. 同一长消息 8 处命中：连按下一处 7 次，当前命中块中心 y 单调递增，每次位移 > 单行高；计数严格 `1/N…8/N`。
3. 循环与边界：末项→首项、首项→末项；0 命中 `0/0` 且两个箭头 `disabled`。
4. 焦点保持：AT-SPI 取「下一个」真实矩形后点击，计数 `k→k+1`；随后键入 `zzz`，计数变 `0/0`。
5. 两级可辨、无竖线：像素扫描同时检出当前项色族与非当前项浅色族；不存在任一 x 列上跨度 >300px 的琥珀像素柱。
6. 计数与高亮一致：同一个词出现在标题、表格单元格、行内代码中，计数与可定位命中数一致，且两处命中能把视口滚到不同位置。
7. Esc 清空：高亮像素归零。

**判定纪律：**
- 每条断言必须记录测量方式、实测数值与阈值到 `evidence/od-arch-084/facts.json`，并记录脚本自身 sha256。
- 禁止恒真断言（如「像素 ≤255」「元素存在即通过」）；AT-SPI 取不到几何时必须说明替代取法。
- 断言失败就修实现，不得下调阈值。

**回归（全部要实跑并在 STATUS 记结果）：**
- `cd desktop && npx tsc --noEmit -p tsconfig.json` → exit 0
- `cd desktop && npm run build` → exit 0
- `eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)"` 后 `cargo test --offline -q --lib` → ≥261 passed
- `python3 -m pytest tests -q` → 全绿
- `verify_ui_082.py` / `verify_ui_083.py` → 全部断言仍通过（琥珀阈值语义未变）
- `git status --porcelain runtime/` → 0 行

## 硬约束

- 不修改 `runtime/` 内 DSH Runtime；不动模型路由与凭证。
- 不接触真实业务材料、案件线索或未获准数据。
- 不提交、不推送、不打标签。
- 发现计划技术问题写入 STATUS.md 并停下回报，不自行扩大范围。
- 空闲显示号用 `:110` 以上（`:104`–`:109` 已被本轮探针占用）；跑完必须 `session.stop_all()`、`Xvfb.terminate()`、`dbus.terminate()`。

## 交付

改动文件清单、实际运行的测试与结果、证据路径、未验证项、计划偏差、Git 状态。完成后停下，等待 ARCHITECT 独立复核。

