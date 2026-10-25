# OD-ARCH-083 返工指令：只修验证脚本与 083 证据

> 依据：REVIEW.md 顶部「OD-ARCH-083 审查结论」（ARCHITECT，2026-10-25）。**UI 实现已判合格，不得重写。**

## 允许修改

- `tests/verification/desktop/verify_ui_083.py`
- `tests/verification/desktop/evidence/od-arch-083/`（截图与 facts.json）
- `STATUS.md`（追加返工记录）

## 明确不做

- 不改 `TopNav.tsx` / `TurnRail.tsx` / `theme.css` 的视觉实现（唯一例外见下方「可选」的 `PREVIEW_H`）。
- 不改 `verify_ui_082.py`，不覆盖 `evidence/od-arch-082/`、`evidence/od-arch-081/`。
- 不改 `runtime/`、Profile、凭证、模型路由；不提交、不推送、不打标签。
- 不降低任何判定标准来换绿。

## 必改 6 条（每条都要在 facts.json 里留下测量值与阈值）

1. **直角（替换恒真断言）。** 先精确定位卡片边框：在白底卡面周围找连续长边（例如从卡片中部向左/向右扫描第一处非背景像素，得到左/右边界；同理得上下边界），据此推出四角坐标。断言：①四角角点像素属于边框色（不是背景、不是卡面白）；②角点内侧 (x+1,y+1) 为卡面底色；③角点外侧 (x-1,y-1) 为背景；④四条边在角点两侧连续各 ≥4px 无缺口。四角分别记录。**禁止** `max(px)<=255` 这类任何像素都成立的判据。
2. **样式（改为运行时事实）。** 用 WebView 运行时取 `getComputedStyle`：`border-radius === "0px"`、`background-color` 为 `rgb(255,255,255)`、`border-top-width === "1px"` 且描边色灰度在 outline-variant 量级。若无法从 AT-SPI 取到计算样式，改为「渲染像素 + 边框实测宽度」双重判定并说明取法；仅 grep 源码不算。
3. **无系统提示（AX 断言）。** 断言刻度按钮 `get_help_text()` 为空、无原生 `title` 暴露；悬停前后 AX 树中均不存在独立 tooltip 节点；按钮 `get_description()` 非空且来自 `aria-describedby` → `role="tooltip"`。三项分别记录。
4. **键盘（真按键）。** 指针移到远离轮次条处，按 Tab 让焦点进入轮次条，断言卡片随焦点出现并截 `04-turnrail-focus.png`；再按 Shift+Tab，断言焦点与卡片内容切到相邻轮次（记录前后轮次号与区域像素差）。全程不得用鼠标点击刻度。
5. **Logo（真实 bbox）。** 先用像素扫描定位 Logo 实际外接框（深色底），再在该框内采样：断言尺寸 32×32（允许 ±1）、深底像素占比高、存在彩色环像素、四角为圆角而非直角裁切，并断言 Logo 视觉中线与「OmniDesk」文字中线差 ≤2px。删除硬编码的 `(135,26,172,60)`。
6. **清理 082 遗留。** docstring 改为 083 实际内容；用法示例指向 `verify_ui_083.py`；默认 `--root /tmp/od083-home`；`THREAD_ID = "t083evidence"`；会话标题去掉「搜索与索引」；写入说明与真实文件名 `facts.json` 一致。`facts.json` 每项写明 measured（含几何/坐标/像素值）与阈值，并记录脚本自身 sha256。

## 同时重做

- `03-turnrail-tooltip-corners.png`：四角裁切必须以第 1 条定位出的真实边界为准（左上、右上、左下、右下各裁到角），并标注角点坐标。
- `01-topnav-brand.png`：按第 5 条定位出的真实 bbox 裁切，不沿用错位框。

## 可选（改了要在 STATUS 说明）

- `TurnRail.tsx:20` 的 `PREVIEW_H = 96` 与实际卡高（约 120–130px）不符。若改，改为按实测卡高（`getBoundingClientRect().height` 或与内容一致的常量），并补一条「末端刻度卡片不溢出输入区」的断言；若不改，则补该边界断言并在 STATUS 记录保留常量 96 的理由。

## 验收

1. `python3 -u tests/verification/desktop/verify_ui_083.py --display :<空闲> --root /tmp/od083-rework` → exit 0，所有断言在 facts.json 中有可复核的测量值。
2. 故意破坏验证：说明每条新断言在什么情况下会 fail（例如在 facts.json 里记下阈值的构造方式），证明不是恒真。
3. `cd desktop && npx tsc --noEmit` exit 0；`npm run build` exit 0；`cargo test --offline -q --lib` ≥261 passed；`python3 -m pytest tests -q` 全绿；`git status --porcelain runtime/` 0 行。
4. `git status` 中不得出现 `desktop/src/` 的额外交付改动（除可选项 `PREVIEW_H`），不得出现 081/082 证据改动。
