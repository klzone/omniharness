# OD-ARCH-083 派工指令：顶栏品牌标换自有 Logo + 轮次预览卡改直角

> 依据：`docs/BUILDER-Task-Instruction-Template.md`。计划来源 `PLANS.md` 的 OD-ARCH-083（用户 2026-10-25 批准「A 款」，并追加「要融入、过渡到导航条其它内容，保持美观」）。

## 获批依据

- 计划位置：PLANS.md → OD-ARCH-083
- 批准人：用户（2026-10-25，明确选择 A 款 + 融合要求）
- 允许修改：
  - `desktop/src/shell/TopNav.tsx`（品牌区）
  - `desktop/src/shell/TurnRail.tsx`（预览卡 JSX、去掉刻度按钮原生 `title`）
  - `desktop/src/theme.css`（可新增 `turn-rail-tooltip` 一类承载直角与描边，避免视觉细节散在 className）
  - 新增 `tests/verification/desktop/verify_ui_083.py`
  - 新增 `tests/verification/desktop/evidence/od-arch-083/`
  - STATUS.md（执行记录）
- 明确不做：
  - 不改 `runtime/`、不改 Profile、不读写凭证、不改模型路由
  - 不改 `desktop/public/omnidesk-icon.svg` / `omnidesk-monomark.svg` / `src-tauri/icons/*` 源资产
  - 不改顶栏高度、胶囊圆角、其余控件配色，不新增分隔线与装饰元素
  - 不改聊天区用户头像（`HomeChat.tsx:1671` 的 `Icon name="brand"`）与输入区 `BrandMark`（`HomeChat.tsx:1949`）
  - 不改 `verify_ui_082.py` 的既有断言与 082 证据目录（不得覆盖 `evidence/od-arch-082/`）
  - 不提交、不推送、不打标签

## 开工前检查（只读）

1. `git status`、`git rev-parse --short HEAD`（预期 HEAD `0ac9edf`、43 项既有改动）；`git status --porcelain runtime/` 须为 0。全部既有未提交/未跟踪改动保留，不 reset、不 clean、不 stash。
2. 读 `desktop/src/shell/TopNav.tsx` 品牌区（约 439–460 行）、`desktop/src/shell/TurnRail.tsx` 全文、`desktop/src/theme.css` 的 `.turn-rail-*` 段与 `@theme` 令牌、`desktop/src/components/BrandMark.tsx`。
3. 记录当前 082 顶栏与预览卡基线：`tests/verification/desktop/evidence/od-arch-082/turnrail-card.png`、`00-rail-before.png`、`search-match.png`。**只读引用，不得覆盖。**

## 实施要求

### S1 顶栏品牌标（A 款）

1. 用 `/omnidesk-icon.svg`（Vite `public/` 根路径引用，构建后为 `dist/omnidesk-icon.svg`）替换现在的 `w-8 h-8 rounded-full bg-gradient-to-tr from-primary to-primary-container` 圆底 + `Icon name="brand"`。
2. 用 `<img>` 呈现，`alt="OmniDesk"`、`draggable={false}`、`decoding="async"`；尺寸基准 `w-8 h-8`（32px），必要时按下方兜底降到 28px。
3. 形状与过渡（用户追加要求，逐条落实）：
   - 不加外层方框、不加彩色光晕、不加渐变外框、不叠多重大投影；最多一层极浅中性细边（如 `ring-1 ring-black/5`）让深底与白胶囊之间的硬边有过渡。
   - 图标与「OmniDesk」文字的水平间距与「名称 → Profile 徽标」的间距一致（当前容器为 `space-x-2.5` 内层 / `space-x-3` 外层，保持等距节奏即可，不要引入新的间距数值层级）。
   - 图标视觉中线与文字字面中线对齐；不改顶栏 `py-2.5` 与 `rounded-full` 结构。
   - 全条顺序维持「Logo → OmniDesk → Profile 徽标 → IPC 胶囊 → 搜索框 → 日期 → 按钮」；Logo 是唯一深色元素，其余控件不动。
4. 不得改动 `ProfileBadge`、`DateFlip`、`WindowControls`、搜索框的任何类名。

### S2 轮次预览卡改直角 + 统一视觉

1. 预览卡去掉 `rounded-xl`，四角为直角（`border-radius: 0`）。建议在 `theme.css` 新增 `.turn-rail-tooltip { border-radius: 0; }` 并把卡片视觉集中到该类，JSX 只保留布局与内容。
2. 视觉与软件既有卡片同源：白底 (`--color-surface-container-lowest`)、1px `--color-outline-variant` 级细描边、克制投影（不超过现有 `shadow-lg`）、正文 12px / 摘要 11.5px、现有灰阶。**可**在标题前加小号等宽字标「第 N 轮」（`font-label-caps`，10px 左右，`text-primary` + `bg-surface-container-low`），以呼应 Profile/IPC 徽标的小字标语言；若加入后使卡片过高或挤掉摘要，允许只用现有一行标题 + 摘要，不强求。
3. 宽度 `w-[300px] max-w-[70vw]`、定位 `right-6`、`top` 夹取逻辑、`PREVIEW_H`、`PITCH`、`INSET` 全部保持不变。
4. **去掉刻度按钮的 `title` 属性**（`TurnRail.tsx:129`）。现状是自定义卡片与 WebKitGTK 系统提示同时出现，用户看到的圆角小框主要来自后者。保留 `aria-label`、保留 `aria-describedby` → `role="tooltip"` 关系，保留 `onPointerEnter` / `onFocus` / `onBlur` / `onClick` 行为。
5. 刻度线、当前轮加粗、悬停变宽、busy 呼吸、滚动渐隐、跳转行为均不改。

## UI 参考（已完成检索，直接采用）

- 检索关键词：`chat turn navigator`、`sidebar tooltip preview card`
- 参考：ChatGPT（Web，对话轨 + 悬浮摘要）、Aside（Web，侧栏驱动的窄条导航 + 贴近指针的说明卡）、Condens（Web，极简侧栏 + 小字号等宽标签）
- 借鉴要点（2 句内）：**预览卡靠留白与小号等宽标签分层，不用大圆角和大阴影抢注意力；贴边浮动卡只保留一层细描边，与宿主卡片共用同一套表面色。**

## 验证

1. `cd desktop && npx tsc --noEmit` → exit 0
2. `cd desktop && npm run build` → exit 0
3. `cd desktop/src-tauri && cargo test --offline -q --lib` → ≥261 passed
4. 仓库根 `python3 -m pytest tests -q` → 全绿
5. `python3 -u tests/verification/desktop/verify_ui_082.py --display :<空闲编号> --root /tmp/od083-regress-082` → 17 项全 pass。**注意：该脚本会写入 `evidence/od-arch-082/`——运行前先把 `tests/verification/desktop/evidence/od-arch-082/` 备份到 `/tmp/od083-082-evidence-backup/`，跑完核对并把 082 证据恢复原样（`git status` 中 082 证据的改动形态须与开工时一致），再用新数值记录到 083 证据。若无法保证恢复，改用 `--root` 隔离运行并在 STATUS 说明未跑该脚本，改跑 083 脚本覆盖同样断言。**
6. 新增 `tests/verification/desktop/verify_ui_083.py`，基于现有 `window_harness.AppSession` / `XSession` 与 `verify_ui_082.py` 的起屏、造数据方式（合成对话，`--root` 隔离，不触真实会话）。断言：
   - `brand_logo_is_omnidesk_asset`：顶栏品牌位为 `<img>` 且 `src` 指向 `omnidesk-icon.svg`；32px 下区域非纯色、非原渐变圆底（用像素/属性双重判定）。
   - `tooltip_square_corners`：悬停刻度后卡片浮现，采样卡片四角外沿像素——直角应在角点附近直接出现边框色/卡片底色，圆角会留下背景色三角。
   - `tooltip_style_tokens`：卡片背景为 `#ffffff`、描边为 `--color-outline-variant` 量级、`border-radius` 计算值为 `0px`。
   - `no_native_title_tooltip`：刻度按钮不再有 `title` 属性；AX 树 `described-by` 仍指向 `role=tooltip`。
   - `tooltip_keyboard_focus`：键盘聚焦刻度后卡片可见（不能只截文件存在）。
   - 真跑数值写入 `evidence/od-arch-083/facts.json`。
7. 证据落 `tests/verification/desktop/evidence/od-arch-083/`：`01-topnav-brand.png`（顶栏 1:1 裁切）、`02-turnrail-tooltip.png`（悬停卡）、`03-turnrail-tooltip-corners.png`（四角 4× 放大裁切）、`04-turnrail-focus.png`、`05-topnav-full.png`（整条顶栏，用于融合核对）、`facts.json`。
8. `git status --porcelain runtime/` → 0 行。

每项结论在 STATUS.md 标注证据等级：文件事实 / 静态检查 / 脚本运行 / 真实窗口 / 未验证。

## 硬约束

- 不修改 `runtime/` 内 DSH Runtime；不动模型路由与凭证
- 不接触真实业务材料、案件线索或未获准数据；真实窗口一律用 `/tmp` 隔离数据根 + 合成对话
- 不覆盖 `evidence/od-arch-082/` 与 `evidence/od-arch-081/`
- 不提交、不推送、不打标签
- 发现计划本身的技术问题：写入 STATUS.md 并停下报告，不自行扩大范围

## 交付

STATUS.md 新增 OD-ARCH-083 小节：开工 HEAD 与 status、逐条实施记录、实际运行的命令与结果、证据文件清单、遗留问题与未验证项。完成后回报 ARCHITECT 做独立审查。
