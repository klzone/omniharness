# 当前实施计划

### OD-ARCH-084：会话内搜索的命中定位、逐处切换与焦点保持（待批准，2026-10-25）

> **当前状态：待用户批准。** 本节为计划，未动任何代码。批准后按 `docs/BUILDER-Task-Instruction-Template.md` 派 BUILDER。

**需求来源：** 用户 2026-10-25 实测反馈「搜索功能没法自动聚焦到已搜索到的词上，而且点上下箭头时切换得不顺」，要求先参考 DSH 原生搜索、再结合当前设计综合分析后修复。分类：**缺陷修复 + 交互打磨**（082 S8 的实现未达可用标准）。

**开工基线：** 分支 main，HEAD `0ac9edf`，工作区 48 项既有未提交/未跟踪改动（081/082/083 成果）全部保留；`git status --porcelain runtime/` 为 0。本计划不含提交、推送、打标签、改 Profile、改模型路由。

#### 已核实事实（分级）

**文件事实（读码）：**

- `desktop/src/shell/HomeChat.tsx:317-330`：`searchMatches` 只记 `{ messageId, index }`，**不记命中词在消息内的位置**。
- `HomeChat.tsx:336-344`：滚动目标是整条消息容器 `#chat-message-<id>`，`scrollIntoView({ block: "center" })`。同一消息内多个命中共享同一目标 → 切换时滚动是幂等操作。
- `HomeChat.tsx:1654 / 1687`：当前命中只给**整条消息**套 `ring-2 ring-amber-400`。长消息里这条环表现为左右两条竖线，命中词毫无标识。
- `desktop/src/shell/MarkdownText.tsx:63-100`：`highlightTextChildren` 只处理 `p`/`li` 的**直接字符串子节点**，所有 `<mark>` 同一外观、无 id / data 属性，无法区分「当前项」与「其余项」，也无法被查询定位。标题、表格单元格、引用、行内代码不参与高亮。
- `desktop/src/shell/TopNav.tsx:512-540`：上一处/下一处/清空按钮只有 `onClick`，**没有 `onMouseDown` 阻止默认行为**，点一下焦点即离开输入框；三个按钮还带原生 `title`（WebKitGTK 另弹系统提示框，与 083 已消除的同类问题同源）。
- `desktop/src/App.tsx:197-205`：`searchCurrentIndex` 只在改查询时归零；`searchMatchCount` 变化时不做夹取。

**脚本运行（真实 Tauri 窗口，Xvfb 一次性显示，合成数据，非真实模型）：**

| 探针 | 场景 | 观察结果 |
| --- | --- | --- |
| `:104` 探针 1 | 长消息内唯一命中词位于末尾 | 计数 `1/1`；视口内琥珀连通块只有一个 1×505px 竖条，即整条消息的右侧描边；命中词在视口外，看不见 |
| `:104` 探针 1 | 同一长消息内 8 处命中 | 连按回车 4 次计数 `1/10→5/10`；视口内命中块 y 只在 329/475/622 三点间 ±1px 摆动，等于在两个「整条消息居中」位置之间抖动，**没有逐处推进**；同屏 3 个 mark 外观完全一致 |
| `:107` 探针 4 | 用 AT-SPI 取按钮真实外接矩形后点击「下一个」 | 1/60 → **2/60**（按钮本身工作正常；探针 2 的「点一下跳到 60/60」是坐标点偏，作废）；随后按 Enter → 3/60；**再键入 `x`，计数仍是 3/60** → 焦点已被按钮接管，键盘输入丢失 |

**读码推断（未单独复现，随修复一并消除）：** 计数按原文统计、高亮只覆盖 `p`/`li` 的部分子节点，因此标题/表格/代码块里的词会出现「计数有、正文没有」；连续 `smooth` 滚动会互相打断。

**结论（三条根因，均有上面证据支撑）：** ①滚动目标是消息而非命中词；②同一消息内所有命中共享同一目标，切换必然幂等；③当前项没有独立视觉标识，且焦点会被箭头按钮抢走。

#### 定案方案

1. **命中项成为可定位的实体。** `MarkdownText` 的高亮从「只认 `p`/`li` 直接字符串」改为**递归遍历文本子节点**，覆盖标题、段落、列表、引用、表格单元格、行内代码；每个 `<mark>` 输出 `data-search-hit`（消息内从 0 递增，单次渲染内局部计数，不跨组件共享），并统一 class `md-search-hit`。围栏代码块（`pre`）不参与高亮，并在计数中一致排除，避免「计到但看不到」。
2. **计数以渲染结果为准。** `HomeChat` 用 `useLayoutEffect`（依赖 query / currentIndex / messages）在 `.chat-scroll` 内按 DOM 顺序查询 `[data-search-hit]`，以此作为全局命中序列并上报长度。计数与可见高亮由此天然一致（含 Markdown 结构调整、流式增长）。`App` 侧在 `searchMatchCount` 变化时夹取 `searchCurrentIndex`，杜绝 `3/2` 这类越界。
3. **当前项两级视觉。** 当前命中独占 `data-search-active`（在 `useLayoutEffect` 中标注，先于绘制，重渲染后立即重标）：当前项用琥珀 300/400 系实底（蓝通道 < 120，保持 082 脚本既有判据的含义），其余命中降为更浅的琥珀 200 系。**删除整条消息的 `ring-2`**，长消息不再出现两条竖线。
4. **滚动换成按命中词定位。** 以 `.chat-scroll` 容器为基准（沿用 `HomeChat.tsx:1627-1636` `jumpToTurn` 的几何写法）计算目标 `scrollTop + markRect.top - containerRect.top - (可视高 × 0.32 - 半行高)`，把命中词放到视口约 1/3 高度，留 ≥24px 安全边。命中已在安全区内时不动，避免来回抖。切换时先 `behavior:"instant"` 落位取消上一次动画，再按需 `smooth`；`prefers-reduced-motion` 下全程 instant。
5. **一次短强调，可被用户打断。** 滚动停稳（`scrollend`，或 150ms 无滚动）后给当前项加一次约 700ms 的单次动画（底色/描边过渡，不改布局宽度），随后撤除；`wheel`/`touchstart`/`pointerdown`/方向键任一发生即取消。参考 DSH `experimental/session-inspector/.../reveal.ts`：先精确滚动、停稳再闪、不向内容节点插元素、减少动画时直接到位。
6. **焦点不再被箭头抢走。** 上一处/下一处/清空三个按钮加 `onMouseDown={(e) => e.preventDefault()}`，点击后光标仍留在输入框可继续键入；移除这三个按钮的原生 `title`，改用 `aria-label`，消除系统圆角提示框。Enter = 下一处、Shift+Enter = 上一处，循环方向与按钮一致。0 命中时按钮 `disabled`，按钮与 Enter 均为空操作。

#### 影响范围（预计）

`desktop/src/shell/MarkdownText.tsx`、`desktop/src/shell/HomeChat.tsx`、`desktop/src/shell/TopNav.tsx`、`desktop/src/App.tsx`、`desktop/src/shell/HomeCanvas.tsx`（仅透传，如无需要不动）、`desktop/src/markdown.css` 与 `desktop/src/theme.css`（新增命中两级与短强调样式）、新增 `tests/verification/desktop/verify_ui_084.py`、`tests/verification/desktop/evidence/od-arch-084/`、`STATUS.md`、本文件、`REVIEW.md`。

**不做：** 不改 `runtime/`、Profile、凭证、模型路由、DSH 数据根；不改 `verify_ui_082.py`/`verify_ui_083.py`，不覆盖 081/082/083 证据；不新增依赖；不重做顶栏布局与配色；不引入查找面板/侧栏等新容器。

#### 验收标准（每项都要有测量值与阈值）

1. **唯一命中可见：** 长消息内唯一命中词位于末尾时，搜索后当前命中块的像素矩形完整落在 `.chat-scroll` 视口内（上下各留 ≥24px），中心 y 位于可视区 25%–45%；截图为证。
2. **逐处推进：** 同一长消息 8 处命中，连按「下一处」7 次，当前命中块中心 y 单调递增，每次位移 > 单行高（~24px）；计数严格 `1/N…8/N`。反向连按「上一处」对称。
3. **循环与边界：** 末项按下一处回到首项，首项按上一处到末项；0 命中显示 `0/0` 且两个箭头按钮 `disabled`。
4. **焦点保持：** 用 AT-SPI 取「下一个」真实外接矩形后点击，计数 `k→k+1`；随后键入 `zzz`，计数变为 `0/0`（证明字符进入输入框）。
5. **两级可辨、无竖线：** 同屏 ≥3 命中时，像素扫描同时存在当前项色族（蓝通道 < 120）与非当前项浅色族；且不存在任一 x 列上跨度 >300px 的琥珀像素柱（整条消息描边已去除）。
6. **计数与高亮一致：** 同一个词分别出现在标题、表格单元格、行内代码中，计数与可定位命中数一致，两处不同命中能把视口滚到不同位置。
7. **Esc 清空：** 高亮像素归零。
8. **回归：** `tsc --noEmit` exit 0；`npm run build` exit 0；`cargo test --offline -q --lib` ≥261 passed；`python3 -m pytest tests -q` 全绿；`verify_ui_082.py` 全部断言仍通过（其琥珀阈值含义不变）；`verify_ui_083.py` 通过；`git status --porcelain runtime/` 为 0 行。

#### 设计参考（OD-ARCH-035 要求，已用 `screenshots-design-inspiration` 检索）

关键词 `developer tool text editor find bar`、`document reader in-page search highlight`；参考 Warp（密集工具条里的等宽小计数 + 上下箭头，不抢正文）、Claude Design（对话流里用底色深浅区分当前项）、Condens（文档型阅读区的留白与单色 mark）。

**借鉴要点（≤2 句）：** 查找状态收进顶栏同一个控件里，计数用等宽小字、当前项只比其余命中深一档，不额外加浮层；滚动先把命中词带到视口安全区，停稳后只做一次短强调，用户一滚动就让位。

#### 风险与停止条件

- 若 DOM 计数在流式渲染中与视觉出现跳变，改为在滚动容器附近加 `aria-live="polite"` 的稳定计数并记录差异，不靠放宽断言掩盖。
- 若按命中词定位在 WebKitGTK 下出现连续切换抖动，保留「instant 落位」为默认并记录 `smooth` 的实测差异，不退回整条消息居中。
- 若两级配色在深色问句气泡上对比不足，仅调整该 tone 下的 mark 配色，不改气泡底色与布局。
- 发现需要改 `runtime/`、Profile、凭证或模型路由才能解决时，停止并回报，不自行扩范围。
### OD-ARCH-083：顶栏品牌标换成 OmniDesk 自有 Logo + 轮次预览卡改直角并统一视觉（已执行并复核通过，2026-10-25）

> **当前状态：** 已验收通过（REVIEW.md 顶部终审）。自有 Logo 32×32 入顶栏并做细边过渡、整条顶栏节奏未变；轮次预览卡四角直角、白底细描边小字号，系统原生圆角小窗已消除，键盘 Tab / Shift+Tab 均能驱动卡片。验证脚本经两轮返工，17 项断言有可复核的测量值与阈值。两项遗留限制（`PREVIEW_H` 仍为 96、几何扫描为启发式）登记在 REVIEW，不阻断本轮。本节保留原批准口径与验收标准，作为该计划的存档。

**批准依据：** 用户 2026-10-25 提出两条 UI 小改动，同日在看到 A/B 两版视觉对照后明确答复「A 款」，并追加要求：**要考虑新 Logo 如何融入、过渡到导航条其它内容，保持整体美观**。该追加要求属本计划内的视觉融合约束，不扩大文件范围。**开工状态：** 分支 main，HEAD `0ac9edf`，工作区 43 项既有未提交改动全部保留，`git status --porcelain runtime/` 为 0。本计划不含提交、推送、打标签、改 Profile、改模型路由。

**目标：**

1. 顶栏左侧现有的通用 spark 图标（`Icon name="brand"` 套渐变圆底）换成 OmniDesk 自己的 logo。
2. 右侧轮次索引条的悬停预览文本框：**四角直角**（`border-radius: 0`），视觉语言与软件既有卡片一致（白底、细描边、同一套字号与灰阶），不再出现圆角。

**已核实的代码事实（本计划前提）：**

- `desktop/src/shell/TopNav.tsx:447-449`：顶栏品牌图标是 `w-8 h-8 rounded-full bg-gradient-to-tr` 底 + `Icon name="brand"`（通用四圆点放射符号）。品牌文字「OmniDesk」在 450-452 行。
- `desktop/public/omnidesk-icon.svg` 与 `desktop/public/omnidesk-monomark.svg` **sha256 完全相同**（`7a5b7e01…`），都是 512×512、深色圆角方底（`#0B0F17`, rx=116）+ O+D 环 + 五节点；`desktop/src-tauri/icons/icon.png` / `128x128.png` 是同一标识的 Dock/窗口图标。
- `desktop/src/components/BrandMark.tsx` 是同一 O+D 标识的**去掉深色底**版本（透明描边），当前用于输入区「+」左侧（`HomeChat.tsx:1949`，22px）。
- `desktop/src/shell/TurnRail.tsx:154-170` 预览卡：`rounded-xl` + `border-primary/30` + `bg-surface-container-lowest` + `shadow-lg`，内含标题一行（12px）与回答摘要（11.5px，最多 3 行）。
- `TurnRail.tsx:129` 刻度按钮上还有原生 `title` 属性。WebKitGTK 会另外弹一个系统绘制的小提示框（自带圆角、单行），与自定义卡片**同时出现**，这正是「看起来是圆角小文本框」的来源之一。
- `HomeChat.tsx:1671` 用户问句旁的 28px 小头像也用 `Icon name="brand"`。
- 回归脚本 `tests/verification/desktop/verify_ui_082.py` 的 `turnrail_hover_card_appears` 以区域 (950,420)-(1290,580) 像素差 >1500 判定卡片浮现；`turnrail_card_focus_shot` 只检查截图存在。改样式后像素差会变，但阈值是下界，预计仍成立；本条须实跑确认。

**影响范围（预计）：** `desktop/src/shell/TopNav.tsx`（品牌区）、`desktop/src/shell/TurnRail.tsx`（预览卡 JSX 与原生 `title`）、`desktop/src/theme.css`（如需要可新增 `turn-rail-tooltip` 类承载直角与描边，避免把视觉细节散在 className 里）、新增 `tests/verification/desktop/verify_ui_083.py`、`tests/verification/desktop/evidence/od-arch-083/`、本文件、STATUS.md。**不动** `runtime/`、Profile、凭证、模型路由、DSH 数据根。

**定案口径：**

1. **品牌标（已定方案 A）：** 顶栏改用 `public/omnidesk-icon.svg`（与 Dock/窗口图标同一份资产，深色圆角方底），以 `<img>` 呈现，基准尺寸 30–32px，保留 `alt="OmniDesk"`；去掉渐变圆底包裹，因为图标自身已带底。不新画 logo、不新增第三方资产、不改 SVG 源文件。**方案 B 已由用户否决，不实施。**
   **融入与过渡（用户追加要求，必做，不可省）：** 图标自身的深色方底与白色胶囊顶栏之间存在明显的明度落差，直接贴上去会像一块外来补丁。实现须让它成为导航条色阶的一部分，而不是孤立的色块：
   - 形状呼应：外轮廓的圆角半径与胶囊/徽标（现有 `rounded-full` 圆底、`plate-soft` 徽标）保持同一套圆角语言；图标 12% 的圆角在 32px 下不宜再加外框，避免出现「方中套方」。
   - 尺寸与光学对齐：图标视觉中线与「OmniDesk」文字的字面中线对齐（不是与行高盒对齐），与右侧 Profile 徽标同处一条水平基线上；图标与文字的间距与「徽标到徽标」的间距同值，保持等距节奏。
   - 明暗过渡：允许在图标周围加一圈极浅的中性缓冲（例如 `ring-1 ring-black/5` 或 1px 同色描边），使深底与白顶栏的硬边变成有层次的一条细边；**不得**加彩色光晕、渐变外框或投影堆叠。
   - 全条一致性：改完后顶栏依次为「深色圆底 Logo → 名称 → Profile 徽标 → IPC 胶囊 → 搜索框 → 日期 → 按钮」。Logo 是全条唯一的深色元素，须由它承担视觉起点，其余控件保持既有浅色 plate 体系，不因这次改动重新调色、不新增分隔线、不改顶栏高度与圆角。
   - 尺寸兜底：若 32px 下 O+D 细节（五节点、中央缝）糊成一团，可降到 28px 或改用图标内留白较紧的裁切方式；以真实窗口截图为判据，不以推测为准。
2. **预览卡：** 直角（无圆角），采用与设置弹层/板卡同一套 tokens：`bg-surface-container-lowest` 白底、1px `outline-variant` 级细描边、克制的投影、`font-label-caps` 小号等宽字标「第 N 轮」、正文 12px / 摘要 11.5px 沿用现有灰阶与行高。宽度与定位（`right-6`、`top` 夹取）不变，避免影响导航可用性。
3. **去重原生提示：** 删掉刻度按钮的 `title`，只保留自定义卡片 + `aria-label`，避免系统圆角提示框叠在卡片上；`aria-describedby` → `role="tooltip"` 关系保持。
4. **不改的范围：** 家聊天区用户头像（`HomeChat.tsx:1671`）与输入区 `BrandMark` 保持不变；刻度节距、滚动、跳转、无障碍键盘聚焦行为不变。若用户希望头像同步换成新 logo，需另行批准。

**步骤与验收：**

1. **S1 品牌标：** 顶栏左侧显示 OmniDesk 自有 logo，30–32px 下 O+D 环清晰可辨、与相邻「OmniDesk」文字基线对齐，深浅背景下不发虚、不被 `rounded-full` 裁切。
2. **S2 预览卡：** 悬停与键盘 Tab 聚焦均可看到预览卡；四角为直角，样式与软件其余卡片同源；不再出现系统原生圆角提示框。
3. **S3 回归：** `tsc --noEmit` exit 0；`npm run build` exit 0；`cargo test --offline -q --lib` ≥261 passed；`python3 -m pytest tests -q` 全绿；`verify_ui_082.py` 17 项仍全 pass（若仅像素差数值变化，记录新值，不降低判定标准）；`git status --porcelain runtime/` 为 0。
   **S1b 融合验收（用户追加要求）：** 顶栏整体截图 1:1 裁切后目视确认——Logo 与名称、Profile 徽标、IPC 胶囊在一条基线上；Logo 与名称间距等于徽标间距；深色底与白胶囊之间只有一层细边过渡，无光晕/重投影；顶栏高度、圆角、其余控件位置相对 082 截图无位移（用 082 顶栏裁切做逐像素对照，允许像素差异仅出现在 Logo 与预览卡两块区域）。若 28px 后仍不清晰，记录实际尺寸与裁切参数，不靠放大顶栏解决。
4. **S4 真实窗口证据：** 一次性 Xvfb 显示里截图「顶栏品牌标（改后）」「悬停预览卡（含四角特写裁切）」「键盘聚焦预览卡」，落 `tests/verification/desktop/evidence/od-arch-083/`，并写 `facts.json` 记录实际像素数值与断言结果。改前对照可引用 `evidence/od-arch-082/turnrail-card.png` 与 `00-rail-before.png`，不覆盖 082 证据。
5. **S5 新增 `verify_ui_083.py`：** 断言 ①顶栏品牌位渲染的是自有 logo（DOM 属性 / 截图裁切像素非空且非纯渐变圆底）；②悬停后卡片浮现；③卡片四角像素与背景交界处为直角（对角点采样，圆形化会在角上留下背景色三角）；④AX 树中刻度不再暴露原生 `title` 文案作为独立提示，`described-by` 仍指向 `role=tooltip`。不修改 082 脚本的既有断言，避免改动 082 证据口径。

**设计参考（OD-ARCH-035 要求，`screenshots-design-inspiration` 检索）：** 关键词 `chat turn navigator`、`sidebar tooltip preview card`；参考 ChatGPT Web（对话轨 + 悬浮摘要，弱化边框只留一层浅描边）、Aside（侧栏驱动的窄条导航 + 贴近指针的说明卡）、Condens（极简侧栏、小字号等宽标签）。借鉴要点：**预览卡靠留白与小字号等宽标签分层，不用大圆角和大阴影抢注意力；贴边浮动卡保持单层细描边、与宿主卡片同一套表面色。**

**风险与停止条件：** ①图标换成深色方底后与圆角胶囊顶栏不协调 → 先按上面的「明暗过渡 / 光学对齐」收敛一轮（细边、尺寸、间距），仍不协调则在 STATUS 记录实际截图与偏差，停回报 ARCHITECT，**不自行改回方案 B、不改顶栏形状**；②直角卡片在浅色对话背景上边界不清 → 加细描边补偿，不加回圆角；③改样式导致 `verify_ui_082.py` 悬停断言跌破阈值 → 停并按实际像素差调整采样区域，保留判定严格性，不直接降阈值；④真实窗口无法启动 → 记录为未验证，不拿静态检查冒充运行证据；⑤为「融入」而改动顶栏高度、胶囊圆角、其他控件配色或新增装饰元素 → 越界，停手并写 STATUS 等裁定。

### OD-ARCH-081：首页三项小修复（问候常驻 / 品牌标 / Markdown 正文）（已执行并复核通过，2026-10-25）

**批准依据：** 用户明确提出三条小修复并委托执行。本计划不扩大范围。

**目标：** ①「夜深了 · 欢迎回到 OmniDesk」与其后的待办条**常驻顶部**，不再因指针进入对话区而隐藏、也不再把对话整页上移；②输入框「+」左侧的装饰性 spark 图标换成 OmniDesk 品牌标（复用既有 O+D 标识，非交互、带无障碍标注）；③助手回复与用户问句的正文按 Markdown 渲染（标题、列表、表格、代码块、引用、链接），参照 DSH 原生正文的排版密度；流式增量仍逐段呈现、不丢字。

**影响范围：** `desktop/src/shell/SceneScaffold.tsx`（去掉两层互斥与 5 秒静止恢复，改为固定上下两段）、`desktop/src/shell/HomeChat.tsx`（正文渲染入口、输入区图标）、新增 `desktop/src/components/BrandMark.tsx`、`desktop/src/shell/MarkdownText.tsx`、`desktop/src/markdown.css`、`desktop/src/main.tsx`（样式引入）、`desktop/package.json` + lock（新增 `react-markdown`、`remark-gfm`，见下）、本文件、STATUS.md。**不动** `runtime/`、DSH 数据根、凭证、模型路由、真实材料。

**依赖理由：** 现有前端依赖中没有 Markdown 渲染能力，自研渲染器在表格、嵌套列表、代码块上易出错误，因此引入 `react-markdown` + `remark-gfm` 两个纯渲染依赖。不启用 `rehype-raw`，模型或用户文本中的 HTML 不会被执行或注入（无 `dangerouslySetInnerHTML`）。

**步骤与验收：**

1. S1 布局：问候 + 待办固定在对话上方，指针移动、输入框聚焦、静止等待都不改变其可见性；对话区在其下方自然占满剩余高度。
2. S2 品牌标：输入区「+」前显示 OmniDesk 标识（纯装饰，`aria-hidden`），点击不承担动作。
3. S3 Markdown：助手正文支持标题 / 无序与有序列表 / 粗体 / 行内代码 / 围栏代码块（带复制）/ 表格（横向滚动）/ 引用 / 链接；用户问句按 Markdown 渲染，但纯文本换行仍保留；流式输出时逐字显现与 Markdown 排版可共存、不出现乱码。
4. S4 回归：`tsc --noEmit`、`npm run build`、`cargo test --offline -q --lib`（≥257）、`python3 -m pytest tests -q`（≥75）、`git status --porcelain runtime/` 为 0。
5. S5 真实窗口证据：一次性 Xvfb 显示里截图确认问候与待办常驻（指针移入对话区前后均可见）、输入区显示品牌标、含表格/代码/列表的合成回复排版正确；证据存 `tests/verification/desktop/evidence/od-arch-081/`。不提交、不推送。

**风险与停止条件：** 新增依赖导致构建或 CSP 失败 → 停并报告，不退回自研渲染器硬凑；布局改动引起对话区高度异常或输入框不可见 → 停并保留原骨架；真实窗口无法启动 → 记录为未验证，不把静态检查当运行证据。

### OD-ARCH-065：多提供商模型档案 + 探测沿用已存密钥（已批准并执行完毕，2026-10-22；ARCHITECT 复核通过见 REVIEW.md）

> **批准依据：** 用户实测 064 通道时点「测试连接」得到 `https://amr-link.open-design.ai/v1/models answered 401`。排查结论（源码事实）：该端点无凭据直连必回 401；用已登记的 `opendesign_Key` 直连返回 200 与 12 个模型。问题不在密钥，而在探测请求没带凭据。用户同时要求：能长期维护多个提供商/模型及其地址与密钥，并明确不得泄露。

**目标：** ①「测试连接」在未输入新密钥时自动沿用该提供商已保存的密钥，不再误报 401；②模型配置从「单套覆盖」升级为「多提供商档案」：每个提供商各存一组模型、各自的地址与凭据引用，用户可增删改与切换默认；③全程沿用 DSH 官方凭据机制，OmniDesk 侧永不读取、缓存、回显或落盘密钥明文。

**已由源码证实的事实（本计划成立的前提，BUILDER 须先复核再动手）：**

1. DSH 的 provider 配置只接受 `apiKeyEnv` 引用名，明文写不进去（`packages/llm/llm-pi-ai/src/index.ts::resolveApiKey`）；密钥只能经官方 `credentials/set` 进入 `$DSH_HOME/.credentials.yaml`。
2. `credentials` Remote 命名空间只有 `describe / set / unset`，**没有任何取回明文的读路径**（`packages/api/settings-controller/src/credentials.ts`，注释明写 "Secret values cross in one direction only"）。因此「在 OmniDesk 里读回密钥再转发」不可实现，也不能去实现。
3. DSH 的 `llm/discoverModels` 已内置正确做法：请求里带 `provider` 时，它用该路由已登记的凭据解析密钥（`discoverModels(request, storedProfile)`，`discovery.ts` 第 308–315 行）；只有「草稿未带 key 且未指 provider」时才会裸请求。当前 OmniDesk 恰好走的是后者——`probe_models` 有意不传 `provider`（`dsh_settings.rs` 注释：避免命中内置 catalog 而绕过真实网络请求）。这是本缺陷的直接成因。
4. 传 `provider` 的唯一副作用：若该 provider 名正好是 pi-ai 内置目录里的厂商（openai / deepseek / openrouter / zai / qwen 等），DSH 会直接返回内置模型表而不发网络请求。当前及计划内的提供商（opendesign、agnescn、glm 网关等自建路由名）不在内置目录中，因此不影响真实连通性判定；但这一点必须在实现中显式处理，不得默认忽略。
5. `llm-pi-ai` 的 `providers` 是**以路由名为键的字典**（`config.ts::resolveProfiles`），天然支持多提供商并列；补丁层按 entry id 合并，因此多个提供商可写进同一份 OmniDesk 覆盖文件。当前 `render_overlay()` 只渲染一个 provider，是本计划要扩的地方。
6. **补丁层是浅合并，不是深合并**：`applyEntryPatches` 对非 insert 补丁执行逐键赋值（`runtime/deepseek-harness/vendor/include/src/index.ts` 约 114–119 行），所以补丁里的 `config` 会**整体替换**原 entry 的 `config`。实测 `dsh --profile web --dump-config --patch <覆盖文件>` 后，`providers` 下只剩覆盖文件里的那条路由，Profile 自带的 `agnescn` 消失，`agent-default-model` 同样被整体替换。因此「多 provider 渲染」必须按下面的合并口径写，否则会静默抹掉用户已有的其它路由。
7. 「测试连接」走的设置服务是独立起的 `dsh web --port 0 --no-open`，**不带 `--profile` 也不带 `--patch`**（`dsh_settings.rs::spawn` 约 394–420 行），Profile 取 DSH 默认（`web`）。所以该进程内存里没有覆盖文件里的 `opendesign` 路由：即使请求带上 `provider`，也解析不到该路由的 storedProfile，仍会退化成裸请求。要让「沿用已保存密钥」真正成立，设置服务必须以与引擎相同的 Profile + 同一份覆盖文件启动。

**影响范围：** `desktop/src-tauri/src/model_settings.rs`（数据结构与渲染，含迁移）、`desktop/src-tauri/src/lib.rs`（`model_settings_probe` 增加 provider 参数；保存/加载命令适配多档案）、`desktop/src-tauri/src/dsh_settings.rs`（`probe_models` 透传 provider，并区分“内置目录答复”与“真实网络答复”）、`desktop/src/SettingsPanel.tsx`（提供商列表 + 增删改 + 设为默认）、`desktop/src/shell/ModelPicker.tsx` 仅当现有目录数据不足时才动、`docs/user-testing-checklist.md` 对应步骤、`docs/OmniDesk-Desktop-Data-Layout.md`（配置文件结构说明）、PLANS/STATUS/REVIEW。通用红线：runtime/ 零差异、凭证零读取零打印零落盘、真实材料不进 Git/evidence、一次性授权分批提交、推送一次。

**步骤：**

1. **S1 探测沿用已存密钥（先修 bug，独立可验收）：** `model_settings_probe` 增加可选 `provider` 入参，前端把当前表单的 `provider` 一并传入；`probe_models` 在草稿里带上 `provider`（仅当该 provider 已存在于覆盖文件时才带，避免凭空造路由）。判定口径：界面文案改为「沿用已保存密钥 / 本次输入的新密钥 / 未带密钥」三态之一，让用户知道这一次到底用了什么。若 DSH 返回的是内置目录而非网络结果，界面须明确标注「该提供商名命中 DSH 内置目录，未发起真实网络请求」，不得当作连通性证明。验收：不填新密钥点测试，OpenDesign 应返回 12 个模型；填入错误密钥点测试，应明确报鉴权失败；两者都不改任何已保存配置。**前提（源码事实 7）：** 设置服务现在不带 `--profile` / `--patch`，进程里的路由表没有覆盖文件里的 provider，只带 `provider` 仍解析不到已存凭据。因此 S1 必须同时让设置服务按引擎同 Profile + 同一份覆盖文件启动：参数只取自 `EngineConfig`（`profile` 与 `sync_overlay()` 后的 `patch_files`），不接受任何外部输入；覆盖文件不存在时退回现有启动参数。此改动只影响 OmniDesk 自己拉起的设置服务进程，不改 Profile 文件、不改 DSH Runtime、不碰原版 DSH 数据。
2. **S2 多提供商档案数据结构与迁移：** 配置升级为 `omnidesk.model-settings.v2.json`，结构为 `providers: [{ provider, display_name, base_url, api, api_key_ref, models: [...], reasoning_effort }]` + `default: { provider, model }`。**必读**：读取到 v1 文件时自动就地升级为 v2（保留全部字段与老文件名作为 `.v1.bak`），升级失败不得删除或覆盖原文件，界面明确报错并只读。每个 provider 的 `api_key_ref` 必须互不冲突，重名直接拒绝（否则两个提供商共用一把密钥会导致串号）。
3. **S3 覆盖文件多路由渲染：** `render_overlay()` 渲染全部 providers，`acp` 与 `agent-default-model` 两段指向 `default`。保留既有“只写非机密字段”的约束与单测（`overlay_contains_only_non_secret_fields` 必须继续通过，并扩充为多 provider 场景）。保存前必须校验：路由名唯一、模型列表非空、baseURL 合法、api 在允许集合内。

   **合并口径（因源码事实 6，不得省略）：** 因为补丁层对 `config` 是整体替换，写覆盖文件前必须先把“覆盖生效后的完整路由集合”算出来：读取引擎 Profile 的 `llm-pi-ai` providers（用现有 `--dump-config` 通道，不读凭据），以 OmniDesk 数据根里的 provider 集合覆盖同名项、保留其它项，然后把合并结果整体写入覆盖文件；合并失败（Profile 读不到、结构异常）时直接报错不写盘，不得退回“只写 OmniDesk 自己那几条”（那会静默抹掉其它路由）。若合并后的路由集与引擎实际加载不一致，以 `--dump-config` 的实测结果为准并记录差异。验收必须包含一条：合并后 `providers` 下同时存在 `opendesign` 与 Profile 自带的 `agnescn`（或当前 Profile 实际自带的路由）。
4. **S4 设置页多提供商管理：** 「模型与端点」页改为左侧提供商列表（显示名 + 是否已配密钥 + 是否默认）+ 右侧编辑区。支持新增、编辑、删除（删除仅移除配置与该路由，**不删除 DSH 凭据**，界面须明说）、设为默认。删除最后一个提供商时须明确警告将导致无可用路由。所有密钥输入框仍为只写：保存后清空、回显只有「已配置 / 未配置 / 读取失败」。密钥仅写入当前选中 provider 的 `api_key_ref`，切换提供商时不得回填或复用上一家的密钥；不提供“复制另一个 provider 的密钥”能力。
5. **S5 模型选择器与对话路由：** 确认 `ModelPicker` 能列出多提供商下的全部模型并正确切换（当前已有 `providers[]` 结构，预计改动很小）；若切换某提供商模型后引擎未按其路由启动，记录为独立缺陷并停下报告，不在本计划内扩大修复。
6. **S6 回归与证据：** 单测覆盖 v1→v2 迁移、多路由渲染、密钥引用冲突拒绝、探测三态判定；`tsc` / `npm build` / `cargo test` / `pytest` 全绿；055/060 联动脚本复跑；真实窗口截图存 `evidence/od-arch-065/`（提供商列表、探测成功沿用已存密钥、探测错误密钥失败）；`facts.json` 落盘。**证据中不得出现任何密钥字符或其片段。**

**存储方式（回答“是否用 DSH 原版保存方式”）：** 是。密钥仍由 DSH 官方凭据服务保存在 OmniDesk 隔离数据根的 `.credentials.yaml`（权限 0600），OmniDesk 不另建加密库，也不把密钥写进自己的 JSON/YAML。多提供商的差异只体现在引用名：每个 provider 有自己的 `api_key_ref`（即 DSH 的 `apiKeyEnv` 名），每个名字在 DSH 凭据库对应一条密钥；同一提供商下的多个模型共用该引用名，不重复存密钥。因此新增提供商 = 新增一条路由配置 + 一个新引用名 + 一次官方 `credentials/set`，彼此互不覆盖。

**验收：** 不填新密钥点「测试连接」能成功并显示模型列表；填错密钥能明确报鉴权失败；至少能并列保存两个提供商并各自记住模型列表、地址与凭据引用；切换默认模型后对话走对应路由；v1 配置可无损升级；`runtime/` 零差异；仓库与证据中不存在任何密钥明文；`overlay_contains_only_non_secret_fields` 类单测通过。

**安全要求（不可打折）：** 不实现任何“读回密钥”的接口，无论前后端；不在日志、错误信息、测试夹具、截图、`facts.json` 中出现密钥；不打日志打印请求头；不把 DSH 的 `.credentials.yaml` 复制进 OmniDesk 数据根或仓库；判断“是否已配置”只允许用 `credentials/describe` 的 `configured` 布尔值。

**风险与停止条件：** ①`llm/discoverModels` 在带 `provider` 时对自建路由仍不解析凭据 → 停下报源码位置与实际错误码，不改成前端读密钥；②v1→v2 迁移在真实用户配置上出现字段丢失 → 立即停，保留原文件不动；③多 provider 渲染导致 DSH 启动失败 → 保留单 provider 回退路径并报告；④同一问题连续两轮修复未解决即停。⑤设置服务按 `--profile` + `--patch` 启动后，原先依赖 DSH 默认 `web` 行为的探测或保存流程失效（例如覆盖文件不合法导致设置服务起不来）→ 立即停，退回不带 `--patch` 的启动方式并报告，不得为了让探测通过而写一份不完整的覆盖文件；⑥合并后的覆盖文件在真实 Profile 上造成路由意外丢失或被顶掉 → 停，保留原文件，提交差异清单等 ARCHITECT 裁定。

### OD-ARCH-064：用户实测通道整备（启动入口 + 清单校准）（已批准，2026-10-22）

### OD-ARCH-066：响应速度诊断收尾 + 搜索授权链路（待批准，2026-10-23）

**背景（REVIEW.md 2026-10-23 已诊断）：** ①40 条任务台账实测成功任务中位 11 秒，本地链路毫秒级，30 秒感受来自模型推理与中转网络，无需架构改动；②场景里触发需要审批的工具（联网搜索）后卡住——OmniDesk 的 Web-API 通道没有审批 UI，前端也不消费 tool_activity 事件，turn 挂在等审批直到超时或用户取消。本计划补齐审批链路并加首字延迟埋点。

**目标：** ①模型审批请求在 OmniDesk 界面可见、可允许 / 拒绝，允许后任务继续；②任务卡片显示「等待审批中」状态，不再无限静默；③提交到首字到达（TTFB）有埋点，下一轮实测可直接对比模型侧耗时。

**影响范围：** engine/omnidesk_engine/web_api_adapter.py（把 DSH 审批请求转成归一化事件 + 提供 approve/reject 响应方法）、desktop/src-tauri/src/lib.rs（透传审批事件与用户决定命令）、desktop/src/shell/HomeChat.tsx（审批卡片渲染）、desktop/src/types.ts、desktop/src/api.ts；可选 engine_state 台账增加审批字段。不改 runtime/、不改 DSH 凭据、不新增通信层。

**步骤：**
1. **S1 审批事件贯通：** 适配层识别 DSH 的审批请求事件（approval seam / user-approval 相关事件），转成 EventType.PROGRESS（kind="approval_request"，带 toolName/reason/callId）；前端 HomeChat 渲染审批卡片（工具名 + 原因 + 允许 / 拒绝按钮）。
2. **S2 审批应答：** 适配层新增 respond_approval(callId, allow) 方法，经 DSH 通道回传决定；允许后任务继续流式输出，拒绝后 turn 以引擎语义落定（大概率 blocked→failed，界面如实显示）。Rust 侧加 omni.approval_respond 命令透传。
3. **S3 TTFB 埋点：** 适配层记录 submit → 首个 text_delta 的秒数，进任务台账（新字段 ttfb_seconds）；前端任务详情或 usage 面板显示。不读凭证、不记录请求内容。
4. **S4 测试与证据：** 单测覆盖审批事件归一化 + 应答；合成端点无法产真实审批请求时，用录制帧回放测试；真实审批流程由用户实测验收（触发一次搜索授权并允许）。证据：pytest 结果、tsc、cargo test、台账字段示例（无凭证）。

**验收：** 用户在 OmniDesk 里触发搜索授权 → 界面出现审批卡片 → 点允许 → 任务继续并完成；点拒绝 → 任务以失败/中断落定且界面如实显示；台账含 ttfb_seconds；runtime/ 零差异；未批准的工具默认仍是 ask（不改权限预设）。

**风险：** ①DSH web-api 通道的审批事件格式未实测 → S1 先加录制探针确认事件结构，再写归一化；②审批应答 RPC 可能不在 web 协议里 → 若 DSH 不支持，停下报告，方案改为「界面明示该工具需要到 DSH 原生界面批准」；③不扩大到权限预设管理（已有设置页管）。

> **批准依据：** 061~063 三轮界面收敛验收通过，用户进入实测阶段。本计划只整备实测通道与清单，不做任何功能扩张。用户既定指示：任务合并、不零碎派工。

**目标：** 用户可一键进入应用开始实测；上手清单与 061~063 后的真实界面一致；实测入口唯一。

**影响范围：** scripts/omnidesk.sh（核对即可，若无缺陷不改）、desktop/src/shell/Dock.tsx（若残留设置入口则移除）、docs/user-testing-checklist.md（按当前界面校准：Dock 5 入口名称、设置页 6 分页、分类下拉与发送/取消合并按钮、模型与端点连通测试三态）、PLANS/STATUS/REVIEW。通用红线：runtime/ 零差异、凭证不碰、真实材料不进 Git/evidence、一次性授权分批提交（每批前红线扫描 + 文件清单核对）、推送一次。

**步骤：**
1. **S1 启动入口核对：** 验证 scripts/omnidesk.sh 从干净 shell 一键启动 Desktop 窗口（合成数据根）；如已有缺陷停下报告，不擅自改。若有第二入口（dev-ui.sh 等）在文档中标注用途差异，实测统一走 omnidesk.sh。
2. **S2 Dock 残留核对：** Dock 5 入口与 aria 标签逐一核对；如发现设置入口残留（061 后不应再有），移除并复跑 055 联动脚本。
3. **S3 实测清单校准：** user-testing-checklist.md 按当前界面逐条校准：①Dock 5 入口与名称（工作台首页/工作记录/文案写作/项目跟踪/历史会话）；②设置唯一入口=顶栏齿轮，6 分页（模型与端点/对话分类/成果与文档库/权限/引擎诊断/关于）；③分类选择=左下角下拉胶囊（色点+勾选列表）；④发送/取消合并按钮行为；⑤端点连通测试三态预期不变；⑥分类管理与 vault 初始化入口指引（设置→对话分类）；⑦每步预期结果与「不算数」口径。清单不新增实测步骤之外的内容。
4. **S4 回归与证据：** tsc / cargo test / pytest / npm build 全绿；055/060 联动脚本复跑；omnidesk.sh 启动成功截图 1 张存 evidence/od-arch-064/ 并把事实写入 facts.json；STATUS 登记执行记录；REVIEW 的 064 验收结论由 ARCHITECT 复核后补写。

**验收：** omnidesk.sh 一键启动成功且 Dock 5 入口；清单与真实界面零偏差（每条对应一张现势截图或既有 063 证据）；全量回归绿；runtime/ 零差异；facts.json 落盘。

**风险与停止条件：** ①omnidesk.sh 启动失败 → 停下报告根因，不擅自改启动链路；②清单与界面有出入 → 以界面为准改清单，不反向改界面。

### OD-ARCH-063：分类 chip 下拉化 + 清理收尾（已批准，2026-10-22）

> **批准依据：** 用户已拍板分类 chip 改下拉（色点 + 勾选弹出列表 + 与模型选择器同套样式）；062 复核发现的证据勘误与仓库残留一并收尾。继续合并派工，不零碎。

**目标：** ①对话分类选择器改为下拉胶囊（色点 + 弹出勾选列表，对齐模型选择器样式），分类多时不占行、宽度恒定；②清掉 062 复核发现的仓库残留与命名尾巴；③证据落盘口径修正。

**影响范围：** desktop/src/shell/HomeChat.tsx（chip 改下拉按钮 + 弹出列表）、desktop/src/components/Icon.tsx（如需下拉箭头）、desktop/src/SettingsPanel.tsx 与 styles.css（.wb-header 类名收尾，仅设置页在用才改名，否则注明）、desktop/src/offline-flows.css 与 .ofl-v2 命名（仅设置页在用则改名，牵动布局回归时停下列清单）、删除仓库根 `./0` 空文件、删除 tests/verification/desktop/evidence/od-arch-060/00-home.png (deleted) 残留文件（原 00-home.png 正常保留）、evidence 命名核对、PLANS/STATUS/REVIEW。通用红线同 062：runtime/ 零差异、凭证不碰、旧数据零迁移零覆盖零删除、一次性授权分批提交（每批前红线扫描 + 文件清单核对）、推送一次。

**步骤：**
1. **S1 分类下拉：** 平铺 chip 行改为单枚下拉胶囊：左侧色点（分类色，未分类用中性色）、中间当前分类名、右侧下拉箭头；点开弹出列表（勾选态 + 色点 + 分类名，滚动容纳），单选后立即收起并生效（复用既有 retag/归档逻辑，不改持久化格式与键）。样式与模型选择器同套（复用其类或同规格新类，不引第三方组件库）。新增分类后下拉列表自动可见。键盘可达（Enter/Esc）与 aria 属性齐备，沿用 data-testid 钩子。
2. **S2 仓库残留清理：** 删 `./0` 空文件；删 evidence/od-arch-060/00-home.png (deleted) 残留；.wb-header 仅设置页在用则改名（全引用核对），否则保留并注明；.ofl-v2/offline-flows.css 若仅设置页在用则同步改名，若牵动 >3 处布局回归则停下报清单。
3. **S3 回归与证据：** tsc / cargo test / pytest / npm build 全绿；055/060 联动脚本复跑；真实窗口截图存 evidence/od-arch-063/：下拉收起态（色点可见）1 张、展开列表（含勾选与新增分类）1 张、选中后收起生效 1 张、设置页布局 1 张；**验收事实统一写入 evidence/od-arch-063/facts.json**（命令、结果、截图清单），修正 062 的事实未落盘问题；STATUS 登记执行记录；REVIEW 的 063 验收结论由 ARCHITECT 复核后补写。

**验收：** cargo test 全绿；tsc 0 错误；pytest 通过；npm build 通过；runtime/ 零差异；chip 下拉在 4 类与新增后 ≥5 类两种状态下宽度不变且列表完整可滚；选中变更仍走既有归档链路（不改 thread_store 数据）；facts.json 落盘齐全；仓库根与 evidence 目录无残留文件。

**风险与停止条件：** ①下拉展开与模型选择器/语音按钮层叠冲突（z-index/遮挡）→ 停下报截图与定位，不硬改全局层叠；②分类改名后旧会话显示回退失败 → 如实显示内部键并报问题，不静默改数据；③.ofl-v2 改名牵动面大 → 停下等裁定。
@@
### OD-ARCH-062：分类管理持久化 + workbench 清理收尾（已批准，2026-10-22）

> **批准依据：** 061 复核两项裁定（REVIEW.md 顶部：分类管理高级能力、取消测试解耦方案）+ 一项修正要求（SettingsPanel 样式类去 workbench 语义、OfflineFlows 处置）。用户既定指示：任务合并、不零碎派工。

**目标：** ①对话分类管理落成宿主持久化能力，设置页从展示升级为可管理（显示名/新增分类/文件夹与 vault 映射/初始化骨架）；②彻底清理 workbench 遗留（源文件、样式类、无引用组件），并让取消测试不再依赖待删文件。

**影响范围：** desktop/src-tauri/src/settings.rs（只增字段与读写，不改既有字段与命令语义）、命令注册、desktop/src/SettingsPanel.tsx（分类页管理 UI；根节点样式类改名去 `.workbench ofl-v2` 语义）、desktop/src/shell/HomeChat.tsx（chip 显示名读宿主配置）、新增 engine-cancel 区域独立模块（位置由 BUILDER 按现有结构定，如 src/engineCancelModel.tsx）、tests/verification/desktop/verify_engine_cancel.py（改抽取源）、删除 desktop/src/workbench.tsx 与 workbench-ui.css、OfflineFlows.tsx 确认无引用后删除、styles.css 中仅设置页在用的 .workbench 类同步改名（历史文档与既有 evidence 不动）、PLANS/STATUS/REVIEW。通用红线同 061：runtime/ 零差异、凭证不碰、旧 YvanAI 与既有成果零迁移零覆盖零删除、一次性授权分批提交（每批前红线扫描 + 文件清单核对）、推送一次。

**步骤：**
1. **S1 分类持久化与管理页：** settings.v1 新增 thread_categories 持久化（默认四类 plain/weekly/writing/project，每类含显示名、归档目标文件夹、vault 绑定），读写命令不改既有字段语义，附单测（默认值、roundtrip、越界拒绝）。设置页分类页实现：显示名编辑、新增分类（只进 chip 与归类下拉，不自动加 Dock 图标）、每类映射成果文件夹与 Obsidian vault（目录必须限定在成果根或已绑定 vault 内，越界拒绝并明示原因）、初始化骨架按钮（只在目标根内建目录骨架，已存在跳过并提示，不触碰既有文件）。HomeChat chip 与归档归类显示名从宿主配置读取，内部键不变。
2. **S2 取消测试解耦与 workbench 删除：** 把 workbench.tsx 中 `#region engine-cancel-model` 区域**逐字**迁到独立模块（不改逻辑、不改行为），verify_engine_cancel.py 改从新模块抽取；先跑该测试确认通过，再删 workbench.tsx 与 workbench-ui.css（删前列文件清单与全 desktop/src 引用核对）。SettingsPanel 根节点样式类改名，styles.css 内 .workbench 类若仅设置页在用则同步改名，否则保留并注明。
3. **S3 OfflineFlows 处置：** 确认全仓无 import 后删除该文件；若发现仍被引用，停下列引用清单报 ARCHITECT，不擅自改引用方。
4. **S4 回归与证据：** tsc / cargo test / pytest / npm build 全绿；改源后的 verify_engine_cancel.py 真实通过；055/060 联动脚本复跑；新增截图存 evidence/od-arch-062/：分类页编辑态 1 张、新增分类在 chip 下拉生效 1 张、初始化骨架后成果目录 1 张；STATUS 登记执行记录（含删除文件清单与迁移对照说明）；REVIEW 的 062 验收结论由 ARCHITECT 复核后补写。

**验收：** cargo test 全绿且 settings 新字段单测覆盖（默认四类/roundtrip/越界拒绝）；tsc 0 错误；pytest 通过；npm build 通过；runtime/ 零差异；desktop/src 内 workbench 与 OfflineFlows 引用 0 处；verify_engine_cancel.py 通过且抽取源为新模块；真实窗口完成一次「新增分类 → chip 下拉可见 → 成果映射保存 → 重启后仍在」链路验证。

**风险与停止条件：** ①settings 既有序列化格式与新增结构不兼容（读旧文件失败）→ 停下报兼容方案，不擅自迁移用户数据；②迁移取消区域发现依赖 workbench 内部状态或上下文 → 停下报告，不得改写逻辑迁就；③新增分类持久化写失败或读取冲突 → 界面明确报错，不静默降级。

### OD-ARCH-061：界面精简与设置归一（已批准，2026-10-22）

> **批准依据：** 用户实测 060 后给出的五条微调，即本计划范围：①对话左下角归类固定为 未分类/工作记录/文案写作/项目跟进，设置中提供分类管理与初始化（含本地文件夹与 Obsidian 适配），样式匹配现有 UI；②设置入口唯一化（顶部导航设置按钮），设置页按功能分页；③去掉挂载库；④Dock 只留工作场景（周报已改工作记录）+ 历史会话；⑤删任务工作台及其相关功能，成果目录与引擎诊断并入设置。

**目标：** 收敛界面入口与设置职责：首页对话保留四类归类，Dock 只剩场景入口，设置一处分页管理。内部 scenario 键（plain/weekly_report/writing/project）、数据目录名、既有命令语义一律不变，只动展示层与设置承载。

**影响范围：** desktop/src/shell/Dock.tsx、HomeChat.tsx、App.tsx（路由）、SettingsPanel.tsx（分页重构）、TopNav.tsx（必要时）、Workspace.tsx 与 EngineTechSlice.tsx（随 workbench 下线，删代码列清单）、desktop/src-tauri（仅当分类映射需要持久化字段时新增，不改既有命令语义）、samples/profiles/discipline.yml（如入口声明需同步）、PLANS/STATUS/REVIEW。通用红线同 059：runtime/ 零差异、凭证不碰、旧 YvanAI 与既有成果零迁移零覆盖零删除、一次性授权分批提交（每批前红线扫描+文件清单核对）。

**步骤：**
1. **S1 对话归类升级：** HomeChat 左下角场景 chip 固定四类（未分类/工作记录/文案写作/项目跟进），样式对齐现有设计语言（胶囊形/surface 色阶）；选中即会话场景标签（复用 retagThread）。设置页新增"对话分类"分页：四类默认展示、显示名可改、可新增未来分类（新增分类只进归类下拉与归档映射，不自动加 Dock 图标）、每类映射成果文件夹与 Obsidian vault（复用 059 vault 绑定机制）、提供"初始化文件夹"按钮（只在成果根/vault 内建目录骨架，不触碰既有文件）。
2. **S2 设置唯一入口与分页：** SettingsPanel 改分页：模型与端点 / 对话分类 / 成果与文档库（成果目录管理自 Dock settings 页迁入）/ 权限 / 引擎诊断（只读，自 engine 页迁入）/ 关于。顶部导航设置按钮为全应用唯一设置入口；Dock 移除 settings 与 engine 两项。
3. **S3 去挂载库：** HomeChat 输入区"挂载库"按钮及相关无引用状态移除（语音置灰占位保留，不在本计划范围）。
4. **S4 Dock 精简与任务工作台下线：** Dock 固定 5 入口：工作台首页/工作记录/文案写作/项目跟踪/历史会话（"项目与会话"改名"历史会话"）。删除 workbench 入口与路由及其独有组件（EngineTechSlice 面板迁入设置后原页面移除；Workspace.tsx 自带 HomeChat 等 060 遗留死代码一并清理，删改清单写入执行记录）。声明文件场景列表只覆盖 5 个业务场景，基础设施入口由代码固定（沿用 026 规则）。
5. **S5 证据与记录：** 截图存 evidence/od-arch-061/：Dock 5 入口、设置分页各 1 张、归类 chip 至少两种选中态、挂载库移除后输入区 1 张；复跑 verify_scene_decl_linkage_055.py 与 verify_scene_overview_060.py；STATUS 登记执行记录；REVIEW 中已写入的 051/060 复核结论随本批入库提交（061 验收结论由 ARCHITECT 复核后补写）。

**验收：** tsc 0 错误；cargo test 全绿（沿用既有 PKG_CONFIG_PATH 环境命令）；pytest 通过；npm build 通过；git status runtime/ 零差异；Dock 渲染 5 入口且无 workbench/settings/engine；HomeChat 归类四值与 retag 下拉一致；"挂载库"grep 全仓 0 处；workbench 路由与组件无引用残留（grep 清单核对）；055/060 联动脚本复跑通过。

**风险与停止条件：** ①分类管理需新增 Rust 持久化字段 → 先停下报方案再动手；②删除 workbench 发现被 055/声明联动或其他模块引用 → 停下列引用清单等 ARCHITECT 裁定；③设置分页重构若导致既有设置项丢失 → 优先用原组件分页承载，不重写设置逻辑。

### OD-ARCH-060：场景页 v2——状态条 + 文档库 + 时间线（已批准，2026-10-22）

> **背景：** 用户看过方案讨论后拍板按 ARCHITECT 推荐（A/B 融合骨架）开发，看效果再调。三场景同一骨架，差异用模块开关；会话历史降为次级资源。本计划同时把 059 收尾材料（059 执行记录、059 复核结论、首页下拉框修正）一并入库提交。

**目标：** 场景页从"公告栏 + 双栏"升级为：场景状态条（待确认/逾期/截止）+ 左文档树 + 右成果视图（卡片/列表 + frontmatter 筛选）+ 底部折叠会话历史；工作记录页加"本期记录碎片"区，项目页加"进度"页签（现有）。

**影响范围：** desktop/src/shell/ScenePage.tsx（重排）、AnnouncementBar.tsx（状态条化改造，保留 046 降级规则）、新增 ArtifactExplorer.tsx（文档树+视图）、HomeCanvas.tsx（"今日重点推进"接真数据，摘掉"未接入数据源"空态徽标）、desktop/src-tauri（新增场景汇总命令：按场景聚合 pending_writes 数量与本周截止；复用现有命令，不改既有命令语义）、PLANS/STATUS/REVIEW。提交沿用一次性授权分批模式。通用红线同 059（runtime 零差异、凭证不碰、旧数据零迁移）。

**步骤：**
1. **S1 状态条：** AnnouncementBar 顶部改为三段状态卡：待确认写入 N（点击展开 PendingWritePanel）/ 逾期 N（pending_writes 过期项）/ 周期提醒（工作记录页=本期周报·周五截止·进行中；项目页=最近节点·剩余天数；文案页=本周产出计数）。数据全部来自既有确定性命令（pending_writes list、weekly/project 汇总），**不新增模型调用、不编造数字**；046 的 AI 复盘建议区与降级规则原样保留在状态条下方。
2. **S2 文档库视图：** 新增 ArtifactExplorer：左侧目录树（树状模式数据已有）；右侧成果卡片/列表切换，卡片显示 title（frontmatter）、日期、场景标签、字数；顶部筛选条按 scenario 标签 + 本周/本月时间过滤（frontmatter 数据从 task_read_offline_artifact 读，列表页懒加载标题，避免全量读正文）。搜索框过滤文件名/标题。
3. **S3 会话历史折叠：** 场景页左栏 300px 会话列表移到底部折叠区（默认收起，展开显示现有列表 + 去对话）；retag/删除功能原样保留。
4. **S4 场景差异模块：** 工作记录页右侧新增"本期记录碎片"区（本周对话中已确认的事项条目，来自现有确定性汇总）；项目页保留"进度"页签（现有，不重做）。
5. **S5 首页今日重点接真数据：** HomeCanvas"今日重点推进"卡片摘掉"未接入数据源"徽标，接入跨场景待确认+逾期汇总（与 S1 同数据源）；无数据时显示"今天没有要处理的"诚实空态。ThreeWeekCalendar 不动。
6. **S6 证据与记录：** 真实窗口截图（三场景各 1 张全页 + 状态条展开待确认 1 张 + 筛选生效 1 张）存 evidence/od-arch-060/；STATUS 登记；REVIEW 写 059 复核结论与 060 验收结论。

**验收：** tsc / cargo lib（含新增汇总单测）/ pytest / npm build 全绿；runtime/ 零差异；状态条数字与 pending_writes 实际条数一致（单测断言）；frontmatter 筛选对 059 新格式成果生效（用合成成果验证）；旧成果（无 frontmatter）在视图中按文件名显示不报错（兼容单测）；三场景入口隐藏声明联动不回退（复跑 verify_scene_decl_linkage_055.py）。

**风险与停止条件：** ①懒加载标题若导致视图首屏卡顿或读文件过多（>100 文件），停下报告，改为按需分页；②frontmatter 解析对用户手改文件失败时按"无元数据"降级显示，不报错不阻塞；③发现需改既有命令语义才能取数 → 停下报告，不擅自改。

### OD-ARCH-059：综合批次（056 修复 + 057 对话归一 + 058 Obsidian 兼容）（已批准合并执行，2026-10-22）

> **背景：** 用户指示将 056/057/058 三项合并为一份计划交 BUILDER（DS v4.1-flash）执行。三块内容互相独立（设置修复 / 界面重构 / 成果库格式），可分批提交、独立验收。**本计划获批即视为 056/057/058 一并批准**，其详细步骤以下文 S1–S3 三节为准（各节原文来自对应计划，此处为合并后的权威版本）；若 BUILDER 发现三节间有冲突，以后文为准并报告冲突点。

**目标：** ①修掉夜间批次复核遗留缺陷；②全应用对话归一到首页、场景页纯档案化、"周报"改名"工作记录"；③成果库成为 Obsidian 兼容 vault。三项完成后用户即可开始真实场景试用。

**总影响范围：** desktop/src（前端）、desktop/src-tauri（pending_writes frontmatter、settings vault 绑定；**不改 lib.rs 既有命令语义**）、scripts/setup.sh 与 init-yvan-data.sh（仅注释）、samples/profiles/discipline.yml（入口改名）、docs/OmniDesk-Desktop-Data-Layout.md、docs/user-testing-checklist.md（对话与 vault 新流程）、PLANS/STATUS/REVIEW。

**通用红线（三节共用）：** 不碰 runtime/（零差异验收）；不碰凭证、不改路由；真实材料不进 Git/evidence；旧 YvanAI 与既有成果文件零迁移零覆盖零删除；内部 scenario 键（weekly_report 等）与数据目录名不变；提交沿用一次性授权分批进行，每批前红线扫描+文件清单核对。

---

**S1 = 056 复核遗留修复（先行，最小批）：**

1. 凭据状态收敛：前端在 view 已加载且 keyStatus 为 null 且无 key_error 时显示"尚未配置"（现状：永久"读取中…"）。不新增命令、不改 Rust。核对三态文案齐全：未配置 / 已配置（含来源与可否改写）/ 读取失败。
2. setup.sh 口径对齐：仅改 setup.sh 与 init-yvan-data.sh 顶部注释，写明 ~/YvanAI 为项目早期数据根、现存数据只读使用、新部署仍初始化到该路径以保持脚本兼容、桌面端成果不写此目录。不改行为、不改默认路径。
3. Dock 图标可访问名：核对 Dock.tsx 每个图标是否带 aria-label；缺则补（纯属性，同值用于自动化定位）。
4. STATUS/REVIEW 登记；REVIEW.md 顶部 051~055 复核结论随本批一并入库提交。

**S2 = 057 对话中枢归一（S1 全绿后）：**

1. 首页 HomeChat 输入区加场景选择 chip（默认"自由对话"，可选 工作记录/文案写作/项目跟进，与模型选择器同排）。选定后会话携带场景预设与归档目标；会话中途可改标签（复用 retagThread）。首页历史列表按场景小标签展示、可筛选。
2. 场景页纯档案化：ScenePage 移除 HomeChat，布局单层化（提醒/公告栏 + 会话列表（每条带"去对话"跳首页加载该会话）+ 成果档案；项目跟进保留进度页签）。SceneScaffold 双层逻辑仅首页保留。
3. 改名"周报"→"工作记录"：仅展示层（Dock 入口名、前端文案、空态提示、声明样例 discipline.yml）；内部键 weekly_report 与成果目录不动，旧数据零迁移。代码 grep 核对展示层无"周报"残留。
4. 首页卡片层（今日概览三模块 + 合并提醒）保留现状，不重做。
5. 声明联动：隐藏某场景入口后，首页场景选择器同步隐藏对应选项。

**S3 = 058 Obsidian 兼容（S2 全绿后）：**

1. frontmatter 标准（仅新文件）：pending_writes 与保存成果共用的写入口（write_new_file_durable 调用前）注入 YAML frontmatter：scenario / title / created(ISO8601) / tags / omnidesk_version。字段固定小集合，供 Obsidian Dataview 直接用；frontmatter 不进红线扫描正文（扫描仍只对用户原文）。
2. vault 绑定：设置页成果库区新增"vault 目录"显示与修改（默认 ~/OmniDesk/workspace/artifacts；改绑后新成果落新目录，旧目录文件不动不迁）。不做多 vault。
3. 接入指引：成果档案页与设置页各一步说明（在 Obsidian 中"打开文件夹作为 vault"指向绑定目录）。Obsidian 本体不安装、不检测、不拉起。
4. 边界声明：OmniDesk 只写自己的成果文件，不读不改用户在 vault 中的其他笔记；用户在 Obsidian 端的整理/改名/删除，OmniDesk 不感知不干预。

---

**验收（每 S 节收尾独立跑）：**

| 节 | 必跑 | 专项核对 |
| --- | --- | --- |
| S1 | tsc 0 / cargo lib 全绿 / pytest / runtime 零差异 | 凭据三态文案；setup.sh --check 行为不变 |
| S2 | 同上 + npm build | 首页选工作记录后成果落 weekly_report 目录；旧会话"去对话"跳转可续聊且历史完整；Dock 改名生效；隐藏入口联动 |
| S3 | 同上 | 新成果首行为可解析 frontmatter（serde_yaml 单测）；既有成果文件哈希不变（单测断言）；vault 改绑后新文件落新目录、旧目录无写入 |

全部证据截图存 tests/verification/desktop/evidence/od-arch-059/（分 S1/S2/S3 子目录），密钥与真实材料不得出现在任何证据中。

**风险与停止条件（合并版）：** ①S2 移除场景页对话后发现某能力依赖场景页对话上下文 → 停下报告，不擅自加回；②retagThread 中途改标签丢历史映射 → 停下报告；③S3 发现保存入口不止 write_new_file_durable 一处 → 停下报告清单，不擅自全改；④vault 改绑后读档列表越界 → 沿用 resolve_local_data_root 校验，不够用就停；⑤同一问题连续两轮修复未解决即停，写根因与阻塞报告；⑥发现疑似真实材料/凭证、runtime/ 出现差异、需要改 DSH Runtime → 立即停并报告。

**执行顺序与批次建议：** S1（1~2 批）→ S2（2~3 批：结构重构 / 改名+联动 / 证据）→ S3（2 批：frontmatter+单测 / vault 绑定+指引+证据）→ 收尾批（STATUS 总结 + REVIEW 材料 + 交接）。预计 7~9 个提交。完成后停止等 ARCHITECT 复核，不自行开新计划。

<details>
<summary>原始分计划归档（056/057/058 条文，已由 059 取代）</summary>


### OD-ARCH-057：对话中枢归一 + 场景页纯档案化（待批准，2026-10-22）

> **背景：** 用户实测后提出界面重构方向：每个场景页都重复挂对话框，只有卡片不同——应对话归一：首页对话框内选场景（周报改名"工作记录"），内容和成果按所选场景归档；Dock 场景页只做文件/文档管理，不再出现对话框。本计划将该方向落为实施项。

**目标：** 全应用只保留首页一个对话实例；场景选择收进对话框；场景页转为纯档案管理页。

**影响范围：** desktop/src 前端（App.tsx、HomeChat.tsx、ScenePage.tsx、SceneScaffold.tsx、Dock 相关）、samples/profiles/discipline.yml（改 Dock 入口名）与数据根声明样例、PLANS/STATUS/REVIEW。不碰 Rust 宿主（thread store 与成果按 scenario 归档的机制现成）、不碰 runtime/（零差异验收）、不改内部 scenario 键与数据目录（weekly_report 等键不变，**不做任何数据迁移**）。

**步骤：**
1. **S1 首页对话场景选择器：** HomeChat 输入区加场景选择 chip（默认"自由对话"，可选 工作记录/文案写作/项目跟进，与模型选择器同排）。选定后该会话携带场景预设与归档目标；会话中途可改标签（复用现有 retagThread）。首页历史列表按场景小标签展示、可筛选。
2. **S2 场景页纯档案化：** ScenePage 移除 HomeChat，布局改为单层：提醒/公告栏 + 会话列表（每条带"去对话"跳首页加载该会话）+ 成果档案（项目跟进保留进度页签）。SceneScaffold 双层焦点/静止恢复逻辑仅首页保留，场景页不再挂载。从档案页跳转 = onNavigate 到 home 并预载对应 thread。
3. **S3 改名"周报"→"工作记录"：** 声明样例 discipline.yml 的 Dock 入口名、前端展示文案、空态提示全部改用"工作记录"；内部 scenario 键 weekly_report、成果目录、线程标签**一律不动**（旧数据零迁移零覆盖，只改显示名）。声明样例与前端回退文案保持一致。
4. **S4 首页卡片层保留现状：** 今日概览三模块与合并提醒作为首页空闲卡片层（现有双层逻辑已实现，不重做）；公告栏随场景页保留（S2）。
5. **S5 记录与证据：** STATUS 登记、REVIEW 复核结论；真实窗口截图（首页含场景选择器、场景页无对话框、改名后 Dock 与空态）存 tests/verification/desktop/evidence/od-arch-057/。

**验收：** tsc、cargo lib、pytest、npm build 全绿；runtime/ 零差异；行为核对：首页选"工作记录"对话后成果落 weekly_report 目录；旧会话从档案页"去对话"跳转后可继续且历史完整；"周报"字样在 UI 不再出现（代码 grep 核对展示层）；隐藏场景入口后首页选择器同步隐藏对应选项（声明联动）。

**风险与停止条件：** ①场景页移除对话后若发现某能力依赖场景页内的对话上下文（如项目进度与对话联动），停下报告，不擅自加回；②retagThread 中途改标签若发现会丢历史映射，停下报告；③改名仅限展示层，若发现内部键有"周报"字样外泄到用户可见的文件路径，记录但不改（避免数据迁移），在执行记录中说明。

### OD-ARCH-058：成果库 Obsidian 兼容（待批准，2026-10-22）

> **背景：** 用户提出用 Obsidian 兼容文件夹作为本地知识库底座，让用户绑定 Obsidian 获得成熟的知识管理能力，OmniDesk 不自建。方向成立：成果已是 Markdown 文件、数据在仓库外用户目录，天然贴近 Obsidian；差距在目录语义与元数据（无 frontmatter），且须守住红线（不自动上传、不反向改用户手写内容）。

**目标：** 成果目录即 Obsidian vault；成果带标准 frontmatter；用户可把任意 vault 目录绑定为成果库；同步向用户提供接入指引。**不做**任何"发送到 Obsidian/同步服务"的自动上传功能。

**影响范围：** desktop/src-tauri（pending_writes 写入时补 frontmatter、settings 增加 vault 绑定项）、SettingsPanel（成果库区显示绑定路径与"在 Obsidian 打开"指引）、docs/OmniDesk-Desktop-Data-Layout.md（vault 口径）、PLANS/STATUS/REVIEW。不碰 runtime/（零差异验收）、不动旧 YvanAI 与既有成果文件（frontmatter 只对**新写入**的文件生效，已有文件不改）。

**步骤：**
1. **S1 frontmatter 标准（新文件生效）：** pending_writes 与保存成果共用写入口（write_new_file_durable 调用前）注入 YAML frontmatter：scenario、title、created（ISO8601）、tags（场景标签）、omnidesk_version。字段固定小集合，供 Obsidian Dataview 类插件直接用。已有文件零改动；frontmatter 不进红线扫描正文（扫描仍只对用户原文）。
2. **S2 vault 绑定：** 设置页成果库区新增"vault 目录"显示与修改（默认即 ~/OmniDesk/workspace/artifacts；改为其他目录时新成果落新目录，旧目录文件不动不迁）。不做多 vault，就一个绑定项。
3. **S3 打开指引：** 成果档案页与设置页提供一步说明：在 Obsidian 中"打开文件夹作为 vault"指向绑定目录；Obsidian 本体不在本计划范围（不自动安装、不检测、不拉起）。
4. **S4 边界声明：** 设置页与文档写明：OmniDesk 只写入自己的成果文件，不读取不改写用户在 vault 中的其他笔记；用户在 Obsidian 端的整理、重命名、删除 OmniDesk 不感知不干预（读档列表按场景过滤时自然反映文件现状）。

**验收：** tsc、cargo lib（含 frontmatter 生成单测：字段齐全、非法字符转义）、pytest、npm build 全绿；runtime/ 零差异；新成果文件首行为 frontmatter 且 Obsidian 语义可解析（单测断言 YAML 可被 serde_yaml 反序列化）；旧成果文件哈希不变（单测断言不触碰既有文件）。

**风险与停止条件：** ①发现现有保存入口不止 write_new_file_durable 一处（会漏 frontmatter），停下报告清单不擅自全改；②vault 改绑后若发现读档列表跨目录权限边界问题（场景目录校验），沿用现有 resolve_local_data_root 校验逻辑，不够用就停；③不做 Obsidian 安装检测与自动拉起（超出范围）。

### OD-ARCH-056：复核遗留修复 + YvanAI 目录口径定案（待批准，2026-10-22）

> **背景：** 056 起夜间批次收尾后进入修复小批模式（用户 2026-10-22 口径：小修不设长计划，验收按实际改动范围跑必要回归）。本计划内容即夜间批次复核发现的 054/055 两处缺陷 + 一项待用户拍板事项的定案执行。REVIEW.md 顶部 051 通过结论与 053/054/055 复核结论随本计划记录批次一并提交。

**目标：** 修掉凭据状态不收敛与 setup 脚本目录口径矛盾；把 YvanAI 初始化问题按用户决定收口。

**影响范围：** desktop/src/SettingsPanel.tsx（凭据状态收敛）、scripts/setup.sh（注释与目录口径）、docs/OmniDesk-Desktop-Data-Layout.md（如需同步口径）、PLANS/STATUS/REVIEW。提交推送沿用一次性授权模式（每批前红线扫描+文件清单核对）。不碰 runtime/（零差异验收）、凭证、路由、用户数据。

**步骤：**
1. **S1 凭据状态收敛（修 054 缺陷）：** 现状（源码核实）：Rust 侧 key_status 仅在有已存配置时查询，未配置时返回 null 且无 key_error；前端渲染链 keyStatus→key_error→"读取中…" 落到永久"读取中"。修法：前端在 view 已加载且 keyStatus 为 null 且无 key_error 时显示"尚未配置"，与 placeholder 口径一致；不新增命令、不改 Rust。
2. **S2 setup.sh 口径对齐（不改行为，改声明）：** 现状（源码核实）：setup.sh 第 22 行注释明确"只在仓库内与 ~/YvanAI（外部数据）写入"，第 2 步调 init-yvan-data.sh 默认根 ~/YvanAI——行为与注释一致，但与数据布局文档"OmniDesk 统一命名、YvanAI 只读不迁移"的迁移后口径矛盾。修法：在 setup.sh 与 init-yvan-data.sh 顶部注释中写明历史原因（该目录为项目早期数据根，现存用户数据继续只读使用，新部署同样初始化到该路径以保证既有脚本兼容；OmniDesk 桌面端成果不写此目录）。不改脚本行为、不改默认路径、不迁移数据。
3. **S3 Dock 选择器核对（只核对，不改）：** 核对 Dock.tsx 图标是否已带稳定可访问名（aria-label 或 title）；已有则记录结论即可，没有则为每个图标补 aria-label（纯属性，不影响布局，同值用于自动化定位）。
4. **S4 记录：** STATUS 登记、REVIEW 写 053/054/055 复核结论与本计划验收结论。

**验收：** tsc 0、cargo lib 全绿、pytest 通过、runtime/ 零差异；凭据状态三态（未配置/已配置/读取失败）在代码审查中确认各有明确文案；setup.sh --check 行为不变（可跑一次对照）。

**风险与停止条件：** S1 若发现需改 Rust 返回结构才能修复（与源码核实结论不符），停下报告不擅自改；S2 仅注释改动，若发现脚本实际行为与注释不符的更深问题，停下报告。

> **夜间批次（用户 2026-10-22 已批准连夜执行）：** 053 → 054 → 055 依序执行；每个计划收尾先跑本计划全部回归，全绿才进入下一个；任一步失败或触发停止条件，立即停下写阻塞报告，不自行扩大范围。三份计划共含一次性分批提交+推送授权（沿用 047/049/051 模式：每批提交前跑红线扫描+文件清单核对，附在执行记录中；发现疑似真实材料或凭证即停）。

### OD-ARCH-053：原生设置页替代 DSH 设置窗口（已批准，2026-10-22）

> **背景：** 用户实测确认「打开 DSH 设置」实际弹出完整 DSH Web 应用首页（缺陷；设置面板是 DSH 应用内弹层，官方无直达入口）。用户拍板方案 A：OmniDesk 自建原生设置页，DSH 完整应用入口降级隐藏。本计划修订决策 D-026（执行完成后在 docs/decisions.md 追加取代记录）。

**目标：** 模型端点/密钥/权限全部在 OmniDesk 原生设置页内完成；设置不再依赖 DSH UI；场景声明（profile）入口进设置页。

**影响范围：** desktop/src-tauri（新增宿主配置命令）、desktop/src/SettingsPanel.tsx（重构为原生表单）、docs/decisions.md（D-026 追加取代记录）、PLANS/STATUS/REVIEW。不碰 runtime/ 源码（零差异验收）、不改 DSH Runtime、凭证不入日志/Git/evidence。

**步骤：**
1. **S1 Rust 配置层：** 新增宿主命令读写模型配置（BUILDER 先核实现有 provider 配置实际加载源——预计为数据根 ~/OmniDesk/runtime/dsh-home/profiles/web/cordis.patch.yml，确认后以实际为准并在执行记录写明）。字段：provider 名称、base_url、api_key、默认模型、分区权限开关。密钥只落盘，日志/命令返回/evidence 一律打码（仅留尾 4 位）；写入用临时文件+原子替换；文件损坏时回退只读并明确报错，不静默重置。
2. **S2 原生设置页：** SettingsPanel 改为四区表单：模型与端点 / 场景声明 / 权限 / 关于（高级）。「刷新端点信息」改读 S1 层；保留 052 重启提示逻辑不动。
3. **S3 场景声明区：** 显示 ~/OmniDesk/app/profiles/discipline.yml 绝对路径与当前载入指纹；「在系统编辑器打开」按钮（打开数据根文件，非仓库文件）；内嵌编辑框（载入→校验→写前自动备份→保存），校验失败禁止保存并显示原因；保存后显示"重启应用生效"（复用 decl_digest 现有机制）。
4. **S4 DSH 入口降级：** 主界面移除「打开 DSH 设置」按钮；在"关于（高级）"折叠区保留「打开 DSH 原生界面（诊断用）」，默认折叠，文案注明会打开完整 DSH 应用。

**验收：** cargo（含新增单测：原子写/密钥打码/损坏回退/patch 读写往返）、tsc、npm run build 全绿；git status --porcelain runtime/ 为空；代码审查确认设置页不请求 DSH 端口；真实窗口截图四区各一张（存 tests/verification/desktop/evidence/）；扫描全部 evidence 与日志确认无密钥明文。

**风险与停止条件：** ①写坏 patch 文件导致 DSH 起不来 → S1 先完成只读解析+往返单测，再开放写入，并用一次性副本实测写入后 dsh web 仍能启动；②实际配置加载源与预期不符 → 停下报告，不猜；③权限字段与 051 已有权限自管实现冲突 → 以已实现语义为准，不新造第二套。

### OD-ARCH-054：设置页深化与首次运行引导（已批准，053 全绿后执行）

**目标：** 用户拿到新构建后，不开文档即可自助完成"配模型 → 测连通 → 跑场景"。

**步骤：**
1. **S1 连接测试：** 设置页「测试连接」按钮——对 base_url 发 OpenAI 兼容最小请求（/models 或最短补全，载荷固定合成短句），超时 10 秒，结果显示成功/失败原因（网络/鉴权/格式三类起步）。
2. **S2 模型列表拉取：** 鉴权成功后拉取模型列表填充"默认模型"下拉；失败降级为手动输入，不算测试失败。
3. **S3 首次运行引导：** 启动检测无 provider 配置时，主界面显示一步引导卡（跳设置页），配置完成后消失；已有配置的用户不受影响。
4. **S4 权限开关核对：** 核对 051 权限自管项在原生设置页可见可改；已可用则只核对不重做，结论写 STATUS。
5. **S5 文档：** docs/user-testing-checklist.md 设置段落改为原生页流程。

**验收：** 同 053 回归口径；对不可达端点与错误密钥各有明确失败态截图（载荷不含任何真实材料）。

### OD-ARCH-055：场景页收尾核对（已批准，054 全绿后执行；只核对+小修，不新增功能）

**目标：** 三场景（周报/文案/项目跟进）在用户实测前排掉低级问题。

**步骤：**
1. **S1 成果路径核对：** 逐场景验证成果实际保存位置符合统一命名（OmniDesk 数据根；旧 YvanAI 路径只读兼容不迁移）；发现不符仅修路径常量/文档，不动数据。
2. **S2 空状态检查：** 各场景页在无历史会话、无成果时的空状态文案与引导是否明确；缺失则补文案（一处文案一提交粒度）。
3. **S3 场景声明联动核对：** 在 discipline.yml 隐藏一个场景入口 → 重启后入口消失且对应页面不可直达 → 恢复文件。全程用副本演练，结束后声明文件与演练前一致。

**验收：** 同 053 回归口径；S3 演练前后 decl 指纹一致记录。

**夜间总停止条件（三计划共用）：** 同一问题连续两轮修复未解决即停，写根因与阻塞报告；发现疑似真实材料/凭证、runtime/ 出现差异、需要改 DSH Runtime 才能继续时，立即停并报告，等 ARCHITECT 与用户决定。

### OD-ARCH-052：上手清单刷新 + 重启提示 + 移交用户实测（待批准，2026-10-21）

> **背景：** 051 复核通过（见 REVIEW.md 顶部）后，技术链路（会话续接、Profile 声明、设置内嵌、权限自管）全部就位。剩余关键路径是**用户真实模型端到端与三场景日常试用**——只能由用户本人执行。052 是移交前最后一小批。

**目标：** 让用户拿到一份与当前构建一致的实测清单；补上"声明改动需重启"的界面提示；把球交到用户手里。

**影响范围：** docs/user-testing-checklist.md（重写对齐 050/051 现状）、desktop 前端一处提示（条件渲染+文案）、PLANS/STATUS/REVIEW。提交推送沿用一次性授权模式（**用户批准本计划即批准执行中的分批提交与推送**）。不碰 runtime/、凭证、路由。

**步骤：**
1. **重写 user-testing-checklist**：开权限（应用内 DSH 设置窗口，"跟随 DSH 设置"为默认）→ 选模型 → 三场景各跑一单（周报/文案/项目跟进）→ 写确认卡流程 → 强杀进程重启恢复 → 改 profiles/discipline.yml 验证重启生效 → 记录卡点。每步标注预期现象与"不算数"的情形（如离线模板冒充模型产出）。
2. **前端重启提示**：声明文件被修改后（decl 与上次载入不一致或用户改文件后）界面显示"重启应用生效"提示，一处文案不改逻辑。
3. **STATUS.md 登记** + 051 遗留清单核对（清单化确认无遗漏）。

**验收：** 清单每步可在当前构建上照做；cargo / tsc / build 全绿；runtime/ 零差异。

**风险与停止条件：** 无架构风险；清单描述与实测不符时以实测为准修正；不新增功能面。

> **执行状态（BUILDER，2026-10-21）：三步完成，待 ARCHITECT 复核。**
> - ①`docs/user-testing-checklist.md` 已按 050/051 现状重写为 6 步 + 记录表，每步含预期现象与"不算数"的情形；界面文案逐条对照源码核对。②新增"重启生效"提示：`LoadedDecl` 增 `content_sha256`、新增只读命令 `profile_decl_digest`，前端 5 秒比对一次，仅在启动时载入的指纹与磁盘不一致时显示提示；**未引入依赖、未改解析语义、未热重载、未自动覆盖用户文件**。③STATUS.md 已登记，051 五项遗留逐条核对（均不阻塞）。
> - 回归：cargo 172（lib，含新增 3 个指纹单测）+2+13+3+1 全绿 0 失败；cargo build 通过；tsc 0；npm run build 通过；pytest 26 passed；runtime/ 零差异。
> - 真实窗口：`verify_decl_restart_hint.py` 6 项自动断言全 true（改文件出提示 / 用户改动未被覆盖 / 删文件换文案 / 不热重载 / 重启后提示消失且像素与冷启动一致 / 重启才重建默认声明）。
> - 限制：真实模型端到端仍未验证（需用户本人按清单执行）；"启动卡死"仍未复现、未认定修复。
</details>


### OD-ARCH-051：UI 合一 + 原生多轮会话 + 纪检 Profile 固化 + 设置页外观统一（待批准→派工，2026-10-21）

> **背景（用户 2026-10-21 口径修正后的定位）：** ①纪检是**一个** profile，周报/文案/项目跟进是它内部三个场景，不是三个 profile；②会话与历史存 OmniDesk 自有数据根、不依赖 DSH（已实现），缺的是接 DSH 原生会话续接（当前每轮新开会话+重放历史）；③旧 DSH 桌面会话导入功能**不存在、本计划不做**，需用户明确要求才立项；④设置页继续用 DSH 原生页面（D-026），仅外观向 OmniDesk 统一。

**目标：** UI 只留一份（合并用户实际使用的 omniharness-ui 版本）；对话不再每轮重放；纪检 profile 以文件形式固化（Skill+插件+默认设置声明）；设置页窗口外观统一。

**影响范围：** `desktop/`（Rust 宿主 + React 前端）、`engine/omnidesk_engine/web_api_adapter.py`、配置文件（纪检 profile 声明 + 场景入口注册）、PLANS/STATUS/REVIEW/GOALS/decisions.md、提交推送（沿用 047/049 一次性授权模式：**用户批准本计划即批准执行中的分批提交与推送**）。不碰 `runtime/`（零差异验收）、不碰凭证/路由、不接真实工作材料。

**批次 1（先行，有依赖顺序）：**

1. **S1 备份与合并 UI**：`omniharness-ui`（HEAD `5ffc710`，71 个未提交文件）先在它自己的仓库分批提交（每批前跑红线扫描 + 文件清单核对，清单附在执行记录中供用户过目；发现疑似真实材料/凭证即停）→ 按 OD-ARCH-026 既定方式（逐文件审查后择取必要前端，不整支强并）合入主仓库 `desktop/` 前端，旧界面文件冻结归档不删除。验收：合并后 `npm run build` 通过、cargo 全量绿、UI 行为与 omniharness-ui 版本一致（三场景入口 + 首页对话中枢骨架）。
2. **S2 接 DSH 原生会话续接**：web_api_adapter 由「每轮 session/new + 历史重放」改为 DSH Web API 的会话续接（045 第 5 步探针已实测该能力存在）；失败降级为现有行为并记日志，不静默丢轮。验收：多轮对话 token 消耗对比记录（同任务重放版 vs 续接版，合成端点下输入量应显著下降）；对话历史正确性不回退（现有 thread_store 回归全绿）；会话中断恢复行为不回退。

**批次 2（批次 1 验收后）：**

3. **S3 纪检 Profile 固化**：新增一份 profile 声明文件（数据根 `~/OmniDesk/app/profiles/discipline.yml` 或仓库配置目录，位置由 BUILDER 按现有配置加载机制选定并在执行记录中说明），内容：Skill 清单、插件开关、默认设置；周报/文案/项目跟进注册为该 profile 下三个场景入口；预留第二个 profile 的加载位置（只留结构，不实现多 profile 管理界面）。验收：改声明文件、不动代码，场景入口可见变化（新增或隐藏一个场景入口即算证明）；配置加载失败时回退默认行为并明确提示。
4. **S4 设置页外观统一**：D-026 内嵌 DSH 设置窗口的标题、配色、字体向 OmniDesk 主界面统一；不改 DSH 配置读写逻辑、不改会话令牌处理。验收：真实窗口截图；DSH 配置读写回归测试不回退。

**风险与停止条件：** ①omniharness-ui 合并若发现与主仓库前端结构性冲突（同文件不同骨架），停下报告冲突清单，不自行取舍；②S2 若 DSH 续接接口与 045 探针结论不符（接口漂移），停下报告，不猜内部协议；③S3 profile 声明若需动 Rust 代码才能加载，停下报告（说明为什么文件不够）；④批次 1 验收不过，批次 2 不开工。

**验收总表：** 合并清单已过目并推送到 omniharness-ui 仓库；cargo / tsc / build 全绿；runtime/ 零差异；S2 token 对比数据落盘 audit/；S3 改文件即生效的证据；STATUS.md 执行记录 + REVIEW.md ARCHITECT 复核记录。

**明确不做（本计划）：** 旧 DSH 桌面会话导入；多 profile 管理界面；安装包重打；真实模型端到端（等用户批准端点后按 docs/user-testing-checklist.md 走）。

> **执行状态（BUILDER，2026-10-21）：批次 1 完成，S1 按用户裁决改为窄口径整合。**
> - **S1 收口：命中停止条件①（结构性冲突）后由用户裁决，按窄口径完成。** `omniharness-ui` HEAD `5ffc710`（10-16）的未提交前端，已被主仓库 `2705148`（10-20「引入桌面 React 前端」）按**同一份 Aetheria OS 设计稿**落地并继续演进到 048/049/050；两边 `App.tsx`（76/161 行）、Dock 场景（4/8 个）、`api.ts`（239/828 行）骨架不同，整支合并会让主仓库丢掉会话、项目、设置、引擎诊断、线程持久化与流式对话。**未改任何前端源码文件，`omniharness-ui` 的 71 个未提交文件保持原状。**
>   **用户已裁决（2026-10-21）：「按最新的版本来，哪边最新用哪边，如果整合两边也行」→ 以主仓库现版为基线，只整合该分支独有且仍有用的资产。** 实际纳入：`_design/stitch_omnidesk/`（设计稿原件 7 个文件，主仓库 `theme.css` 的令牌注释本就指向这里但目录缺失）、`docs/omnidesk-ui-v2-handoff.md`、`tests/verification/desktop/verify_launcher_identity.sh`、`verify_port_ownership.sh`。未纳入：三个 `host_*_admission.rs`（依赖已被 048 重构掉的 `*_after_admission` 符号，直接复制编译不过；主仓库 `admission_classification.rs` 与 `host_integration.rs::admission_refuses_before_any_process_state_exists` 已覆盖同等能力）、`verify_engine_admission.py`（断言旧常量 `ENDPOINT_ISOLATION_CONFIRMED`，与 021 之后的宿主侧判定口径冲突）、`_preview.html`（临时预览）、`desktop/src/components/Icon.tsx` 等两边逐字一致的文件。因为不再整支合并，`omniharness-ui` 侧的 71 个未提交文件保持原状、未提交。
> - **S2 实现已在 048 落地，本轮补齐验收要求的对比数据。** 证据：`tests/verification/desktop/verify_resume_token_savings.py` → `tests/verification/desktop/evidence/od-arch-051/s2-token-savings.json`，11/11 通过。
>   **实测推翻了计划里的假设：** 前端组装层面三轮输入从 252 字符降到 78（省 69%），但**模型实际收到的量反而更大**（续接轮 6488 字符/6 条消息 vs 重放轮 623 字符/2 条消息）——DSH 续接会话时会把会话历史一起送给模型。因此本计划的「合成端点下输入量应显著下降」只在提示词拼装层面成立，**不能表述为节省 token 或节省费用**。
> - **S3、S4 未开工**，等 ARCHITECT 复核批次 1 后再决定是否开启批次 2（→ 用户 2026-10-21 指令「如果详细计划已经有了就继续往下」后开启，结果见下段）。
> - 未提交、未推送、未打标签；`runtime/` 零差异；全程合成数据 + 一次性数据根。

> **执行状态（BUILDER，2026-10-21）：批次 2 完成（S3 + S4），待 ARCHITECT 独立复核。**
> - **S3 完成，文件驱动。** 声明落 `<应用数据根>/profiles/discipline.yml`（首次用 `create_new` 创建，永不覆盖）。只决定 Dock 上的业务场景与品牌名；不改插件/路由/Skill/判定。**解析器是自写的受限 YAML 子集，未新增依赖**（离线构建，锁文件无 YAML crate）——遇到超出子集的写法（制表符、未知键、重复字段、嵌套映射、未闭合引号）一律报错，不猜测兼容。失败时回退内置入口并在界面显式报错，坏文件原样保留。内置工具入口（任务工作台/设置/引擎诊断）由代码固定，声明不可删改。
> - **S4 只完成窗口外壳**：标题 `OmniDesk · DSH 设置`、底色 `#faf8ff`、1160×820。**DSH 页面内部的字体与配色未改**——该窗口加载 DSH 原生页（D-026），样式由 DSH 控制，CSP 不允许本应用注入脚本/样式。**「外观统一」目前仅到窗口外壳这一层**，验收记录不得写成页面已主题化。DSH 配置读写与令牌处理未动。
> - **测试中发现并修复的缺陷：** 声明异步到达时，原校准逻辑会把正停留在设置页的用户强行切回首页（内置工具页不在声明管辖范围内）。已改为只校准 5 个业务场景。属普通实现缺陷，未改架构。
> - **真实窗口验收（非仅单测）**：①默认启动 → 宿主创建声明文件且与 `samples` 逐字一致，Dock 5 个业务入口；②改文件（去掉「文案写作」、改名「我的纪检工作台」）→ 重启后入口消失、品牌名变化，**未改代码**；③写坏文件 → 顶部红条报出路径与原因、回退内置入口、坏文件未被覆盖、进程存活；④设置窗口标题与尺寸符合，主窗口存活。证据：`tests/verification/desktop/evidence/od-arch-051/profile-decl-run.json`、`settings-shell-run.json` 与 `shots/`。
> - **回归：** cargo `169 passed / 0 failed`（lib，含新增 9 个声明单测）+ 2 + 13 + 3 + 1 全绿；`cargo build` 通过；tsc 0；`npm run build` 通过；pytest 26 passed；`runtime/` 零差异。
> - **限制：** 声明改动需重启应用生效（启动读一次，无热重载，界面未提示）；设置页内部外观未统一（见 S4）；`11-settings-window.png` 因 Xvfb 无窗口管理器只作旁证，以 `12-settings-window-only.png` 为准；用户报过的「启动卡死」在本批次两轮真实窗口中**未复现**，不认定已修复。声明文件是界面层配置，不是 DSH Profile。
> - 按计划授权范围分批提交并推送（A 代码 / B 测试与证据 / C 记录）。
> - **需 ARCHITECT 裁定的三处（BUILDER 未自行扩大）：** ①声明只做「场景入口 + 品牌名」，**未**把 Skill 清单/插件开关/默认设置写进去——那三样在 DSH 侧已有权威源，抄进界面声明即成双写；若要求声明一并掌管，属新增数据契约。②仓库原本无「读声明文件」机制，必须新增 Rust 模块（`settings.rs` 只读设置 JSON）；计划的停止条件③与影响范围（含 Rust 宿主）存在张力，按影响范围判断为在界内。③S4 只统一窗口外壳，页面内部受 CSP 限制未改。
> - **Git：** `18d133f` 代码 / `31b6d4a` 测试与证据 / `3f41f4e` 记录，已推送 `origin/main`。

### OD-ARCH-050
### OD-ARCH-050：修「新建任务后模型消失」+「打开 DSH 设置」改为应用内独立窗口（已执行，2026-10-21）

> **执行结果（ARCHITECT，2026-10-21）：用户批准（内嵌 DSH 设置页 + 同意清理旧进程）后执行。**
> **额外发现并修复的根因（计划外，用户当场症状的真正来源）：** 引擎子进程「启动后立刻退出」——`web_api_adapter._ensure_started()` 起 DSH 时没收回 stdin，DSH 继承了引擎的协议管道，Node 把这份共享的「打开文件描述」设为非阻塞，引擎随后 `for line in sys.stdin` 读到假 EOF（rc=0、无栈）安静退出；表现为模型目录永远读不到、引擎一启动就没。修复：`stdin=subprocess.DEVNULL` + 回归单测 `test_dsh_child_stdin_is_devnull`。
> **S1：** 宿主把 `EngineConfig` 快照放进独立 managed state（`HostSnapshot`），`engine_admission_status` 不再抢 `SharedHost` 锁 ⇒ 引擎冷启动期间也能读到端点事实（实测：应用启动 12 秒、引擎仍 `ENGINE OFFLINE` 时，界面已显示 `agnescn / agnes-3.0-flash`）。
> **S2：** 前端区分「读取中 / 已读到 / 读失败」，刷新期间保留上次事实；首次加载显示「读取中…」而不是「未识别」；失败时保留旧值并附「可重试」说明。
> **S3：** 「打开 DSH 设置」改为：复用本进程已起的实例 → 否则 `dsh web --port 0 --no-open`（输出按行打码后落日志）→ Rust 侧建**应用内独立窗口**「DSH 设置」。会话令牌只留宿主内存，日志 `token=***`；窗口不授予任何 IPC/文件能力；关闭窗口**不再**触发引擎收尾（只允许主窗口收尾，新增源码级测试）；退出时停掉自起的设置服务。
> **S4：** 已停掉 2026-10-20 18:27 起占用 127.0.0.1:3080 的旧 `dsh web`（pid 491674，旧版本按钮遗留），端口已释放。
> **S5（用户新要求）：** 新增「跟随 DSH 设置（推荐）」——归档 OmniDesk 覆盖文件（改名保留、不删除）+ 停引擎，之后新建任务/重启直接使用 DSH 里配置的 provider 与默认模型，不再要求在本应用重选；覆盖文件存在时仍以用户在本应用里的显式选择为准。修了随之暴露的两个问题：覆盖文件变化后 supervisor/事实快照里的 patch 列表不同步（会导致分类器拿到已不存在的补丁路径而拒绝），新增 `EngineConfig::sync_overlay` 并在写/归档后同步。
>
> **验证：** cargo 160 passed / 0 failed（新增 3 个源码级不变量测试 + 2 个 dsh_settings 单测）；`--test thread_store_restart` 1 passed；test_web_api_unit.py **26 passed**（含新增 stdin 隔离回归）；tsc 0；`npm run build` 通过；真实窗口实测：应用启动 12s 模型已知、引擎存活（`omnidesk_engine` 持续运行 3 分钟以上）、模型目录下拉显示 DeepSeek-V4.1-Flash/V4-Pro/AGNESCN、设置窗口独立打开并复用同一实例（同一端口）、关闭设置窗口后引擎仍在、日志无明文 token、`runtime/` 零差异。
>
> **未验证/遗留：** 真实模型端到端回答（本轮不接真实端点做生成）；思考等级在「跟随 DSH」下由 DSH 配置决定（原覆盖文件里的 `reasoningEffort: off` 已随归档保留在 `.bak-*`，如需恢复可切回或重新选一次）；`omnidesk-model.yml.bak-1792578256` 留在用户数据根供回滚。



> **证据（代码事实，非推断）：**
> - `lib.rs` `with_engine_call` 在持有全局 `SharedHost` 锁时调用 `ensure_initialized`；后者持 supervisor 锁执行 `initialize(Duration::from_secs(90))`（`engine_bridge.rs`）。web-api 通道冷启动要起 DSH web 服务，耗时数十秒。
> - `engine_admission_status` 为取 `EngineConfig` 需要同一把宿主锁 ⇒ 引擎启动期间该读取一直排队到 init 结束（最长 90s）。前端此时 `facts=null` ⇒ 「模型：未识别」；设置页进入分支 `!facts && !factsError` 直接显示「端点事实还没读出来」（`SettingsPanel.tsx:344`），把「加载中」误报成「读不出来」。用户看到的「几十秒后模型又出现」＝ init 完成、锁释放。
> - `task_shell::open_dsh_settings` 每次点击都 `spawn node <dsh_cli> web`（固定端口 3080、默认打开系统浏览器）。2026-10-20 18:27 起的旧实例（pid 491674）仍占 3080；之后每次点击在约 70s 后 `EADDRINUSE` 失败，只写 `~/OmniDesk/app/logs/dsh-settings.log`，界面只显示「已启动」，没有任何窗口。日志首行含 `?token=…`（会话令牌，不得外泄）。
> - 现有 UI 没有「应用内 DSH 设置窗口」；OD-ARCH-031 的设计是「跳系统浏览器」。

1. **S1 端点事实与引擎启动解耦（宿主，P0）：** 宿主启动时固定一份 `EngineConfig` 快照放入独立 managed state；`engine_admission_status` 只读快照 + `config_dir`，**不再取宿主锁**。提交路径的准入与守卫语义一字不改（仍走 `with_engine_call`）。新增源码级不变量测试（该命令内不得出现 `state.lock()`）+ 并发分类单测。
2. **S2 事实读取 UI 三态（前端，P0）：** HomeChat / SettingsPanel 区分 `loading / ready / error`；刷新期间保留上次事实（显示「正在刷新」），只有首次加载才显示「加载中」；读取加 5s 超时，超时给重试提示但不清空已知事实。模型选择器在目录不可用时仍显示 facts 里的 provider/model，不再退成「未识别」。
3. **S3「打开 DSH 设置」改为应用内独立窗口（宿主 + 前端，P0）：** 新命令按以下顺序工作——读数据根 `runtime/dsh-settings.json`（pid/port/started_at）→ 进程活着且端口可连则复用 → 否则 `dsh web --port 0 --no-open`（日志落盘前把 `token=` 打码），15s 内解析 `dsh web: http://127.0.0.1:<port>/?token=…` → 由 Rust 侧建 `dsh-settings` 窗口（标题「DSH 设置」）并直接传入 URL（**不经前端、不写日志明文**）。失败返回结构化原因（未启动/超时/端口/日志路径）。窗口不授予任何 IPC 与文件能力；应用退出时停掉自起的设置服务。旧固定 3080 路径移除。
4. **S4 现状清理（需用户单独同意）：** 停掉 2026-10-20 18:27 起占着 3080 的旧 `dsh web`（pid 491674）——那是旧版本按钮留下的进程；不删任何数据、不动 FSH 数据根。

**验收：** ①冷启动首个任务期间连续 3 次打开设置页，「模型与端点」始终显示 `agnescn / agnes-3.0-flash`、事实读取 ≤1s、不出现「未识别/读不出来」；②点「打开 DSH 设置」≤15s 出现独立窗口并显示 DSH 设置页，关闭再点复用同一实例，重复点击只有 1 个服务进程，日志无明文 token；③失败路径（如 DSH CLI 缺失）给明确错误与日志路径；④`cargo test --offline` 全绿、`tsc` 0、`npm run build` 通过、`runtime/` 零差异。

**风险与停止条件：** 远程页面在 Tauri 窗口里的 CSP/权限行为需实测——若被 app CSP 拦住且无法在不放宽主窗口 CSP 的前提下解决，停下报告，改用「系统浏览器 + 界面给出地址」的降级方案等用户裁定；token 是会话令牌，禁止写入日志/仓库/前端可见区域，解析方式若随 CLI 输出变化则停下报告、不猜。

**需用户裁定：** ①设置页形态＝内嵌 DSH 自有设置页（推荐，复用 DSH 本体、不重复实现）／自研模型提供方设置页（会与 DSH 配置双写，违背 D-024 复用原则）；②是否同意 S4 停掉旧 `dsh web` 进程。

### OD-ARCH-049：备份提交 + 会话日志版本化 + 终检设计（已执行，2026-10-21）

> **执行结果（ARCHITECT，2026-10-21）：三段按序完成。**
> ① **备份提交**：63 个未提交路径（44 改 + 19 新）按可编译单元分 5 批提交并推送 `origin/main`（`5ba0738` 引擎 / `f3a5dae` 宿主 / `a728907` 前端与启动器 / `5a0664a` 测试与证据 / `b3802bb` 文档与审计），每批提交前跑 `/tmp/od029/scan-staged.py`（5 批全 OK，零关键凭证命中）并人工核对文件清单；`runtime/` 零差异；工作区 0 项。
> ② **会话日志版本化**：`thread_store` 写入统一打 `version: 1`；读取端 `migrate_thread_record` 只读迁移（v0→v1 只补版本号，**不改写原文件**），未来版本拒绝写入并逐条报原因（`ThreadListing.issues`，前端 `storageNote` 显示）。新增 4 个单测（旧格式样例加载成功、写入打版本、未来版本拒绝+报错、强杀残留 tmp/半截文件下好记录仍可读）。
> ③ **执行入口终检**：`engine_submit_task` 在红线扫描之后、触达引擎之前做一次 `pending_writes::scene_gate`；命中同场景未决卡即整条任务拒绝（错误码 `pending_write_blocked`，消息点名卡片摘要），未命中原样执行；非场景 intent（首页闲聊等）不进入拦截范围。新增 4 个单测（同场景拒绝+指名、他场景放行、已结算/过期放行、坏记录拒绝）+ 1 个源码级接线测试（终检必须先于 `with_engine_call`）。
>
> **两处实现口径偏差（均已登记 REVIEW.md）：** ①「同一资源」在提交入口只能精确到**场景成果目录**——提交请求里没有目标文件名，故以场景为最小可判定边界；②待确认记录目录里出现**无法解析**的记录时选择**拦住并说明**（fail-closed），而不是当作没有记录继续执行。
>
> **未验证：** 真实窗口点选验证（本轮为单元/源码级证据）；真实模型端点下「有未决卡」的现场拦截体验待用户实测。

1. **备份提交（沿用 047 授权模式，用户批准本计划即视为批准本轮提交推送）：** 048 全部未提交改动（44 文件：代码/证据/文档含 048 复核记录、dsh-java 审计报告、audit/dsh-java-review/）按“可编译单元”分批提交并推送 origin/main。每批前跑红线扫描与文件清单核对；runtime/ 保持零差异确认。
2. **会话日志版本化（借鉴 dsh-java SessionFormatMigrationTest，P1-2）：** ~/OmniDesk 线程持久化文件写入时加 version 字段（当前 v1）；读取端遇到无版本/旧版本文件走迁移函数——只增不改写原文件，失败保留原样报错，不静默跳过；新增单测"无版本旧格式样例→加载成功"。验收：单测通过 + 强杀恢复回归通过 + cargo 全量绿。
3. **执行入口终检（借鉴 dsh-java 任务链兜底，P1-1，按用户三条语义条款收紧）：** 宿主任务执行入口加一次状态校验——检查结果只有两种：通过→任务原样执行；不通过→整个任务拒绝，错误信息指名道姓（如"有一条未确认写入（AI 复盘建议保存），请先处理"），**永不静默跳过部分内容**。拦截范围最小化：仅同场景、同一资源的未决确认卡才拦截本任务，其他场景卡片不挡。新增测试用例钉住"同场景未决卡→拒绝+指名错误"与"他场景未决卡→正常执行"两条。验收：两用例通过 + cargo 全量绿 + tsc 0。

**验收：** 提交分批清晰、远端同步；版本化与终检各有独立提交可单独回滚；全部回归绿（cargo/tsc/build）；runtime/ 零差异；STATUS.md 登记。

**完成后：** 真实模型端到端实测移交用户（按 docs/user-testing-checklist.md），产品命运转入使用数据裁决。


## 当前待批准计划与最近复核

### OD-ARCH-082：三大场景 Skill 深化 + Profile 资产绑定（用户已批准 2026-10-25，已执行、ARCHITECT 复核通过）

> **批准依据：** 用户 2026-10-25 明确「批准执行 OD-ARCH-082」。本节经 ARCHITECT 修订三处已过期前提后为唯一实施口径；批准不放宽范围，未列入本节的事项不属于本轮。
>
> **修订说明（ARCHITECT，2026-10-25，均为文件事实）：** ①数据根 `~/OmniDesk/runtime/dsh-home/profiles/` 实际只有 acp / headless / omnidesk-test / web / yvan-work，**没有 discipline Profile**；仓库里 `samples/profiles/discipline.yml` 是 `omnidesk.profile-decl.v1` 的 Dock 场景声明，不是 DSH Profile。②`TurnRail.tsx` 的 tooltip **已有完整视觉样式**（圆角、边框、阴影、最大宽度、问句一行/回复三行截断），真实缺口只有主题色边与 `aria-describedby`。③`TopNav.tsx` 搜索框当前是 `disabled` 的诚实占位（「全局检索（未实现）」）。
>
> **派工前定案（四条口径，实施不得自行更改）：**
> 1. **两态 Profile 组成：** web-api 通道白名单为 `omnidesk-base`（默认）与 `discipline`。两者 bundles 均为 `@deepseek-ai/dsh-base` + `@deepseek-ai/dsh-web-app`；web-app 是本地 Web API 与前端托管的**运行基础设施**，不算业务插件。不复制 web Profile 的第三方插件（free-search / cost-meter），不复制 yvan-work 的 inspector。acp 通道保持单一 `acp`，不受白名单影响。
> 2. **Skill 可见性由 agent preset 决定，不由 Profile 顶层补丁决定（已实跑验证）：** web-app bundle 在 preset 层挂载 `skill-filesystem` / `tool-skill`，默认 preset 是 `standard`；Profile 顶层 `- id: skill-filesystem` 的 `disabled` 只作用于宿主层，**拦不住 preset 内的那一份**（`--dump-config` 实测：顶层已 disabled，preset-standard 内两条仍在）。因此 `omnidesk-base` 的补丁必须覆盖 `preset-standard` 的 `config.plugins`（补丁层对 `config` 是整体替换），保留上游条目与顺序、把 `skill-filesystem` 与 `tool-skill` 标记 `disabled: true`。该覆盖有漂移风险，必须配一个校验脚本比对上游 `web-app/presets/standard.patch.yml` 的条目 id 序列。
> 3. **安全底线不随 Profile 关闭：** 红线词表与隐私掩码仍由「规则包是否安装」唯一决定，**本轮不改** `RulePolicy` 的加载语义。因此「默认模式」的准确表述是：不挂业务插件、不注入任何 Skill 正文、不使用场景 Skill 绑定；若用户装过规则包，红线与掩码照常生效。界面文案必须按此如实显示，不得宣传成「等同全新 DSH」。
> 4. **已有保存值不静默改写：** 当前 `settings.v1.json` 的 `engine_profile` 是 `yvan-work`，不在新白名单内。解析时**保留保存值原样**，本次启动回落默认 `omnidesk-base` 并在徽标/弹层显式说明「当前保存的 Profile 不在可选清单，本次使用默认」；不得自动写回、不得删除该目录。
>
> **正式启动路径的既有缺陷（本轮 S6 必须一并修，属 S6 前置）：** `scripts/omnidesk.sh` 在子进程里无条件 `export OMNIDESK_DSH_PROFILE="${OMNIDESK_DSH_PROFILE:-web}"`，而宿主 `engine_supervisor.rs` 以环境变量为最高优先级，导致设置里的 Profile 在正式启动时永远不生效。改为：脚本不再注入该变量（外部显式设置仍优先），由宿主读取 settings；acp 通道的默认仍为 acp。

**背景（用户定调）：** 桌面体验已达到类 DSH-desktop 水平，进入「场景 + 配套 Skill + 必要插件」阶段。OmniDesk 有了定制的 profile 后才是真正的 DSH 二开作品。当前 6 个 Skill 已存在于隔离数据根（material-writing / knowledge-search / knowledge-agent / document-review / memory-agent / project-agent），但质量与场景绑定深度不足，且 Discipline Profile 的规则资产与 Skill 之间缺一次系统性对齐。

**目标：** ①把三大场景（工作记录/文案写作/项目跟踪）的 Skill 内容按真实工作流深化到可用水平；②Skill 与 Profile 规则资产显式绑定（每个场景启动时加载哪些 Skill、哪些规则段），不再靠隐式发现；③每个 Skill 带合成评测用例，回归可跑。

**影响范围：** 数据根内 skills/ 目录内容（受 .omnidesk-managed 管理）、samples/profiles/ 模板、设置页规则资产分页展示、engine/ 侧 skill 注入链路（如需）；不改 runtime/、不改协议、不碰凭证。

**步骤：**
1. **S1 Skill 现状盘点（只读）：** 逐个读 6 个 skill 的 `SKILL.md`，记录触发条件、输入输出、当前与场景的绑定方式、质量缺口。只按文件事实，不按记忆判断。盘点简表写入 STATUS.md。
2. **S2 场景-Skill 绑定契约（schema v1 → v2，向后兼容）：** 在 `discipline.rules.json` 增加 `skill_bindings`：每个场景声明 `skills`（skill id 数组，允许空数组）与可选的注入上限。同步改 Rust `profile_rules.rs`（结构仍 `deny_unknown_fields`）、TS `types.ts`、`SettingsPanel` 规则资产分页、`verify_profile_rules_068.py`。校验：id 必须在六项清单内、同场景不得重复、单场景上限 3 个。**v1 文件必须原样可加载**（无绑定即不注入），不得自动改写用户已装规则文件。
3. **S3 三场景 Skill 深化 + 合成用例：** 按已确认工作流（工作记录=随手记轻装+快速整理；文案写作=素材收集+结构生成+多轮打磨；项目跟踪=事项登记+进度汇总+逾期提醒）重写对应 `SKILL.md`，含触发词、输入要求、输出结构、质量自检、数据边界引用规则包。顺带修正 `memory-agent`、`project-agent` 里残留的「Yvan AI Workbench」与 `~/YvanAI/...` 路径（改为 OmniDesk 事实）。每个 Skill 附不依赖模型的合成评测用例（输入样例 + 期望行为要点），固化为可跑回归。
4. **S4 注入链路（前端提示词组装，不动 runtime）：** 注入点在 `desktop/src/conversationPrompt.ts`（`composeConversationPrompt`）、`desktop/src/aiDraft.ts`（`buildDraftPrompt`）与 `HomeChat` 的组装处。按「当前 Profile + 当前场景绑定」把对应 Skill 正文拼入提示词，受 S2 上限与既有 `CONTEXT_MAX_CHARS` 约束；随手记分支保持轻装（最多一个绑定 Skill）。校验：切换场景后注入内容确实不同；续答（continuation）每轮重新注入并有用例覆盖；无规则包/无绑定时提示词与现状一致。
5. **S5 回归与三场景证据：** 三场景各用一条合成输入走真实窗口，落提示词组装事实（含命中的 Skill id 与片段），不依赖真实模型；全量回归见验收。
6. **S6 两态 Profile（可运行，create-new-only）：**
   - 新建 `omnidesk-base`（默认）与 `discipline` 两个 Profile 目录，bundles 均为 `@deepseek-ai/dsh-base` + `@deepseek-ai/dsh-web-app`；**只允许新建**，不得覆盖或删除既有 `acp` / `headless` / `omnidesk-test` / `web` / `yvan-work`。
   - `omnidesk-base` 补丁覆盖 `preset-standard` 的 `config.plugins`，保留上游条目与顺序，仅把 `skill-filesystem`、`tool-skill` 标 `disabled: true`；配校验脚本比对上游 `web-app/presets/standard.patch.yml` 的条目 id 序列，漂移即失败。
   - `profile_choice.rs` 加白名单：web-api 通道只出 `omnidesk-base` 与 `discipline`（默认前者），acp 通道仍 `acp`；保存值 `yvan-work` 保留原样，本次回落默认并在徽标/弹层显示「当前保存的 Profile 不在可选清单，本次使用默认」。
   - 修 `scripts/omnidesk.sh`：去掉无条件 `export OMNIDESK_DSH_PROFILE` 默认值，改为由宿主读 settings（外部显式设置仍优先）。
   - 徽标与 plate-soft 视觉体系统一，不新造视觉语言。
   - 证据：两态各启动一次引擎并用 `--dump-config` 比对插件与 skill 可见性差异（合成端点即可），落 `evidence/od-arch-082/` 的 facts JSON 与截图。
7. **S7 TurnRail 悬停卡（只补缺口）：** tooltip 已有形状、底色、阴影、宽度与截断，**只加主题色边与 `aria-describedby` 关联**，保持现有 plate / surface token，不重写组件、不改布局。验收：悬停与键盘聚焦都出现该卡，截图一张。
8. **S8 顶栏当前会话搜索：** TopNav 搜索框去掉禁用态，改为「当前会话内查找」：查询态由 App 持有，同时传给 TopNav 与 HomeChat（两者是兄弟组件，需该最小通道）；按消息轮次/段落文本匹配，命中高亮、上/下跳转、计数 `n/m`，无命中显示 `0/0`，Esc 退出。不解析 HTML、不做跨会话检索、不做虚拟化。验收：多轮合成会话命中并跳转、计数正确；命中态与无结果态各一张截图。

**明确不做（本轮范围外）：** REVIEW.md 顶部预检 A/B/C/D 列举的修复事项（另立计划）；修改 `runtime/` 内 DSH Runtime；改动凭证、模型路由或真实业务材料；跨会话全局检索；重写 TurnRail 或 TopNav 其余部分；删除/迁移数据根已有 Profile 目录；提交、推送、打标签。

**过夜批次（用户批准一次跑完、不需逐步审核）：** 按顺序执行 S1→S8；S1 盘点、S6 两态差异、S8 搜索口径都写进 STATUS.md；S3 用例与 S6/S7/S8 证据落 `tests/verification/desktop/evidence/od-arch-082/`；S8 完成后统一跑全量回归；完成后在 STATUS.md 写完整记录并停下等 ARCHITECT 复核。遇阻塞（skill 发现机制与注入方式冲突、白名单影响已有用户配置）按 AGENTS.md 停止规则处理：记录、不硬编、不扩大范围——涉及 `runtime/`、凭证、模型路由或数据根破坏性操作则立即停。

**验收标准（合并后）：** ①六 Skill 盘点有文件事实依据，绑定契约 v1 可读、v2 校验生效；②三场景注入内容确实不同，随手记保持轻装，续答每轮注入；③`omnidesk-base` 与 `discipline` 两态 `--dump-config` 差异可复现，preset 校验脚本能发现漂移；④下拉只出现两个选项，已保存的 `yvan-work` 原样保留且有显式提示；⑤TurnRail 卡片仅补边与 aria，悬停/聚焦均可见；⑥顶栏搜索命中、跳转、计数、无结果 0/0 均可用；⑦tsc 0、npm build 成功、cargo lib 全绿、pytest 全绿、`sync-skills.sh --check` 通过、`runtime/` 零差异；⑧evidence 目录含两态 facts、三场景提示词事实与 S6/S7/S8 截图。

**风险与限制（合并后）：** ①preset 覆盖属对上游结构的复制，升级 DSH 后可能漂移——由校验脚本兜住，漂移时停下报告而非静默放宽；②skill 全局发现仍存在：本轮以绑定注入为主，`discipline` 下模型仍可能自行发现未绑定的 Skill，不承诺强制隔离；③规则包可能仍生效：默认模式只是「不挂业务插件、不注入 Skill 正文、不用场景绑定」，装过规则包时红线与掩码照常；④会话搜索在数千轮时可能卡顿，本轮标注已知限制；⑤S8 命中定位以段落/轮次文本为主，不做 HTML 解析，Markdown 富文本内的跨节点命中可能漏配，需在 STATUS 如实记录。
### OD-ARCH-080：真实模型端到端实测准备 + 未提交改动分批推送（已执行并推送，2026-10-25，ARCHITECT 复核通过）

**执行结果：** 127 项改动按 4 批提交并推送（`4f8c578` 文档与状态 → `a4327ee` Rust 宿主 → `bd6afd8` 前端 → `328a498` 引擎与验证证据），`origin/main` = `328a498`，工作区干净。提交前把本地真实制度抽取目录 `.kb_extract/` 加入忽略并移出暂存，凭证模式扫描零命中。推送前全量回归：tsc 0、npm build 成功、cargo 257 passed、pytest 75 passed、`runtime/` 零差异。详见 STATUS.md 同编号条目。

**背景（承接 075–079 复核结论）：** ①首页对话区、附件、分支、索引条已对齐 DSH 原生，界面层打磨暂告段落；②工作区累计 127 项未提交改动，持续累积单点丢失风险；③用户下一步核心诉求是真实模型端到端实测（此前所有验证均为合成端点）。

**目标：** ①分批整理并推送未提交改动到私有仓库（备份目标，最高优先）；②为用户真实模型实测做好入口准备（不接触凭证、不修改路由）。

**影响范围：** Git 提交与推送操作、docs/user-testing-checklist.md 更新；不改 runtime/、不新增功能代码、不碰凭证与模型路由。

**步骤：**
1. **S1 分批提交推送：** 按逻辑分 3–4 批（①文档与状态文件；②Rust 宿主改动；③前端 UI 改动；④引擎与测试证据），每批提交前跑敏感信息扫描（sk-/Bearer/api_key/凭证关键词）+ 文件清单核对，确认无真实材料、无凭证后提交。推送前跑一次全量回归（cargo --lib + pytest + tsc + npm build）确认基线健康。推送目标为既有私有仓库远端。
2. **S2 实测清单更新：** 在 docs/user-testing-checklist.md 补入 078/079 新能力的实测步骤（附件选择框、答句五项操作条、轮次索引条、分支到新对话），并标注「真实模型端到端」为本次实测重点。
3. **S3 状态文件同步：** STATUS.md 记录提交批次与哈希；PLANS.md 标注计划状态。

**验收标准：** ①origin/main 与本地同步，无遗漏文件；②敏感扫描零命中；③全量回归全绿；④runtime/ 零差异；⑤实测清单含 078/079 新能力。

**风险与限制：** ①推送是不可逆操作，须用户明确批准后才执行；②若扫描发现真实材料或凭证，立即停止该批提交并报告，不自行处理；③真实模型实测本身仍需用户自行配置端点与凭证（065 已开放入口），BUILDER 不接触。

### OD-ARCH-079：四项实测修复（已执行，2026-10-25，已复核通过）

**背景（用户实测反馈 078 收尾）：** ①点「+」选仓库内文件（如 `AGENTS.md`）报「拒绝读取：该路径位于被禁止的目录（DSH / agent / 仓库内）」；②问句下方仍有「撤回 / 修改」两个不可用入口；③「分支到新对话」当时是本地重放的新线程，与 DSH 原生 `session/fork` 语义不一致；④缺少 DSH 原生对话右侧的轮次索引条。

**目标：** 附件读取边界只拦真正的运行数据根；问句操作条只保留可用项；分支走引擎原生 fork；补齐原生轮次索引条。

**影响范围：** `desktop/src-tauri/src/task_shell.rs`、`desktop/src/shell/{HomeChat.tsx,HomeCanvas.tsx}`、新增 `desktop/src/shell/TurnRail.tsx`、`desktop/src/theme.css`、`desktop/src/{api.ts,types.ts}`、`desktop/src-tauri/src/{engine_bridge.rs,lib.rs}`、`engine/omnidesk_engine/{contract.py,task_core.py,web_api_adapter.py,serve.py}`、`tests/verification/desktop/verify_home_polish_078.py`、`tests/verification/engine-interface/` 用例。不改 `runtime/`、不碰 `~/.dsh`、`~/.agents`、凭证、模型路由、真实数据。

**步骤与验收：**
1. **S1 附件边界拆分：** 新增仅供附件使用的禁止片段表（保留 `.dsh` / `.agents` / `.codex` / DSH Desktop 数据 / `deepseek-harness` / `omniharness`，移除 `myworkstation`）；产物落盘与 `read_material` 仍拒绝仓库内路径。验收：选仓库内 `AGENTS.md` 出现附件 chip 且无红色拒绝提示；产物路径仍被拒绝（单测各一条）。
2. **S2 问句操作条：** 问句下方只留「复制」，答句保留复制 / 朗读 / 重新生成 / 分支 / 耗时。验收：窗口截图里问句只有一项目可用操作。
3. **S3 原生分支：** 引擎侧转发 `session/fork {sessionId, atSeq}`，切点取实际 `turn/end` 序号记录在任务结果里；ACP 通道返回 `unsupported`；无切点时按钮禁用且不新建线程，失败如实提示。验收：`W11.json` 真实 fork（父结果不变、子会话继承前缀）、`W12.json` 无效切点如实报错；窗口负向证据不新建线程。
4. **S4 轮次索引条：** 新增 `TurnRail`，节距 / 留白 / 刻度比例 / 悬停预览沿用 DSH 原生 `TurnNavigator` 口径，`<lg` 隐藏、≥2 轮才出现、点击回到该轮。验收：刻度数等于问句数，点击首/末轮视图真的变化。
5. **S5 回归：** `tsc`、`npm run build`、`cargo test --offline --lib`、`python3 -m pytest tests -q`、真实窗口验收脚本全绿；`runtime/` 零差异。

**风险与限制：** ①附件放开到仓库内不等于放开任意敏感目录，运行数据根仍拒绝；②引擎侧 fork 的正向证据来自接口级真实调用，窗口内未跑通完整正向分支；③索引条悬停预览卡未单独截图验证；④本轮未用真实模型，全部为合成素材。

### OD-ARCH-078：附件入口放开 + 首页顶栏与对话细节打磨（已执行并复核通过，2026-10-25）

**背景（用户实测反馈 075–077）：** ①附件当前限数据根内 + 文本白名单，用户要求「+」号弹文件选择框、放开常见类型、允许桌面等任意外部路径（用户是唯一用户+开发者，多模态模型识别不了会自报）；②顶栏与对话区 8 处 UI 细节需打磨。

**目标：** 附件入口易用且不再误拒用户文件；顶栏信息收敛；对话区对齐 DSH 原生操作习惯。

**影响范围：** desktop/src/{HomeChat.tsx, attachmentPrompt.ts, shell/TopNav.tsx, HomeCanvas.tsx}、desktop/src-tauri/task_shell.rs（读取路径校验段）；不新增通信层、不改 runtime/、不碰凭证与模型路由。

**步骤：**
1. **S1 附件入口与类型放开：** 「+」号接宿主文件选择对话框（多选），保留拖拽；文件类型白名单改为开放校验（txt/md/word/wps/pdf/excel/et/csv/图片/视频等常见扩展放行），不再预判「不支持」；读取仍走宿主命令，正文原样进提示词+红线扫描；二进制格式读取失败时如实报错并提示换文本格式，不假装摘要；越界路径放开（允许数据根外，如桌面），但**红线扫描不豁免**（扫描对象仍含附件正文）。单轮数量（5）与总字数上限（120k）保留。
2. **S2 顶栏收敛：** ①Profile 徽标改窄（只显示「专业」二字，具体 profile 名移入下拉选项）；②删除顶栏人物头像占位块；③「LOCAL IPC ACTIVE」改「IPC ON/OFF」紧凑胶囊（按实际状态变色），腾出宽度给搜索栏；④问候语「晚上好·欢迎回到 OmniDesk」首页常态显示；⑤删机器人图片。
3. **S3 对话区对齐 DSH 原生：** ①历史对话去渐隐；②问句下加 tips：复制/撤回/修改；③答句下加 tips：复制/朗读/重新生成/分支到新对话/任务累计完成时间；④问答正文字号 13px→15px；撤回/修改/重新生成/分支均走既有引擎会话能力（撤回=删本地并同步引擎删除该轮，若引擎不支持则明确说明，不假装）。
4. **S4 验证：** tsc 0、npm build 通过、cargo 定向（task_shell 相关测试）；真实窗口截图（顶栏、问答 tips、附件 chip）；runtime/ 零差异。不提交不推送。

**验收标准：** ①拖桌面任意 txt/pdf/图片→不误拒，正文或诚实报错进对话；②顶栏仅保留：品牌/专业徽标/IPC 胶囊/搜索/日期/设置/通知/窗口控制；③问答 tips 与 DSH 原生功能对齐；④字号≥15px；⑤runtime/ 零差异。

**风险：** ①多模态文件（图片/视频）当前引擎是文本注入路线，无法视觉理解——本轮诚实处理（文件名+提示模型「请用户改用文本描述」），真多模态注入另评估引擎协议再立项；②越界路径读取涉及安全边界，已明确红线扫描覆盖；③「撤回」「分支」若引擎能力不支持，按钮置灰+说明，不硬编。

### OD-ARCH-077：功能补齐批次（已执行并复核通过，2026-10-24；偏差：附件正文由前端拼进提示词而非新增协议字段，数据边界裁定见 REVIEW.md）

**背景（用户指令）：** 对照 DSH 官方桌面把除插件与 Skill 外的功能补齐。经只读核查，当前缺口：①对话无文件附件/拖拽/@引用（官方桌面有，材料写作场景刚需）；②语音输入是占位按钮；③无托盘与全局快捷键。同时用户定调稳定性政策：**插件清单冻结**（出厂即定，增量只能走版本升级流程，不再运行时随意新增）；**Skill 平时可修改、不能随意增加**（新增走受控流程）。

**政策落点（先于功能）：**
1. **插件冻结：** 代码层不再提供「新增插件」入口；现有能力管理页只保留查看/启停/更新提示。更新流程 = 升级整个 OmniDesk 版本（版本内插件清单固定）。docs/decisions.md 记为 D-030。
2. **Skill 受控：** Skill 文件平时可编辑（改内容即时生效，重启或重新加载即用），但**新增 Skill 文件**在界面上不提供入口；新增通过「版本升级 + 更新说明」进行。能力管理页对新增动作做提示文案（说明该政策），不做强制文件锁（用户手改文件系统仍自由，但产品语义是受控变更）。

**功能步骤：**
1. **S1 文件引用与拖拽（优先级最高）：** 对话输入框支持：①拖拽本地文件到输入区（多文件）；②输入 @ 弹出最近文件选择器（限当前场景成果目录 + 知识库目录，经宿主路径校验）；③选中文件以附件 chip 显示，可删除；④发送时文件路径列表随消息体传给引擎，引擎侧把文件内容读出注入该轮 prompt（文本类直接读，二进制类给摘要占位）。路径校验复用宿主现有 `assert_local_root_allowed` 语义（只允许数据根内路径），越界拒绝并在 chip 上标红。不新增协议字段——复用现有消息结构，附件信息放在消息元数据里。
2. **S2 语音输入（按当前实现诚实处理）：** 当前按钮是占位（disabled）。本轮不做真实语音识别（涉及模型端点与授权），改为：按钮保留但点击弹出说明「语音输入需要配置语音模型端点，当前版本未开放」，不再显示禁用灰色样式误导用户以为即将可用。语音能力列入打包阶段评估项。
3. **S3 托盘 + 全局快捷键（可选，用户未强烈要求）：** 最小实现——系统托盘图标（左键显示/隐藏主窗口，右键菜单：打开主窗口/退出）；全局快捷键暂不做（涉及系统级权限，收益存疑）。若托盘实现代价超预期（依赖冲突/平台差异），标记「推迟到打包阶段」即完成本步，不硬做。
4. **S4 验证与证据：** cargo 定向 + tsc 0 + build 通过；拖拽/引用真实窗口截图；附件注入 prompt 的引擎侧单测；runtime/ 零差异。

**影响范围：** desktop/src/HomeChat.tsx（输入区）、desktop/src-tauri（新增 2-3 个命令：读场景目录文件列表、校验并读取附件内容）、engine/omnidesk_engine（消息体附件注入）、tauri.conf.json（托盘配置，若做 S3）。不改 ACP 协议、不改 runtime/、不碰凭证。

**验收标准：** ①拖拽 2 个文本文件到输入框→chip 出现→发送→引擎收到文件内容且回复引用了内容；②@ 选择器只列数据根内文件，越界路径被拒；③语音按钮说明文案准确；④插件冻结/Skill 受控文案在能力管理页可见；⑤cargo/tsc/build 全绿；⑥runtime/ 零差异。

**风险：** ①附件注入位置如果引擎侧结构不支持，可能需要在 engine 消息组装层加字段（最小侵入，不动协议帧格式）；②托盘依赖（tauri-plugin-tray）与当前版本兼容性——失败则推迟不硬做；③Skill 受控目前靠文案说明而非代码强制，用户手改文件系统无拦截——这是预期行为（用户就是开发者本人）。

### OD-ARCH-075：设置页通道能力说明（已执行并复核通过，2026-10-24；偏差：说明落在引擎诊断页，因界面本无通道选择器，通道由环境变量决定）

**背景：** 074 复核通过后，Luna 审计的 P1 项——通道能力差异说明——仍未落地。用户在 Web API 通道下会以为 ACP 通道同样支持界面审批，实际不支持且取消语义不完整，存在误操作风险。

**目标：** 在设置页现有通道选择位置加简短说明，与实际实现一致，不改代码行为。

**影响范围：** 仅设置页前端组件（SettingsPanel 或对应文件）+ 一处文案。不改引擎、不改协议、不改默认通道。

**步骤：**
1. 在通道选择控件下方加两行说明：Web API —— 支持界面审批与引擎取消；ACP —— 当前不支持界面化审批，取消请求不代表底层已确认终止。
2. 纯文案改动；若有 i18n 文件则同步补 key。
3. 验证：tsc 0、npm build 通过；截图一张（不需要全量回归，纯静态文案）。

**验收：** 文案与实际行为一致；不新增协议、不改默认通道；改动最小（预计 1 文件）。

**预计规模：** 单文件小任务，可与下一批次合并执行。

### OD-ARCH-076：单实例运行 + 移除 DSH 原生界面入口（已执行并复核通过，2026-10-24）

**背景（用户实测反馈）：** ①用户要求本应用同一时间只允许开一个窗口（当前无任何单实例限制，可开多窗并发触发引擎互斥报错）；②模型设置已全部收回 OmniDesk 原生设置页（D-027），设置页里「DSH 原生界面（诊断用）」折叠区仍会起一个完整 DSH Web 应用窗口（`task_open_dsh_settings` → `open_settings_window`），与「打开后看不到 DSH UI」的产品口径冲突，应移除。

**目标：** 第二次启动 OmniDesk 时自动聚焦已有窗口并退出新进程；移除 DSH 原生界面入口与相关死代码；不影响凭据 RPC（`credentials_rpc` 仍无头复用 SettingsService，不开窗口）。

**影响范围：** desktop/src-tauri/{Cargo.toml,lib.rs}、desktop/src/SettingsPanel.tsx、desktop/src/api.ts；`dsh_settings.rs` 保留（凭据 RPC 依赖），仅删窗口路径；docs/decisions.md 补记。

**步骤：**
1. **S1 单实例：** 引入 tauri-plugin-single-instance（需联网拉一个 crate；先核对与当前 Tauri 版本兼容，不兼容则用启动时检测已有主窗口的自研轻方案）；回调聚焦主窗口并退出新实例。验证：连续两次启动 → 第二个进程退出、已有窗口聚焦。
2. **S2 移除 DSH 原生界面入口：** 删 SettingsPanel「DSH 原生界面（诊断用）」折叠区与 openDshSettings 回调；删 lib.rs 的 task_open_dsh_settings 命令与 open_settings_window 函数及注册；删 api.ts openDshSettings 与相关类型。SettingsService 与 dsh-settings.json 记录保留（credentials_rpc 无头使用）。引擎/协议/runtime 不动。
3. **S3 验证与证据：** cargo 定向单测 + tsc 0 + npm build 通过；真实启动两次验证单实例聚焦；截图一张设置页（无 DSH 入口）。不提交不推送。

**验收：** 单实例生效（聚焦已有窗口）；设置页无任何打开 DSH 原生界面的入口；凭据保存/探测功能不受影响；runtime/ 零差异。

**风险：** ①插件兼容性失败则回退自研轻方案（检测已有主窗口即退出）；②误删 credentials_rpc 依赖的代码——S2 只删窗口路径，cargo 编译错误即为回归信号。

### OD-ARCH-047：夜间连续任务——045 补复核 + 备份提交 + 真实模型实测准备（待批准→派工，2026-10-21）

> **执行方式：** 三步按序，每步独立验证登记；失败记录后跳下一步。不碰凭证、不接真实端点、不改 runtime/。
>
> **执行结果（2026-10-21，用户已批准含推送）：** 三步全部完成。①045 补复核结论已写入 `REVIEW.md`；②84 个未提交路径按“可编译单元”分 3 批提交并推送 `origin/main`（`ba77c2d` 代码 / `a3bf0ad` 证据 / `bb09af1` 文档）——未按 045/046 逐块拆代码，因两轮改同一批函数，逐块拆会留下无法编译的中间提交；③新增 `docs/user-testing-checklist.md`，只读核实引擎当前实际路由为 `agnescn / agnes-3.0-flash`（与 `yvan-work` Profile 里的 `deepseek-v4.1-flash` 不同，因 OmniDesk 覆盖文件优先），未改动任何路由/凭证。回归：tsc 0、build 通过、cargo 165 passed。未打标签。

1. **045 补复核（ARCHITECT 职责，BUILDER 仅准备材料）：** 按 045 计划六步清单逐项核对（线程落数据根/成果目录树开关/首提卡顿修复/AI 复盘打通/DSH web 验证报告/整批回归），已覆盖项标证据、缺项列出；不重复已由 046 复核覆盖的部分。
2. **分批备份提交（需用户在此明确批准后执行）：** 25 项未提交改动按主题分 3-4 批提交——①045 线程持久化+成果目录树；②046 四项机制（pending_writes/scheduler/设置页/公告栏）；③借鉴研究与计划文档；④证据目录。每批提交前跑红线扫描与文件清单核对；提交后推送到 origin/main（沿用 OD-ARCH-029 一次性授权模式，本轮为第二次明确授权）。
3. **真实模型实测准备（只读准备）：** 检查 DSH 侧端点配置现状（不读凭证值）；整理"用户实测清单"——开权限→设端点→跑周报起草→看公告栏定时复盘→确认写卡流程，作为用户上手指南写入 docs/user-testing-checklist.md。

**验收：** 提交历史清晰分批、远端同步；清单文档可直接照做；045 复核结论写入 REVIEW.md。



### OD-ARCH-048：换接缝（D-024 路径 B）——对话通道切到 DSH 本地 Web API，继承逐字流式与思考链（已执行，2026-10-21，待 ARCHITECT 复核）

> **执行结果（BUILDER，2026-10-21）：四项目标全部实现，未提交、未推送、未打标签。** 引擎新增 `web_api_adapter.py`（与 ACP 并存，`dsh.channel` 切换，默认 web-api）+ 抽出 `task_core.py`；宿主加 `channel`/`engine_list_models`；前端 `ModelPicker` 走真实目录、`HomeChat` 消费真实增量。实测：W01–W10 一次性全 pass（逐字流式 0.381/0.615/0.866s 三块到达、取消 0.032s 落定、失败不伪装成功、重启恢复、未知会话即时拒绝、模型目录归一化）；ACP E01–E20 全 pass 未回归；`test_web_api_unit` 25 passed；Rust 165 passed；tsc 0、构建通过、分类器自检 24/24；`runtime/` 无差异。
>
> **四处计划偏差**（详见 `STATUS.md` 与证据 README）：①`turn/end` 实际 `completed`，`stop` 在 finish 帧；②路由分类器只认 `acp`，web Profile 被判 `route_missing`，故给脚本与 `route_classifier.rs` 加 `channel`（计划外最小修复）；③失败态实为 `error`，原映射会落 unknown，已修并补 `max-tokens`/`interrupted`/`blocked`；④`session/not-found` 是顶层错误帧，原实现拖到超时，已修并补单测。另修测试夹具 `admission_classification.rs`（同时声明两条路由、plugin name 需匹配）。
>
> **未验证：** 思考块流式（合成端点不产）、personaPrefix 是否真正送达模型（dump-config 生效但请求体探针未命中）、真实模型端点流式、真实窗口人工抽验、`wss://`（仅 `ws://`）。

> **卡死修复（同计划内追加，2026-10-21）：** 用户报「启动后整窗无响应、场景切不动、只能强制退出」。根因是 Tauri 2 同步命令在 UI 主线程执行，49 个同步命令持全局宿主锁等引擎冷启动，堵住重绘。已把它们的属性改为 `#[tauri::command(async)]`（函数体、签名、锁语义不变），首屏读数据根加 4 秒上界（超时先用本地缓存，后台补上），并新增回归测试 `every_tauri_command_runs_off_the_ui_thread` 钉住这条不变量。实测（Xvfb 隔离显示 + 合成 40s 慢启动、修复后重新构建的二进制）：五个场景来回切换在 0.22–0.84s 内完成且画面正确，等待期间窗口全程可交互，40s 后引擎正常接通。回归 166 passed / 0 failed、tsc 0、构建通过、`runtime/` 无差异。**同轮更正：** 早先记录的 0.17–0.43s 是旧二进制 + 只截 Dock 区域的错误判据所得，已在证据 README 标注并替换。**未验证：用户真实机器同一故障状态未复现、真实 DSH 冷启动耗时未测。** 证据见 `STATUS.md` 同章节与 `tests/verification/desktop/evidence/od-arch-048/README.md` 第 7 节。

- **背景：** 045 第 5 步探针已实测 DSH 本地 Web API 具备：token 级流式（WebSocket /api/remote.mux 的 assistant-stream 帧：block-start/text-delta/finish/usage）、同 sessionId 连续两轮、取消（19ms 落定）、重启后会话持久、模型目录。现 UI 走 ACP 窄接口，整段到达、只能本地模拟打字机。用户批准走 D-024 路径 B。
- **目标：**
  1. **引擎侧新增 Web API 适配通道**（与 ACP 通道并存，配置切换）：会话创建/续接/取消/历史读回/模型目录直连 DSH 本地 HTTP+WebSocket API；参数格式按探针记录（args.request 包装、throughSeq 需 follow 快照 cursor）。
  2. **逐字流式端到端**：assistant-stream 的 text-delta 帧经引擎→宿主→前端实时推进对话区；打字机模拟退役（保留为降级）。思考块流式若模型端点支持则同样透传（合成端点不产思考内容，此项标注未验证即可）。
  3. **模型目录直读**：模型选择器改用 session/modelCatalog 真实数据（provider/分组/reasoning efforts），替换本地手搓列表。
  4. **ACP 通道保留可切**：配置项选择通道（默认 web-api；异常时一键切回 acp），确保升级风险可控。
- **影响范围：** engine/omnidesk_engine 新增 web_api 适配模块与通道切换；宿主 engine_bridge 增加对应转发；前端对话区流式渲染改造+模型选择器数据源替换。**不改 runtime/、不改 DSH 源码、不动凭证与路由文件；DSH 本地 API 属内部接口，已在 decisions.md 记录"锁定 runtime 版本"约束。**
- **验收标准：**
  - 合成端点（带 chunk 间延迟）下真实窗口逐字流式：文本分多次到达而非一次性；连续两轮同 sessionId；取消 1 秒内界面落定。
  - 进程重启后恢复会话并继续对话（resume 路径走 web-api）。
  - 模型选择器显示 modelCatalog 真实内容（含 reasoning efforts）。
  - 切回 acp 通道后既有行为不回归（全量测试+真实窗口抽验）。
  - Rust/tsc/构建全绿；新增适配层单测覆盖帧解析与参数包装；runtime/ 无差异。
- **风险与停止条件：** DSH 本地 API 为内部接口——若发现接口行为与探针记录不符（版本漂移），停下报告不猜；若流式帧格式需要 runtime 改动才能适配，停下报告；凭证注入方式沿用探针的隔离环境变量方案，不新增凭证读取面。
- **执行方式：** 建议分两段连续执行——第一段引擎+宿主通道打通（含单测），第二段前端流式渲染+模型目录（含真实窗口证据）；ARCHITECT 中途不介入，完成后一次性复核。

### OD-ARCH-046：借鉴四项落地——写确认制 / 自动降级 / 定时·心跳 / Agent 式设置页（已执行，2026-10-21，待 ARCHITECT 复核）

> **执行结果（BUILDER，2026-10-21）：四项全部实现，未提交、未推送、未打标签。** 回归：tsc 0 错误、`npm run build` 通过、`cargo test` 165 passed / 0 failed（lib 146 + 2 + 13 + 3 + 1）、`cargo build` 通过。真实窗口实测：设置页五卡、once 定时触发、无确认不落盘、确认后落 `weekly-report/` 子目录、权限关闭时定时不执行、模型断开时降级、起草无模板假输出。逐项证据见 `tests/verification/desktop/evidence/od-arch-046/README.md`；`cargo fmt` 未运行（本机无 rustfmt）；"过期记录界面提示"仅有单测证据。

- **来源：** docs/research-borrow-cherry-lingxi.md（灵犀工作台 + Cherry Studio 对标）；用户圈定四项。
- **范围与实现要点：**
  1. **写确认制（宿主强制）**：新增统一的"待执行写操作"机制——凡是模型/定时任务发起的写动作（保存成果、更新任务状态、写入建议文件），宿主生成待确认记录（含动作摘要与影响范围），前端渲染确认卡片，用户点击后宿主校验该确认记录存在且未过期才执行；无确认记录的写请求一律拒绝（代码级，不靠提示词）。人工在对话中点"保存"的既有路径不变。
  2. **自动降级（分级诚实）**：提醒/巡检/AI 复盘类功能在模型不可用、超时或权限关闭时自动退回本地确定性结果（规则提醒本来就是本地计算），公告栏明示"AI 暂不可用（原因），已用本地规则结果"。**内容生成类（周报/文案起草）不降级为模板**——失败就明确失败，不用模板冒充模型产出（数据诚实红线）。
  3. **定时/心跳**：宿主内置应用运行期调度器（不做开机后台守护）。两类：**定时任务**（cron 表达式或预设，如每周一 09:00 生成周报 AI 复盘建议，须权限开+确认制放行）；**心跳**（可配间隔，默认 2 小时，扫描拖延/超期/日程，刷新公告栏规则提醒，不调模型）。配置存 ~/OmniDesk 数据根设置文件；错过触发下次打开补跑一次。
  4. **Agent 式设置页**：设置页重构为卡片组——模型与端点（现有跳转 DSH+刷新保留）、模型权限三档、定时与提醒（新）、成果目录与下级开关（已有能力收拢）、数据与日志（保留期限展示）。每卡显示当前状态摘要与主操作；不改权限默认关闭语义。
- **影响范围：** desktop/src 前端（设置页、公告栏、确认卡片组件）、Rust 宿主新增 3-4 个命令（待确认写操作 CRUD+校验、调度器配置读写、触发事件）；engine 侧不动；不动 runtime/、协议、凭证、路由。UI 实现前按派工模板先搜同类产品设置页参考。
- **验收标准：**
  - 定时/心跳：合成端点下，把定时任务触发时间改到 1 分钟后实测触发；权限关闭时定时 AI 任务明确不执行并在公告栏说明原因；心跳扫描产出与手动"逾期扫描"结果一致。
  - 写确认制：模拟一次定时任务尝试写建议文件——无确认时不产生文件；点确认后落盘到数据根该场景目录；确认记录过期（如 24h）自动失效。
  - 降级：断开引擎/关权限两种情况下，公告栏出现规则提醒+可见降级说明；周报起草请求明确报错，无模板假输出。
  - 设置页：五张卡片状态真实反映当前配置；改动即时生效；截图佐证。
  - 回归：Rust/tsc/构建全绿；不提交不推送。
- **风险与停止条件：** 调度器不得自起常驻进程或系统级定时（越界即停）；确认机制若发现需要改 engine 协议，停下报告。

### OD-ARCH-045：功能完善夜间批量任务（已执行，2026-10-21，待 ARCHITECT 复核）

> **执行方式：** 六步按序执行，每步独立验证并登记；某一步失败记录原因后跳到下一步，不反复重试。全程不提交、不推送（等用户晨间复核）；不碰凭证、不接真实模型端点（一律合成端点验证）；不改 runtime/。
>
> **执行结果（BUILDER，2026-10-21）：六步全部执行完毕，未提交、未推送。** 第 1/2/3/5/6 步按验收通过；第 4 步功能通过但有 1 处计划偏差（复盘成果落在数据根顶层，未按场景分目录）。回归 `cargo test --offline` 154/154、tsc 0、构建通过；第 5 步思考块与工具事件未验证；首页对话尚未接入线程持久化。逐项证据与缺口见 `STATUS.md` 同名小节与 `tests/verification/desktop/evidence/od-arch-045/README.md`。

1. **线程持久化到数据根（收 033 遗留①）：** 新增宿主存储命令，把工作台线程/会话记录落到 ~/OmniDesk 数据根、按项目分目录；前端本地存储降为缓存，启动时从数据根恢复。旧数据只读兼容，不删除不覆盖。验收：新建线程→重启应用→从数据根恢复；git status 不出现任何线程记录文件。
2. **成果目录树与下级开关（收 033 遗留②）：** 宿主新增成果目录列举命令（带目录层级）；每个场景设置"是否含下级目录"开关（默认不含）；三个场景页成果右栏按开关显示并可打开文件。验收：开关切换后右栏树状/平铺正确变化。
3. **首次提交卡顿修复（收 041 遗留）：** engine_submit_task 改为异步启动引擎（UI 立即给"正在启动"状态，不冻结）；权限校验保持同步语义不变。验收：冷启动后首次提交 UI 不冻结，可取消；既有测试全绿。
4. **AI 复盘建议打通（默认关闭，权限开时可用）：** 公告栏"AI 复盘"按钮接通引擎（走现有权限三档开关）；关闭时按钮明确置灰并说明原因；用合成端点验证一次真实调用与建议文件落盘（存仓库外数据根该场景目录）。验收：开/关两种状态行为正确；建议文件在数据根不在仓库。
5. **D-024 第 2 步：DSH 自带 Web 应用本地 API 验证（只读实验，零产品代码）：** 在隔离环境起 dsh-host-webserver，用合成端点经本地 API 发消息，验证 token 级流式、思考块、模型目录、会话续接是否真实存在，形成证据报告。验收：报告含逐项"存在/不存在"结论与复现命令；不改任何产品代码与 runtime/。
6. **整批回归与证据包：** Rust/tsc/构建全绿；真实窗口截图（首页+三场景页+成果右栏开关两态）；更新 STATUS.md 与 PLANS.md 各步登记。验收：证据目录齐全，晨间可独立复核。

**停止条件：** 任一步需要改 runtime/、协议、凭证或真实端点，停下登记；需要 git 操作时停下等用户。
### OD-ARCH-035：接入免费 UI 设计参考 MCP（screenshots-design-inspiration）（用户已批准；BUILDER 已执行；待 ARCHITECT 复核，2026-10-20）

> **执行口径修正（2026-10-20，用户当场纠正）：** 目标环境是 **Codex**，不是 OpenCode。
> 实际配置写在 `~/.codex/config.toml`（`[mcp_servers.screenshots_design_inspiration]`），
> OpenCode 配置未作任何改动。下文步骤 1、2 中「OpenCode」一词按此口径替换为「Codex」。
> 另因本机走代理而 Node 默认不读代理环境变量，该 MCP 专属 env 增加了 `NODE_USE_ENV_PROXY=1`。
> 证据与未验证项见 `tests/verification/desktop/evidence/od-arch-035/README.md`。

- **目标：** 为 BUILDER 接入开源设计参考 MCP，使后续 UI 任务实现前能先搜真实产品界面作参考。不涉及产品代码改动。
- **步骤：**
  1. 在 Codex 的 MCP 配置（`~/.codex/config.toml`）中添加：command = "npx"，args = ["--yes", "screenshots-design-inspiration-mcp@latest"]（Node.js 已具备即可，无需克隆仓库）。
  2. 重启 Codex 后验证：调用 search_inspiration 搜 "AI chat workspace dashboard" 类关键词，确认能返回截图结果。
  3. 在任务指令模板中加入固定条款：UI 实现前先用该 MCP 搜 2-3 个同类产品布局（关键词示例：chat workspace / sidebar dock / task dashboard），给出 2 句以内的借鉴要点，再动工。
- **验收：** Codex 会话中能成功调用该 MCP 并返回截图；未成功时报告失败原因（网络/Node 版本），不降级硬编。
- **风险：** npx 首次运行需联网下载包；该 MCP 数据为静态截图库（410+ 产品），覆盖有限，效果不足再评估 Mobbin 付费版。
### OD-ARCH-042：顶部卡片默认显示 / 静止恢复（**已执行，2026-10-21，ARCHITECT**）

- **状态：已执行。** 打开即显示卡片；指针进对话区卡片隐藏（与对话互斥）；
  鼠标静止满 5 秒且输入框无焦点 → 卡片回来；输入框有焦点 → 不恢复。
- **验证：** 四态真实窗口截图（`evidence/od-arch-042/shots/`）；tsc 0、构建通过；未改 Rust。

### OD-ARCH-041：场景切换卡顿（**已执行，2026-10-21，ARCHITECT**）

- **状态：已执行。** 根因＝`engine_admission_status` / `task_list_model_routes` 在主线程同步 spawn node，
  而对话区每次场景切换重新挂载 ⇒ 每次切换重拉 ⇒ 整页卡住。处置＝宿主侧改 async（用 AppHandle 取状态）
  + 前端缓存（模型候选缓存到失效、端点事实 2 秒短缓存）。
- **实测：** 首页→周报 6.07s→0.62s；周报→首页 3.51s→0.48s；首页→项目跟踪 6.03s→0.46s；
  项目跟踪→首页 3.35s→0.73s。回归 Rust 143/143、tsc 0、构建通过。
- **未解决（另立计划）：** 首次提交仍会卡一下（`engine_submit_task` 同步且要启动引擎进程）。

### OD-ARCH-034：真多轮接线——去掉上文重放（**已执行，2026-10-20，ARCHITECT**）

- **状态：已执行（用户批准后由 ARCHITECT 执行；提交见 REVIEW/STATUS 同名小节）。**
  同一线程复用同一引擎会话（`new` / `continued` / `resumed` / `degraded` 四种会话取得方式，
  回执与台账带 `session_mode`），前端不再重放上文；降级时下一轮自动补重放。
  **按事实修正了计划措辞：** DSH 的 `session/resume` 只接受已落盘可重开的会话，对活会话报
  `invalid_argument` ⇒ 进程内续发直接再发 `session/prompt`，进程重启后才 `session/resume`。
  验证：单元 27/27、真实引擎+合成端点 33/33（消息条数 3→5→7）、提示词 24/24、
  真实窗口台账 `new→continued` 且会话号相同、Rust 143/143、tsc 0、构建通过。
  证据：`tests/verification/desktop/evidence/od-arch-034/`。
  **本轮不含 D-024 第 2/3 步**（DSH web API 验证与路径 A/B 裁定）——按计划另行立项。

- **背景：** D-024 复盘已核实：ACP 本身支持 session/prompt 与 session/resume，我方适配层也已实现 recover_session，但 submit_task 每轮新建引擎会话、前端靠"上文重放"模拟多轮。这是自造绕路，不是引擎限制。接线后可获得真多轮与原版延迟。
- **目标：** 同一工作台线程复用同一引擎会话：线程首次对话用 session/new 并记住 engine_session_id；后续轮次经 session/resume + session/prompt 续发，不再重放全部上文。前端线程记录持有 engine_session_id 并持久化。
- **影响范围：** engine/omnidesk_engine/acp_adapter.py（submit_task 接受既有 engine_session_id）与 engine_bridge.rs 的传参打通；前端线程存储加字段。不新增通信层、不改协议、不动 runtime/、不动模型路由与凭证。
- **验收标准：**
  - 同一线程连续三轮对话，适配层日志显示第一轮 session/new、后两轮 session/resume，界面无需重放即有完整上下文（第二轮能引用第一轮内容）。
  - 线程重启应用后恢复，续发仍走 resume；会话被关闭（close）后 resume 仍可用（ACP 已注明支持）。
  - 旧线程（无 engine_session_id）首次续聊自动降级为新建会话并开始持有 id，不报错、不丢历史。
  - Rust 与 tsc 测试全绿；新增适配层单测覆盖 new/resume 分支。
- **风险与停止条件：** 若 resume 后上下文语义与预期不符（如引擎侧不带历史），停下报告而不是保留两套机制；本轮不做 D-024 第 2/3 步（DSH web API 验证与路径 A/B 裁定），那两步另行立项。
- **执行方式：** 本计划与「D-024 第 2 步验证」（只起 DSH 自带 web 应用验证流式/思考/模型目录是否存在，不改产品代码）可合并为一个任务交给 BUILDER 连续执行；第 2 步只产验证报告，不产产品代码。

### OD-ARCH-033：场景模块页设计 v3（已执行，2026-10-20，ARCHITECT）

- **状态：已执行（2026-10-20，ARCHITECT 执行；原计划派 super-worker，因该会话 agent 目录发现不到项目 agent 而改由主会话执行）。**
  已实现：骨架插槽化（首页与场景页共用 `SceneScaffold`，首页版式未变）、周报/文案双栏 + 项目跟进「进度/阶段成果」页签、
  顶部提醒/建议栏（规则提醒＝既有确定性汇总；AI 复盘建议＝可选模型调用、默认关闭、成本透明）、
  会话按入口打标与事后改挂。**按停止条件停下并报告两项：** ①线程记录真正落到 `~/OmniDesk/...` 并按项目分目录
  （现为浏览器本地存储，已确认不在仓库内）——需宿主新增读写能力；②成果目录「是否含下级目录」开关——
  宿主成果列举是平铺匹配、无目录树能力——需宿主新增目录列举命令。另「数据分析占位页」未新增（Dock 现有「任务工作台」
  即分析试点）。证据：`tests/verification/desktop/evidence/od-arch-033/`（脚本 + 6 张真实窗口截图 + 索引）；
  回归 Rust 144/144、tsc 0、构建通过、verify_conversation_prompt 12/12、verify_ai_draft 27/0。

### OD-ARCH-033：场景模块页设计 v3（设计确认稿，待用户批准，2026-10-20）

- **新增需求（用户提出）：** 周报/文案页在双栏之上再加一条横向「提醒/建议栏」（公告栏式），动态展示 AI 定时复盘成果给出的建议；周报侧重拖延/超时/进度提醒，文案侧重结合趋势热点的写作建议。
- **提醒/建议栏设计定稿：**
  1. **两类信息分开来源：** ①「规则提醒」= 纯本地确定性计算（对照截止时间/最近更新时间判断拖延与超期），不花模型 token、不依赖模型在线，项目跟进页的进度区同样复用；②「AI 复盘建议」= 定时任务触发模型，对本场景近期成果与对话做一次复盘，结论写成一份建议文件存入该场景数据目录，公告栏只展示最新一条并可点开历史。
  2. **定时方式（第一版克制）：** 应用运行期间由前端定时器触发（用户在设置里配置频率，如每天/每周、具体时间点错过则下次打开时补跑一次）；不做开机后台常驻进程。AI 复盘走现有引擎与权限开关，遵守模型权限三档设置，复盘输入仅限本场景成果与对话记录（位于仓库外数据根），不扩大范围。
  3. **文案「趋势热点」单独标记为受限能力：** 需要引入外部网络数据源（联网检索），涉及外部服务与合规边界，第一版不做。文案页公告栏第一版只做「AI 复盘写作建议」（基于用户已有成果与方法沉淀）；趋势热点等用户明确数据来源与授权后再议。
  4. **成本透明：** 每次 AI 复盘前公告栏显示预估说明（将调用一次模型），设置里可单独关闭 AI 复盘只留规则提醒；复盘产生的 token 用量计入现有用量显示。
- 

- **背景：** 用户细化场景模块页需求：周报/文案为双栏版式；项目跟进需进度与阶段成果；数据分析流程未定；成果目录只读本场景文件夹（是否含下级目录可设置）；对话历史要从 DSH 拆出、由 OmniDesk 自主持有；并确认复用首页骨架。另提出"场景对话能否自动对应到项目"的设计问题。
- **设计定稿（v2）：**
  1. **骨架复用（用户已同意）：** 首页与场景页共用同一布局组件，顶栏/Dock/下部对话区完全一致，仅上部两栏按场景注入。
  2. **周报页 / 文案页（同版式双栏）：** 左栏=该场景近期历史对话（点击加载到下方对话区）；右栏=该场景成果档案。右栏只读取本场景自己的成果文件夹；是否包含下级子目录做成设置项（默认不含），保证各场景文件管理边界清晰。
  3. **项目跟进页（三块内容）：** 同版式，右栏改为带两个页签的面板——「进度」（事项状态、逾期与阶段汇总，数据来自现有确定性汇总逻辑）与「阶段成果」（本场景成果文件，下级目录设置同上）。左栏仍为历史对话。
  4. **数据分析页（占位）：** Dock 保留入口，页面显示"该场景规划中"空态说明，不实现功能，等用户定流程。
  5. **对话历史持有（回应"从 DSH 拆出来"）：** 现行机制本就是工作台每轮新建引擎会话、把上文重放进下一轮提示，完整对话史已由工作台本地线程持有，不依赖 DSH 内部状态。本计划做两件事：①确认（必要时迁移）线程/会话记录存放位置在仓库外的 OmniDesk 数据根、按项目分目录，不在仓库内；②迁移只做只读兼容，不删除、不覆盖旧数据。DSH 内部会话继续视为可丢弃缓存。不动 runtime/、不新增通信层。
  6. **场景自动归类（回应"对话内容能否自动对应到项目"）：** 采用"入口即标签，事后可改"方案：从哪个场景页/Dock 入口新建的会话，创建时自动打上该场景标签并归入其项目；对话过程中不用模型猜归类（不可靠）。从普通对话入口新建时先选场景（默认普通，可选具体场景）；支持事后手动把会话改挂到其他场景/项目（会话列表处改标签）。标签随会话持久保存，作为左栏过滤与成果归类的唯一依据。模型自动识别归类列为后续可选增强，默认不做。
- **目标：**
  1. 抽取首页布局为可复用骨架，上部两栏成为按场景注入的插槽。
  2. 实现周报/文案页（双栏：历史对话+成果）、项目跟进页（历史对话+进度/成果页签）、数据分析占位页。
  3. 会话创建时按入口自动打场景标签并归入项目；支持事后手动改挂；标签持久化。
  4. 工作台线程/会话记录确认或迁移到仓库外数据根（按项目分目录）；旧数据不删不改。
  5. 场景成果目录读取范围（是否含下级目录）提供设置项，默认不含。
- **影响范围：** 以 desktop/src 前端为主（骨架插槽化+场景页+会话打标+设置项）；若线程记录当前存放在仓库内，迁移只调整宿主存储路径常量（属路径配置调整，不属新增通信层）；无 runtime/ 改动，不动模型路由与凭证。
- **验收标准：**
  - 三场景页与首页同版式；左栏点击历史对话可加载到下方对话区。
  - 周报/文案右栏只显示本场景文件夹成果并可打开；设置开关能切换是否含下级目录。
  - 项目跟进右栏可切换「进度/阶段成果」页签，进度数据与现有汇总一致。
  - 从周报页新建的会话自动带周报标签；普通入口新建时可选场景；事后可改挂标签并立即反映到左栏列表。
  - 会话记录文件位于仓库外数据根；git status 不出现任何会话记录文件。
  - 构建与既有测试全绿；真实窗口截图（首页+周报页+项目跟进页）佐证。
- **风险与停止条件：** 若会话打标需要新增宿主命令或改协议，停下报告；若线程记录迁移涉及既有数据格式，只做新增不重写，遇冲突停下报告。

### OD-ARCH-032：首页定版还原 + 项目/会话模块页（2026-10-20 已执行，待 ARCHITECT 正式复核）

- **状态：已执行完成（2026-10-20，ARCHITECT 执行，提交 `828c5a6`）。** 三件事：①**首页还原定版**——移除首页里的
  ProjectRail 与左右分栏，首页恢复 `HomeCanvas` 全宽对话版式（**没有**项目树、**没有**分栏，左侧仍只有 Dock 进各场景）；
  ②**项目/会话模块页**——项目分组、会话列表、进行中的工作台对话、新建对话与预设选择，整体迁入一个 Dock 模块页
  （改造既有 sessions 页，入口标签改为「项目与会话」；模块页内保留左栏结构，那是模块页自己的布局，不影响首页）；
  ③**构建修复**——`App.tsx` / `HomeChat.tsx` / `ProjectRail.tsx` 的 TS 错误已清零，未提交的 ProjectRail 修改已收敛。
  验收：真实窗口截图确认首页无项目树/无分栏、模块页可用；`npm run build` 0 错、`tsc` 0、Rust 144/144；
  诚实性标注（每轮新 DSH 会话 + 上文重放）保留在界面说明与 REVIEW.md 中。
- **背景：** OD-ARCH-031 Part A 通过（设置跳转 DSH 实测可用，含一个最小宿主命令 `task_open_dsh_settings` 的计划偏离
  ——已审查认可：无参数、固定白名单、不碰凭证与协议）。Part B 结构方向正确（项目-会话-对话流、三场景降为开场预设），
  但**摆放位置违反用户裁定**：项目/会话栏被放进了首页，形成用户不要的左右分栏。用户明确：首页就是全宽对话中枢，
  项目与会话集中到一个 Dock 模块页。
- **目标（三件事，一批完成）：**
  1. **首页还原定版**：移除首页中的 ProjectRail 与左右分栏，首页恢复 HomeCanvas 全宽对话版式；预设芯片（周报/文案/项目跟踪/普通对话）可留在首页对话区，项目树不进首页。
  2. **项目/会话模块页**：项目分组、会话列表、进行中的工作台对话、新建对话与预设选择，整体迁入一个 Dock 模块页（优先改造现有 sessions 页）。模块页内可保留左栏结构（那是模块页自己的布局，不影响首页）。
  3. **修复构建**：修掉 App.tsx / HomeChat.tsx / ProjectRail.tsx 的 4 个 TS 错误；未提交的 ProjectRail 修改一并完成或收敛。
- **影响范围：** desktop/src 前端（App/Workspace/ProjectRail/HomeChat/sessions 页），无宿主改动、无协议改动、无 runtime/ 改动。
- **验收标准：** 真实窗口截图确认首页无项目树、无分栏、版式与用户定版一致；Dock 模块页内项目/会话/新建对话可用；npm build 0 错误、tsc 0、Rust 测试全绿；诚实性标注（每轮新 DSH 会话 + 上文重放）保留在界面说明中。
- **风险与停止条件：** 仅前端布局与状态迁移；若迁移需要新增宿主命令或协议改动，停下报告。
- **完成后交 ARCHITECT 复核；Part B 的自动化缺口（真实窗口对话端到端）继续由用户人工点验覆盖。**

### OD-ARCH-031：模型设置跳转 DSH + 对话流工作区改造（Part A 已执行；Part B 的摆放位置经用户纠正后由 OD-ARCH-032 重做）

- **状态（2026-10-20，ARCHITECT 执行）：** Part A **已执行并实测**（提交 `32200a8`）——设置页常驻「打开 DSH 设置（登记 key）」与
  「刷新端点信息」；点击**真的把 DSH Desktop 拉起来**；未安装时明确报错；端点读不出时两个入口同样可见。
  宿主为此新增**一个**无参数命令 `task_open_dsh_settings`（固定白名单，不拼接命令、不碰凭证），
  属相对「宿主不新增命令」的**必要偏离**，已审查认可（见 REVIEW.md）。
  Part B 的**结构方向**正确（项目-会话-对话流、三场景降为开场预设），但**摆放位置违反用户裁定**：
  项目/会话树被放进了首页、首页变成左右分栏；用户明确首页不做分栏、不出目录树，
  DSH 的项目要收进**一个 Dock 模块页**。Part B 已按此重做（OD-ARCH-032，提交 `828c5a6`）。

- **背景：** 用户首次实测反馈两点：①设置页没有"刷新端点信息"按钮（根因：端点解析失败时整个开关区不渲染，只剩提示行；且把模型配置定位成"只能去 DSH 侧改的外部事实"，违背"在工作台里能用起来"的初衷）；②三场景的"表单+分步点选"向导式设计不符合真实用法——用户要的是 DSH/Codex 式对话流：项目 → 新会话 → 直接说事，AI 自主调 Skill 干活，材料随对话带上。
- **用户已定取舍：** 模型设置**跳转 DSH 原有设置界面**（不自建编辑器）；工作台本身做对话流改造。

#### Part A：模型设置跳转 DSH（小改动，先行）

1. 设置页"模型权限"区块改为引导 + 跳转：一个"打开 DSH 设置"按钮（复用系统打开方式启动 DSH Desktop 设置界面或其设置页；若 DSH Desktop 未运行则给出明确提示，不静默失败）。
2. 修掉"端点解析失败就不显示刷新按钮"的坑：刷新入口在解析失败时也必须可见可点，失败原因明示。
3. 保留现有权限三类开关与默认全关语义不变。
- **验收：** 真实窗口点跳转能到达 DSH 设置；返回 OmniDesk 点刷新，权限区在端点可识别/不可识别两种状态下都可用；关闭权限时提交仍被明确拒绝。

#### Part B：对话流工作区（主改动）

1. **界面结构改为「项目 → 会话」**：左侧变为项目列表（复用现有工作区/项目概念与 engine_list_sessions 的项目分组能力），项目内为会话列表；新建会话 = 直接进入对话。
2. **对话即工作**：会话内就是对话流——用户直接写要求、贴材料路径、传文件；模型自主推进，流式输出。不再要求用户走"表单 → 点按钮 → 逐步确认"的向导。
3. **三场景降级为开场预设**：新建会话时可选一个场景（周报/文案写作/项目跟踪）作为开场上下文——系统自动带入对应 Skill 提示、材料位置约定与写作要求，之后完全交给对话。不选场景就是普通对话。
4. **底层复用，不新增通信层**：会话创建/列表/恢复/流式输出全部走现有 engine 适配层与 DSH ACP 会话机制（这些接口已存在并有测试）；宿主不新增命令，前端不改协议。
5. **向导界面去留后置**：现有三场景向导页保留为可选入口（Dock 内折叠），等用户实际用一段时间确认对话流好用后，再由用户决定删除或保留。
- **验收：** 真实窗口内：新建项目 → 新建会话（选周报预设）→ 对话中给出合成材料路径与写作要求 → 模型流式回复且自动遵循预设的写作要求 → 会话关闭重开后可恢复继续对话；不选场景的普通会话同样可用；现有向导入口不受影响；Rust/tsc 测试通过；runtime/ 无差异。
- **影响范围：** desktop/src 前端结构（App/Dock/会话视图为主）、少量 api.ts 绑定、无宿主新命令、无协议改动、无 runtime/ 改动、不动凭证与路由。
- **风险：** ACP 流式输出在界面的呈现是本次主要技术风险——适配层已有 session/update 事件处理，但前端尚未以对话流形式渲染过，若事件粒度不适配，以最小改动在前端聚合，不倒逼协议扩面。若发现 DSH 会话能力无法支撑"AI 自主调 Skill"（例如 profile 无 Skill 加载机制），停下报告，不擅自改 DSH。
- **执行顺序：** Part A 与 Part B 可一批交给 BUILDER 连续完成（Part A 小，先做热身也先解决你设置端点的急迫需求），完成后 ARCHITECT 一次性复核。

> **当前状态更新（2026-10-19）：** OD-ARCH-025 的技术链路有证据，但产品验收不通过；OD-ARCH-026 的模型场景与 UI 实现已由 OpenCode 报告完成，功能部分仍待 ARCHITECT 独立复核。首页框架已由用户在本机确认并定为 UI 基线，详见 REVIEW.md 顶部与 GOALS.md。OD-ARCH-027 Step 2–4 已按范围复核通过，Step 5 待批准。Token 消耗审计阶段一完成；**OD-ARCH-028 阶段二试点已获用户批准（2026-10-19），已执行完成并通过 ARCHITECT 独立复核**（三场景真实窗口闭环 + 失败/取消负例，证据见 `evidence/od-arch-028/`）。**OD-ARCH-029（备份收口 + 双 Agent 复测 + 使用记录通道自检）已获用户批准并执行完成**：8 批提交已推送到 `origin/main`（远端与本地 HEAD 均为 `3f84ca5`）。除该次**一次性**备份授权外，仍不授权合并 worktree、提交、推送或打标签（见 docs/decisions.md D-022）。

- **状态：已执行（2026-10-20，ARCHITECT）。** Part A 完成并实测（设置页常驻「打开 DSH 设置（登记 key）」与
  「刷新端点信息」；点击**真的把 DSH Desktop 拉起来**；未安装时明确报错；端点读不出时两个入口同样可见）；
  为此新增**一个**无参数宿主命令 `task_open_dsh_settings`（固定白名单启动器，不拼接命令、不碰凭证），
  属相对「宿主不新增命令」的**必要偏离**，已标注待复核。Part B 完成：首页改为「项目 → 会话 → 对话」，
  三场景降级为开场预设（向导页保留在 Dock），新增 `conversationPrompt.ts` 纯函数（上下文累积 + 预设注入），
  脚本级验证 12/12。**未自动化：** 真实窗口的合成键盘注入进不了输入框，端到端打字链路需人工点一次；
  「同一 DSH 会话多轮」在当前协议下不存在（工作台每轮一个新会话 + 重放上文），要真多轮须给适配层加动词（另行批准）。
  证据见 `evidence/od-arch-031/`，结论见 REVIEW.md 同名小节。

### OD-ARCH-030：真实使用前置收口——端点引导 + 记录持久化 + 杂物清理 + 文档登记提交（已执行完毕，ARCHITECT 复核通过）

- **状态：已执行完成（2026-10-20，ARCHITECT 执行）。** 四件事：①**端点引导与验证通过**——设置页引导 8 项要点逐条核对为真（本机回环/非回环分类、默认全部关闭、无法确认位置永不放行、自行确认授权与数据去向、端点来自 DSH 侧、不读取凭证值等），并**修正一处会误导人的表述**（「在隔离环境中把模型配置指向明确的地址」→「先把 DSH 侧的模型配置指向一个明确的地址」）；假端点验证打通「端点判定 → 权限开关 → 配置落盘 → 提交放行 → 模型返回」，且权限关闭时提交被明确拒绝、**不产生任何任务**（不出错、不静默降级）。②**使用记录持久化到正式根通过**——落盘 `~/OmniDesk/app/logs/usage.jsonl`（仓库外，**产品代码无需改动**，此前落在 /tmp 只是测试覆盖了环境变量），内容只含枚举与时间，界面可读回，验证用的 2 条合成记录已用产品自带删除路径清空（文件 0 字节）；`quarantine/`（真实案件材料）与 `runtime/`（含凭证）经目录快照比对**未新增未改动**；保留期限与清理规则登记在 `docs/OmniDesk-Desktop-Data-Layout.md` §5.1。③**遗留 0 字节文件已删除**。④**登记文件与本轮改动已按红线扫描流程提交并推送**（提交号见 STATUS.md 同名小节与 `git log`）。遗留：真实端点与真实模型未验证；T-001 ④（双 agent 派工）因 OpenCode 子代理深度限制未通过，待用户决策（路径 A/B 已列在 `docs/agent-toolchain-retest.md`）。结论见 REVIEW.md 同名小节。
- **背景：** 用户下一步将进行首次真实使用并给出需求反馈；此前的使用记录自检写到了一次性 /tmp 隔离根，正式使用前须把数据落到 ~/OmniDesk 正式数据根；T-001 双 agent 因免费模型额度持续限流，暂不依赖其派工。
- **目标（四件事，一次做完）：**
  1. **模型端点设置引导与验证**：在设置页核实「成果目录与模型权限」的端点配置引导文案完整（本地/外部端点分别说明、权限开关默认关闭、开启需用户自行确认）；用一次合成材料 + 回环假端点验证配置写入与生效路径，不出错不静默降级。不改路由代码与凭证。
  2. **使用记录持久化到正式数据根**：把使用记录写入位置从临时隔离根确认/切换为 ~/OmniDesk 正式数据根的 logs/usage.jsonl（仓库外），验证写入、读回、格式仍为枚举+时间；保留期限与清理规则在文档中登记（暂不实现自动清理）。
  3. **遗留杂物清理**：删除工作区遗留 0 字节文件「当前没有正在执行中的」（用户批准本计划即视为授权）。
  4. **提交并推送**：把 029 复核后的 5 个登记文件（PLANS/REVIEW/STATUS/decisions/agent-toolchain-retest）与本轮新增改动，按既有红线扫描流程提交并推送。
- **影响范围：** 设置页文案与少量 Rust/前端验证逻辑、使用记录落盘路径、1 个杂物文件、状态文档、Git 提交推送。不改 runtime/、不新增通信层、不扩大模型路由功能面、不合并 omniharness-ui worktree。
- **验收标准：** 端点配置引导文案覆盖本地/外部与默认关闭语义；假端点验证通过；使用记录在 ~/OmniDesk 正式根写入读回成功且无正文；杂物文件已删除；红线扫描通过后推送，本地与远端 HEAD 一致；工作区仅剩明确说明的未跟踪项。
- **风险与停止条件：** 涉及真实数据根写入，须验证不触碰既有 app/quarantine/runtime 子目录内容，发现冲突即停；端点配置若牵动 IPC 契约形状则停下报告，不擅自扩面。
- **复核结论（2026-10-20，REVIEW.md 顶部）：通过。** 四项全部完成并入库推送（HEAD 581c964，远端一致）。附带收益：T-001 双 agent 四项复测全部闭环（subagent_depth: 2 派工链路调通）。真实服务商端点与真实模型质量未验证，属用户首次真实使用范围。**当前无待批准计划：等待用户首次真实使用反馈**（docs/pilot-first-real-use.md），下一轮需求以体验为输入制定。

### OD-ARCH-029：备份收口 + 双 Agent 复测 + 使用记录通道自检（已执行完毕，ARCHITECT 复核通过，见 REVIEW.md 顶部）

- **状态：已执行完成（2026-10-20，ARCHITECT 执行）。** 三件事：①**备份收口完成**——提交前对 681 个待提交文件做敏感信息扫描（命中 28 条全部为无害：红线词表本身、合成测试号码、函数调用、文档说明），分 8 批提交（`2705148` 前端 / `90e2731` 宿主 / `1be6992` 引擎 / `b5e7a1e` 样例 / `bfff57f` 脚本 / `593c49f` 双 agent / `2b4be2d` 文档与审计 / `3f84ca5` 验收装置与证据），并已推送 `origin/main`（`5ffc710..3f84ca5`），**远端 refs/heads/main 与本地 HEAD 一致**；提交前 682 项与提交后 681 已提交 + 1 剩余未跟踪逐项对齐，**无丢失**。②**T-001 三项复测未能在本会话执行**（subagent 仍返回 `Unknown agent: super-plan`；两个 agent 文件创建时间晚于本会话的 agent 目录），按风险条款延后，清单见 docs/agent-toolchain-retest.md，**新开会话即可跑完**。③**使用记录通道自检通过**：记录数 0→1，落盘在仓库外数据根，内容只有枚举与时间（无输入/输出/正文/路径），界面「查看记录（1）」可读回。剩余未跟踪项仅 1 个 2026-10-16 遗留的 0 字节空文件 `当前没有正在执行中的`（未提交、未删除）。结论见 REVIEW.md 同名小节。
- **背景：** OD-ARCH-028 三场景真实窗口闭环已复核通过；项目未偏离。当前最大风险是工作区 115 项改动长期未提交（单点丢失）；T-001 三项复测被主动挂起（清单在 docs/agent-toolchain-retest.md）；用户首次真实使用尚未开始。
- **目标（三件事，一次做完）：**
  1. **分批提交并推送备份（最优先）**：把工作区改动按 evidence/docs/desktop/tests 等逻辑分组，先对每批做敏感词+凭证启发式扫描并人工核对文件清单，再分批提交到 main 并推送 github.com/klzone/omniharness。会话日志、真实材料、凭证、仓库外数据一律不提交。每批记录提交号；完成后 git status 仅剩余明确说明的未跟踪项。
  2. **T-001 三项复测**：按 docs/agent-toolchain-retest.md 清单，在新会话中执行三项只读复测（git rev-parse 放行、管道命令匹配语义、双 agent 实际模型标识），结果原样登记 STATUS.md。
  3. **使用记录通道自检**：用合成数据点一次「使用记录（可选）」，验证记录写入仓库外数据根、只含枚举与粗耗时、不含输入输出正文，并能读回显示；结果登记。
- **影响范围：** 仅 Git 提交/推送、docs 证据登记、一次新会话只读复测、一次使用记录写入读回。不修改业务代码、runtime/、模型路由、凭证、用户数据；不合并 omniharness-ui worktree；不扩大授权范围。
- **验收标准：** 远端与本地 HEAD 一致且推送成功；红线扫描记录在案；三项复测有真实运行输出（失败原样记录）；使用记录写入/读回成功且无正文落盘；既有未提交改动在提交前后不丢失不覆盖。
- **风险与停止条件：** 发现疑似真实材料或凭证内容 → 停止该批提交并报告；双 agent 限流 → 复测延后不阻塞备份；远端认证失败 → 停止并报告，不重试绕过。
- **后续（不在本计划内）：** 复核通过后，由用户按 docs/pilot-first-real-use.md 完成首次真实使用并记录体验数据；UI 微调与真实模型端点启用另行单独批准。

### 工具链计划 T-001：OmniDesk 开发固定双 Agent「超级Plan / 超级劳工」（已部署；运行复测待补）

- **状态：用户已批准并已部署完成（2026-10-19），文件已入库（提交 `593c49f`）。** 复测**2026-10-20 晚全部闭环**：① `git rev-parse` 放行**通过**；② 管道/复合命令语义**已明确——被拒（白名单仅按单条命令前缀匹配）**；③ `git diff --check` **通过**；④ 派工链路**通过**（用户批准 B 方案：新增 `.opencode/opencode.json` 设 `experimental.subagent_depth: 2`，同会话即时生效，ARCHITECT→super-plan→super-worker 端到端调通）。结果表见 [docs/agent-toolchain-retest.md](docs/agent-toolchain-retest.md)。既有 111 项差异已由 OD-ARCH-029/030 会话提交入库；本计划不接触业务文件。
- **分类：功能实现（OpenCode 项目级协作配置，非 OmniDesk 产品代码）。** 调研结论：工作区尚无 `.opencode/` 目录；OpenCode V2 支持 `.opencode/agents/<id>.md` 项目级 agent，frontmatter 声明 `description / mode / model / permissions`，正文为 system prompt；权限规则 `action/resource/effect`，后匹配者优先（宽规则在前、例外在后）。
- **目标：** 创建两个固定 agent，均固定模型（用户 2026-10-19 追加指定）：超级Plan = `opencode/glm-5.3-flash-free`；超级劳工 = `opencode/deepseek-v4.1-flash-free`（均已核实为 active 引用）：
  1. **超级Plan**（`.opencode/agents/super-plan.md`，mode: all，可作主 agent 也可被子 agent 调用）：专职 OmniDesk 规划与设计。system prompt 内嵌协作纪律（先读 Git 状态与代码、分类需求、产出计划、证据区分、不伪造运行结论、越界/高风险即停、不读凭证）。权限矩阵：
     - `* / * → deny`（默认全拒），随后放行：`read / grep / glob → allow`；`edit → PLANS.md allow`（其余路径仍 deny）；`shell → allow` 仅限只读命令 `git status* / git log* / git diff* / git show*`；`subagent → 超级劳工（ID: super-worker）allow`；`read → deny` 凭证类文件（`**/.env*`、`**/*credentials*`、`**/*secret*`、`**/*.pem`、`**/*.key`）。`webfetch / websearch / web`、其余 edit/shell、其他 subagent 皆拒绝，如需另批。
  2. **超级劳工**（`.opencode/agents/super-worker.md`，mode: all）：专职在获批计划范围内执行。system prompt 内嵌：只做获批范围、开工前记录 git status/HEAD、保留全部既有未提交/未跟踪/暂存改动、执行记录写 STATUS.md、区分证据类型、计划外高风险差异停手报告、不提交/推送/打标签、不改 runtime/ 与模型路由凭证、不接触真实业务材料与用户数据。权限不加额外限制（等同 build 默认：工具默认可用，敏感路径如环境文件、工作区外访问弹确认）。
- **交接机制：** 超级Plan 调研后把计划写入 PLANS.md（它唯一可写的文件）→ 用户明确批准 → 超级Plan 用 subagent 调用「超级劳工」执行 → 完成回报 → ARCHITECT（本会话/REVIEW.md 流程）独立复核。不设 `default_agent`（不改变现在的默认会话主 agent），不改 `opencode.json`、模型路由与凭证。
- **影响范围：** 仅新增 2 个文件 + 本次 PLANS.md 登记本身。明确不做：不设 default_agent、不改 provider/model 配置、不提交/推送/打标签、不动 runtime/ 与既有工作区差异。
- **验收方式与标准：** (1) 新会话/子 agent 列表能发现两个 agent；(2) 超级Plan 实测只能读代码库、写 PLANS.md、跑只读 git、只能调 超级劳工；(3) 超级劳工可编辑与执行 shell；(4) 两者的实际模型为 `opencode/glm-5.3-flash-free`。可在批准后用一次真实小任务（如某个只读调研 + 一小步执行）端到端试用并记录证据。
- **风险与回退：** agent ID 兼容性风险已通过用户选择英文文件名规避；如实测仍发现 ID/资源匹配问题，向用户报告并按实际表现修清单经批准再改。权限规则是配置级防线，不是代码级安全控制；两个 agent 的 prompt 均写入「不读取/打印凭证、不扩大用户答复适用范围」的纪律。本计划全程不提交、不推送。
- **交付：** 两个 agent 文件、本计划状态更新、（如用户同意）一次端到端试用记录；最终交 ARCHITECT 复核。
- **执行结果（2026-10-19 → 2026-10-20 补记）：** 部署与静态校验已完成（见下）。**三项运行复测（`git rev-parse*` 放行、管道/复合命令匹配语义、super-plan→super-worker 派工链路）仍未执行**：2026-10-20 实测确认两个 agent 文件的目录正确（OpenCode V2 文档：项目级 agent 放 `.opencode/agents/<name>.md`），但**当前会话的 agent 目录是在文件创建之前建立的**，subagent 工具返回 `Unknown agent: super-plan`，故无法在本会话内复测。复测步骤已写成可直接照做的清单：[docs/agent-toolchain-retest.md](docs/agent-toolchain-retest.md)；**新开会话即可发现这两个 agent**，届时三分钟可跑完。

- **执行结果（2026-10-19，ARCHITECT 本会话直接实施，非 BUILDER 派工）：** 新增 `.opencode/agents/super-plan.md`、`.opencode/agents/super-worker.md`（未设 `default_agent`、未改 `opencode.json`、未提交推送）。模型按用户指定固定：super-plan = `opencode/glm-5.3-flash-free`，super-worker = `opencode/deepseek-v4.1-flash-free`（均已核实 active）。**静态校验：** 两文件 frontmatter 用 YAML 解析通过，键为 description/mode/model/permissions，super-plan 16 条权限规则按「先宽后窄」顺序正确落地（`*/*` deny → read/grep/glob allow → edit 仅 PLANS.md → 5 条只读 git allow → subagent 仅 super-worker → 5 条凭证类 read deny），super-worker 无 permissions 段（= build 默认）。**运行实测：** 两个 agent 均可被 subagent 工具发现并加载；super-plan 的 read/glob/grep 可用、单条 `git status --porcelain` 可执行、`git rev-parse HEAD` 与 `find` 等未放行命令被正确拒绝、未越界写任何文件；super-worker 可加载并自述模型为 DeepSeek V4.1 Flash Free。**挂起项（用户指示暂缓，非失败）：** 补入 `git rev-parse*` 后的复测、含管道/复合命令的匹配语义、super-plan→super-worker 派工端到端链路——待模型限流解除后按 REVIEW.md「下一步」补做。

### OD-ARCH-028：阶段二试点——定版首页接入工作模块，以周报完成首个体验闭环

- **状态：已执行完成，并通过 ARCHITECT 独立复核（2026-10-19）。** 前置门（OD-ARCH-026 独立复核）通过后继续；三场景真实窗口闭环成立，失败/取消不产生成果文件，回归全绿。证据：`tests/verification/desktop/evidence/od-arch-028/`（脚本 + JSON + 29 张截图 + README）；结论见 REVIEW.md 同名小节。遗留：真实模型质量、真实 Wayland 桌面、真实使用体验指标（澄清/返工）与 Token（引擎未上报 usage，记未知）未验证。本计划承接 Token 审计阶段二，采用集中实施、集中验收。用户已说明 OpenCode 完成首页 UI 初稿并将其定版；这确认产品方向，不代表批准整体合并另一 worktree。开工快照：主工作区 HEAD 5ffc710，工作区 111 项差异（含既有开发改动和 Token 审计文件，必须保留）；候选 worktree /mnt/data/Code/omniharness-ui 有独立未提交差异，文件逐项核对。
- **目标：** 把已确认首页作为稳定外壳，接入周报与随记、文案写作、项目跟踪入口；以周报完成首个体验闭环：用户提供公开/合成材料，模型整理起草，用户修改确认并保存。其余两个模块接通现有模型工作流并回归，不扩展专业规则或新架构。数据分析和模型调用 Python 暂缓。
- **前置门：** ARCHITECT 先根据 OD-ARCH-026 实际源码、差异与测试作独立复核。若材料边界、模型结果真实性、权限或保存条件不通过，停止本计划功能实施，先提交有边界的修复计划。
- **影响范围：** 通过前置审查后，按需选择性移植/修改主工作区 desktop/src/App.tsx、desktop/src/shell/**、三场景现有 UI 与流程接线、必要前端依赖，以及本计划证据和状态文档。先只读对比候选 worktree；不整体复制/合并，不覆盖既有改动。不得改 runtime/、模型路由/凭证、通信协议、真实用户数据或旧数据目录；不提交、推送、打标签。
- **实施步骤：** (1) 独立审查 OD-ARCH-026，只有通过后继续。(2) 对照用户现场确认的首页与候选交接说明，列出逐文件移植清单；以本工作区已确认 UI 基线为准，不恢复被后续反馈取代的右栏、假数据、持续动画或过时状态提示。(3) 接通左侧三个入口，复用现有任务 API、取消和状态生命周期；仅修复可观察到的不一致和使用阻断，不做全面视觉重设计。(4) 用合成或公开材料在真实桌面窗口完成周报闭环：选材、模型起草、编辑、人工确认、仓库外保存并读回；验证失败与取消不伪装成功。(5) 文案与项目模块覆盖材料传递、模型起草、编辑/保存的定向回归。(6) 集中执行计划相关前端构建、Rust/宿主和引擎回归、桌面成功/失败/取消、隔离与 runtime 无差异检查、真实窗口逐模块截图；更新状态和证据后一次性 Review。
- **验收标准：** 首页保持定版布局和交互；三个模块都可从左侧进入；周报完成真实桌面合成/公开材料下的模型起草、人工编辑确认及仓库外保存闭环；文案和项目通过材料到模型、结果呈现及保存回归；失败/取消不产生成功状态或伪造正文；结果标注未复核；必要回归通过，所有既有工作区差异完整保留。
- **试点评价：** 记录执行请求数、人工澄清数、返工轮数、耗时和测试结果。只有遥测能可靠映射到本计划线程时才记录输入/输出/缓存 Token，否则标未知、不估算。与历史任务比较时说明范围差异，不从单次试点推导普遍比例。
- **风险与停止条件：** 026 审查不通过、两个 worktree 的重叠无法安全分离、候选设计与已确认基线冲突，或合成模型链路不能稳定验证时，停止相关实施并报告。若场景页面已满足目标，只做必要入口接线。真实业务材料、真实端点授权和同事试用不在本计划范围。
- **交付：** 精简执行摘要和证据路径，包含 UI 截图、闭环结果、命令与通过数、Token 可测性、遗留问题及 Git 状态；完成后停止并等待 ARCHITECT 独立验收。

### OD-ARCH-026：AI 对话为中枢的产品校正与三项工作场景闭环

- **状态：用户已批准；BUILDER（OpenCode）已执行完成，待 ARCHITECT 独立复核。** 产品方向校正与桌面功能计划。OD-ARCH-025 的执行证据和产品缺口见 REVIEW.md、STATUS.md 与 [证据索引](tests/verification/desktop/evidence/od-arch-025/README.md)；本轮执行记录与证据见 [OD-ARCH-026 证据索引](tests/verification/desktop/evidence/od-arch-026/README.md)。
- **执行结论（执行方记录，非审查结论）：** 首页已改为对话中枢，模型权限由宿主回报并在提交时强制执行（关闭时提交关闭并给出设置入口）；Dock 新增周报与随记、文案写作、项目跟踪三个入口；三个场景改为模型起草——提示词由用户输入、**用户明确选择的材料文件**正文与可编辑流程包规则组成，确定性核对单独成栏且不作为成果，成果仅来自模型初稿并经人工确认后保存；流程包新增可选可编辑规则 `draft.rules/sections`；宿主新增只读单文件材料读取 `task_read_material`。前端 `tsc`/构建通过，Rust 当前源码全测 142/142，`verify_ai_draft` 27/0，新增 `verify_scenario_materials` 5 场景 0 failed，桌面链路/工作台引擎路径/取消/会话回归全部通过；一次性真实窗口完成周报场景「选择材料 → 模型初稿 → 人工确认 → 保存」闭环（产物含模型正文与材料来源 sha256）。未验证项见证据索引 §3。
- **目标：** 让 OmniDesk 成为以大模型对话和执行指令为中枢的定制 Agent，而非文件管理器或本地汇总器。首页约半屏为主要对话区并可直接启动工作；左侧悬浮图标进入周报与随记、文案写作、项目跟踪。模型基于用户输入和用户明确选择的文档/表格材料，完成整理、提炼、归纳与起草；代码负责权限、任务状态、文件读写、来源记录及确定性校验。周报、文案、项目管理共享交互骨架，各自采用可编辑流程规则。数据分析最后建设，后续另行规划由模型按任务调用预制 Python 处理表格、文档或数据库；本计划不实现该能力。
- **影响范围：** 主工作区现有 React/Tauri/Rust/Python 引擎连接、三类工作场景、可编辑 Skill/任务配置与合成回归；将 `/mnt/data/Code/omniharness-ui` 作为 UI v2 设计和候选前端来源，只读审查后选择性移植，不整体合并其 worktree。不得修改 `runtime/`，不得读取/打印凭证、改变模型路由、接触真实业务材料或用户数据；保留两边全部既有差异，不提交、推送、打标签或合并 worktree。
- **实施步骤：** (1) 记录两个 worktree 的 HEAD、状态和改动边界；只读对照 UI v2 首页对话、Dock 与主工作区 UI/API/权限代码，选择性移植，不整体合并。 (2) 校准 GOALS.md 和 docs/decisions.md：大模型对话是首页与任务执行中枢；首次模型权限严格关闭，用户可按自身授权在设置中开放；不得把有限单位答复扩大到周报、写作、项目跟踪或具体端点。 (3) 接入首页主要对话区及左侧悬浮图标导航；首页对话和各工作场景复用现有引擎 API、取消与任务状态，不另造通信层。 (4) 让周报、文案写作、项目跟踪的模型任务都真实经过当前适配链路：模型读取用户输入及用户明确选择的文档/表格内容，按可编辑、带版本的 Skill/场景配置整理提炼并起草；结果回到界面供用户修改、确认和保存。代码只做日期、状态、数值等确定性工作和文件边界控制，不允许本地模板拼接伪装模型产出。表格/文档解析采用现有可用能力；不得借机开放任意目录扫描或未授权数据库访问。 (5) 为三个场景各设合成模型回归，证明所选材料进入模型任务、模型结果被展示并可保存；同时覆盖失败与取消。真实工作数据/真实模型不作为本计划测试条件。 (6) 在隔离数据根的真实桌面窗口验证对话首页、三类场景、文件选择、保存及重开。运行当前源码 Rust 全测、前端类型检查与构建、桌面 success/failure/cancel、准入/会话/草稿回归、`git diff --check HEAD` 和 `runtime/` 无差异检查；更新 STATUS.md 与证据索引，完成后停止等待 ARCHITECT Review。复杂数据分析、AI 调用预制 Python 对表格/文档/数据库处理，及一般数据库接入均不在本计划实施范围，另行规划。
- **验收标准：** 首页打开即有约半屏、可直接执行任务的主要对话区；左侧悬浮图标能进入周报与随记、文案写作、项目跟踪。三个场景均以真实合成模型调用作为内容处理步骤，所选文档/表格材料参与模型输入，结果可见、可编辑、人工确认并保存；Skill/流程规则可在不改代码时调整。用户设置控制模型权限且 UI 与宿主一致；无模型输出时明确失败/阻塞，不能展示本地汇总冒充 AI 结果。计划要求的测试通过，所有现存工作区改动保留，`runtime/` 无变化。
- **风险与停止条件：** 两个 worktree 均有大量差异，不能整体复制或合并；文件归属或重叠不清时停止移植并报告。不要读取凭证或推定具体模型、服务商、设备和真实工作用途获准；无安全的合成模型链路时停止 UI 提交测试并报告缺口，不以绕过准入或伪造模型回复代替。文件解析仅限用户明确选择的文件；本计划不允许任意扫描目录或接入数据库。若现有适配层不支持需求，先说明契约缺口，不改 Runtime、不自造通信协议。
- **交付：** 集中完成 UI 校正与三类 AI 场景闭环，提供变更文件、真实窗口证据和测试结果；不提交、不推送、不合并 worktree。独立 Review 通过后，由用户做实际工作体验反馈。

#### 首页版式回归：对话为主 + 顶部三模块（用户已批准，2026-10-19；**仍属 OD-ARCH-026，不另开新编号**）

- **状态：已由 ARCHITECT 复核按获批口径通过（2026-10-19）；仍属 OD-ARCH-026。** 按下列口径落地：新增 `HomeCanvas.tsx`、`App.tsx` 首页改渲染、删除 `HomeDashboard.tsx`、`HomeChat.tsx` 仅改展示；`tsc`/`npm run build` 通过；真实窗口（一次性 Xvfb + 隔离数据根 + 回环合成端点）四页截图、首页 5 帧签名=1、空闲 60 s `WebKitWebProcess` 0.08%；工作区条目仍 108。执行记录见 `STATUS.md` 同名小节，证据见 `evidence/od-arch-026/README.md` §4 与 `home-layout/env.json`。未验证项：真实模型质量、Wayland/release、窗口缩放、输入法/粘贴。

- **授权与归属：** 用户确认按 `_design/stitch_omnidesk` 的原始版式恢复首页，并明确「继续 026」；本项是 OD-ARCH-026 的产品校正延续，不登记为 OD-ARCH-028。执行完成后写入 STATUS.md 与证据索引。
- **目标版式：** 删除右栏整列（周报/文案/项目三张场景卡、当前版本边界、查看引擎状态、「OMNIDESK · LOCAL FIRST」）；删除「对话中枢」容器头、说明文字与三个快捷入口；顶部一行 = 机器人（`/mascot.png`）+ 今日提醒（今日重点推进）+ 日程节律（三周日历）；下半屏整幅 = 对话消息流 + 输入框，消息流顶部向上渐隐。
- **用户已确认的三项口径：** (1) **默认静态**：不使用 `animate-float-mascot`、`animate-pulse`、卡片 `backdrop-blur-md`，保持 OD-ARCH-027 Step 2/4 的成果；要动效须另行批准。(2) **不伪造数据**：今日提醒沿用诚实空态「未接入数据源」，日程只给三周日期骨架，不写演示性待办/统计数字。(3) **权限提示压缩**：模型权限信息收为输入框底部一行状态（未开启时含「去设置」），保留宿主事实，不保留整块说明。
- **影响范围：** 新增 `desktop/src/shell/HomeCanvas.tsx`（从只读 worktree 选择性移植并适配本树）；`desktop/src/App.tsx` 首页路由；删除 `desktop/src/shell/HomeDashboard.tsx`；`desktop/src/shell/HomeChat.tsx` 空态与权限提示。复用 `ThreeWeekCalendar.tsx`、`public/mascot.png`。不改 Rust、`runtime/`、模型路由、凭证、用户数据、第二 worktree；不提交、不推送、不打标签。
- **验收：** 首页无右栏与蓝框内容；顶部三模块 + 下半屏全宽对话成立；消息流顶部渐隐；默认无持续动画；`tsc`/`npm run build` 通过；真实窗口逐页截图无回退；未验证项与既有改动保留情况如实记录。

#### 窗口外框一体化（frameless，用户已批准并验收，2026-10-19）

- **授权：** 用户要求把系统标题栏（最小化/最大化/关闭）融入软件界面，类似无边框应用。本项只改窗口外框与顶栏控件，不动页面内容与业务逻辑。
- **改动：** `src-tauri/tauri.conf.json` 主窗口 `decorations: false`；`src-tauri/capabilities/default.json` 增加窗口控件所需最小权限（minimize / toggle-maximize / close / is-maximized / start-dragging / start-resize-dragging / internal-toggle-maximize）；新增 `src/shell/WindowChrome.tsx`（自绘 — □ × 与 8 个边缘/角落缩放热区，只调用真实 Tauri 窗口 API）；`TopNav.tsx` 顶栏加 `data-tauri-drag-region` 并嵌入控件；`App.tsx` 挂载缩放热区。
- **验收：** 无系统标题栏；顶栏可拖动、双击最大化；— □ × 真实生效；无边框后仍可从四边/四角缩放；最大化时缩放热区隐藏；不遮挡现有顶栏交互。
- **风险与回退：** 无边框在 Wayland/不同 WM 下的拖动与缩放存在平台差异；若窗口无法移动或缩放，回退为 `decorations: true` 并移除顶栏控件后重建。需要重新 `cargo build` 并重启应用（仅前端 HMR 不生效）。
- **状态：已完成并经用户现场验收（2026-10-19）。** `tsc`/`npm run build`/`cargo build` 通过，Xvfb 冒烟确认控件渲染与页面完整；用户重启后确认系统标题栏消失、拖动、最小化/最大化/关闭均正常。该外框与当前首页共同作为**已确认 UI 基线**（见 GOALS.md、docs/decisions.md D-020）。

### OD-ARCH-027：桌面端 WebKit 渲染性能治理（OD-ARCH-026 支线）

- **状态：计划已建立（Step 0 证据归档与 Step 1 计划完成）；Step 2 已由 ARCHITECT 独立复核按范围通过（附关键限定，见 REVIEW.md 顶部：原始空闲 ~100% 症状未复现，本步无可宣称的应用级收益）；Step 3 已由 ARCHITECT 复核按范围通过（零产品代码改动，诚实空结果见 REVIEW.md）；Step 4 已由 ARCHITECT 复核按范围通过（含最终「状态点默认静态」改动，满足 §5 的 <10% 目标）；Step 5（真实桌面回归）已于 2026-10-20 在用户自己的 Wayland 会话执行完成：原生 Wayland × DMABUF{1,0} 与 XWayland × DMABUF{1,0} 四组分别记录，空闲 60 s 的 `WebKitWebProcess` 均值 0.08% / 0.17%（原生 Wayland）、0.15% / 0.28%（XWayland），导航 P50 0.31–1.22 s、P95 ≤1.63 s、0 超时，5 帧签名 = 1，设置页连续输入逐字落盘（exact_hit）。**原始 ~100% 症状在真实桌面同样未复现，不得表述为性能提升。** 完整计划见 [docs/OmniDesk-Desktop-Render-Performance-Plan.md](docs/OmniDesk-Desktop-Render-Performance-Plan.md)，证据索引见 [evidence/od-arch-027](tests/verification/desktop/evidence/od-arch-027/README.md)（Step 2 见 §6、Step 3 见 §7、Step 4 见 §8、Step 5 见 §9）。未提交、未推送、未打标签，本支线不与 OD-ARCH-026 的功能开发或复核混同。
- **Step 4 执行结论（执行方记录，非审查结论）：** 单因素对照（6 个同构页 ×3 次 ×30 s）显示：静态模糊/玻璃 **0.00%**、纯动画 **26.00%**、动画+模糊 **98.48%**、**「静态模糊 + 唯一一个 6×6 脉冲点」= 100.19%** —— 成本来自「持续重绘 × 大面积模糊」，与动画元素大小无关，这正是原「~100%」的成因。当前 JSX 已不再使用 `.animate-pulse-ambient`/`.animate-float-mascot`，但**顶栏状态点在 `engineHealthy` 时曾用 `animate-pulse`**（顶栏保留着 Step 2 的 `backdrop-blur-2xl`）：预置权限使状态点脉冲后，应用空闲 `WebKitWebProcess` **18.82%**、UI 进程 10.26%（合计约 29%，**不满足 §5 的 <10%**）；加 `prefers-reduced-motion` 后 **0.10% / 0.13%**。**最终改动：** `.status-dot-pulse` 默认 `animation: none`（状态点默认静态、颜色语义与位置/尺寸/布局/文案不变、键帧保留、一行可回退），并保留 reduced-motion 守卫；`TopNav.tsx` 抽出 `NavClock` 使 30 s tick 只重渲染时钟（本轮未再改 TopNav）。**最终复测（60.07 s）：** 引擎健康 **0.10%（UI 0.12%）**（曲线 0.60→…→0.10，5 帧签名 1/1）、出厂默认 **0.03%（UI 0.10%）**，网络 0.00/0.02%、均无 GPU 进程 —— **满足 <10% 目标**。`SettingsPanel` 丢字**未复现**，未改。`tsc`/build/4 页冒烟/输入均通过；导航 P50/P95 引擎健康 2.548/2.597 s、默认 2.567/2.593 s，但该指标被整窗截图读回（约 2.5 s）饱和、不具亚秒分辨力，**不据此下性能结论**；未见视觉回退。未验证项见证据索引 §8.6。
- **Step 3 执行结论（执行方记录，非审查结论）：** `WEBKIT_DISABLE_DMABUF_RENDERER` = `1`（现状默认）vs `0`，一次性 Xvfb 上各 3 次：页面级 `heavy.html` 100.17/100.15/100.18% vs 100.22/100.26/98.76%（`simple.html` 两档≈0%）；应用级空闲 `WebKitWebProcess` 0.07/0.07/0.08% vs 0.07/0.08/0.08%，导航 P50/P95 与设置页输入（6/6 无丢字）一致；**两档都无 WebKitGPUProcess，82 张截图 0 异常帧**。环境探针显示该变量在本沙箱**不可控**（`unset`/`0`/`1` 均 ~100%；`libEGL: DRI3 error`，EGL/加速渲染不可用），只有 `WEBKIT_FORCE_DMABUF_RENDERER=1` 改变代价（62.07%）——因此 A/B 的「无差异」是构造性空结果，**不能读成「DMABUF 在真实桌面不重要」，也不得仅凭 CPU 下降判定为改进**。**Wayland↔XWayland 对照未做**（沙箱无 GPU，且不得由 BUILDER 连接用户真实 Wayland 会话），留待 Step 5。未验证项见证据索引 §7.8。
- **Step 2 执行结论（执行方记录，非审查结论）：** 全屏光斑由 `filter: blur(120–140px)` 改为静态径向渐变光晕；删除 `plate-sculpted`、`.wb-v2 .pane`、`.ofl-v2 .pane` 与首页三处卡片的 `backdrop-filter`；只保留顶栏 `backdrop-blur-2xl` 一个关键视觉层。实测（debug + vite dev，Xvfb，DMABUF=1，60 s）`WebKitWebProcess` 0.33% → 0.07%，导航延迟 P50/P95 1.821/3.350 s → 1.941/2.577 s（含截图读回开销）。**重要限定：当前工作区首页空闲 CPU 本来就远低于 10% 目标，计划 §1.2 的「约 100%」未复现，不宣称性能提升**；MiniBrowser 单因素对照显示模糊是「有无限动画页面」的成本放大器（100.2% → 19.6%）。逐页前后截图无视觉回退。未验证项见证据索引 §6.4。
- **目标：** 消除空闲时渲染进程持续占用一个 CPU 核；交互延迟相对基线明显下降；连续输入不丢字；Dock/顶栏/卡片层级与主要渐变保留。**不**全面删除玻璃特效，**不**顺带重构 UI 架构。
- **影响范围：** 仅桌面样式的渲染成本、渲染后端配置对照、无限动画与前端重渲染治理，以及配套测量脚本与证据。不得修改 `runtime/`、模型路由、凭证、真实材料、用户数据、第二 worktree；不得提交、推送、打标签或合并。
- **执行顺序与门禁：** Step 0 证据归档（已完成）→ Step 1 计划（已完成）→ **Step 2 CSS 性能优化**（仅样式与既有类，可独立回退）→ Step 3 DMABUF/显示后端独立对照（零代码改动，黑屏即回退）→ Step 4 无限动画与 React 重渲染治理（已执行完成，待复核）→ Step 5 真实 Wayland 桌面回归。**严格分阶段验收，禁止一次性改完。**
- **验收阈值（首轮目标，非保证）：** 空闲 `WebKitWebProcess` CPU 稳定后 60 s 平均 <10%；导航延迟相对基线明显下降且记录 P50/P95；固定测试串连续输入无丢字；视觉效果保留；DMABUF 启用/禁用分别记录并可回退；Xvfb 与真实桌面分别记录、不混为一组。CPU 下降本身不构成成功判据。
- **结论纪律：** CSS 特效与 DMABUF 是两个独立变量，必须分别对照测量；不得因相关性把全部成本归给其中一方；若真实桌面仍高于目标，继续采样定位剩余原因，不得改口径或结案。
- **交付：** 分步改动（每步可独立回退）、落盘的测量数据与前后截图、性能报告、回退记录、证据索引与状态更新；不提交、不推送。

### OD-ARCH-025：个人试用前桌面闭环验收（已执行；产品验收不通过）

- **状态：** 底层技术与窗口自动化证据有记录；但写作任务实为合成数据解释、项目流程为本地确定性汇总，首页与用户要求不符。未验证目标产品行为，不作为实际应用试用的通过依据。详情见 REVIEW.md 顶部与证据索引。
- **执行结论（执行方记录，非审查结论）：** 四阶段全部 pass——(A) 真实窗口内完成 AI 辅助写作任务与项目事项流程，两份成果均落在**用户设置的**成果目录并保存；(B) 关闭重启后设置/权限/任务台账/成果与重开都保持；(C) 可控慢关闭下真实关闭窗口，进程 0.25 s 自行退出、无孤儿进程、被打断任务未伪造成功终态；(D) 当前源码重建的 `.deb` 解包后启动入口可拉起真实窗口且沿用同一数据根。Rust 离线全测 134/134、前端构建与类型检查、桌面任务/取消/会话/草稿回归、`git diff --check HEAD`、`runtime/` 无差异均通过。本轮未改动任何 Rust 或前端产品源码。未验证项与边界见证据索引 §1.2/§6。

### OD-ARCH-024：修复生命周期自锁并补齐宿主并发验证

- **状态：用户已批准；BUILDER 四轮执行完成，ARCHITECT 按获批范围通过。** 完成生命周期自锁和关闭期间并发边界修复，并统一窗口销毁关闭入口；证据、限制与独立复核见 REVIEW.md、STATUS.md 与 [证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)。本计划不授权提交或推送。
- **目标：** 让所有实际触达引擎的宿主操作在权限变更和引擎停止期间遵守明确、可测试的并发规则；确认引擎调用包装器不会因重入生命周期锁而卡死。
- **影响范围：** 仅现有 Tauri Rust 宿主、EngineLifecycle、准入/bridge 接线测试、OD-ARCH-023/024 测试证据和共享状态文件。不得修改 runtime/、模型路由、凭证、前端业务范围、用户数据、真实材料或第二 worktree；不得提交、推送或打标签。
- **实施步骤：** (1) 用最小改法消除持有 CallGuard 时再次锁同一 mutex 的路径；世代校验应使用守卫已持有的世代或等价的不可重入校验，不得删除权限世代一致性保障来“修复”卡死。 (2) 逐项盘点 engine_task_status、engine_task_result、engine_cancel_task 与其他已注册引擎命令；为每个命令记录权限变更/关闭期间的预期行为，特别区分取消操作是否必须继续可用，并避免权限变更等待任务结束时把取消永久堵住。 (3) 用真实宿主包装器配合假 bridge/假引擎添加回归测试：至少验证 with_engine_call 和 with_call_gate 正常返回、不自锁；覆盖描述/提交的准入到动作闭环、权限撤销与调用的互斥、停止失败持续拒绝和恢复后放行；验证状态/结果/取消命令按定义的生命周期语义工作。测试须使用确定性同步，不以源码字符串检查或纯生命周期单元测试替代组合路径。 (4) 修正 OD-ARCH-023 的执行报告中可能过度表述的覆盖结论，并建立单一证据索引；如发现测试只是复用旧预编译二进制，应标明其不能代表新源码。 (5) 在不安装系统依赖、不提权的条件下，运行 Rust 完整离线测试及相关宿主集成测试、前端构建、路由/桌面/取消/草稿/会话连续性回归、git diff --check HEAD 和 runtime/ 无差异检查；记录环境限制、实际测试二进制是否由当前源码编译、精确通过数。
- **验收标准：** 两个包装器的组合路径无自锁；权限世代校验仍有效；权限变更完成后旧调用不能启动/使用引擎；命令级并发行为由测试覆盖，取消不会因错误的生命周期锁设计而失效；Rust 与既有回归结果可追溯且对应当前源码；所有既有工作区改动保留。
- **风险与停止条件：** 若修复需要修改 DSH Runtime、协议、模型路由、凭证或数据格式，停止并报告；若本机无法编译 Rust，不得声称当前源码 Rust 全量通过，需提供可复现失败原因和其余独立检查证据；不安装系统开发包、不提权。发现未授权数据或秘密时立即停止相关检查。
- **交付方式：** 集中完成修复、测试、证据和状态更新后停止。不得自行合并、提交或推送；由 ARCHITECT 按实际源码、当前 HEAD 差异和测试证据独立复核。

#### 窗口销毁关闭路径补齐（已完成并复核通过）

- **目标：** 让窗口关闭与命令触发的引擎停止共享同一并发语义，避免耗时关闭时持有宿主锁。
- **影响范围：** 仅 `desktop/src-tauri/src/lib.rs` 现有窗口销毁回调、共享关闭入口、对应 Rust 宿主测试和 OD-ARCH-024 证据/状态记录。
- **步骤：** (1) 复用 `stop_engine()` 或等价共享函数，使窗口销毁回调不在持有 `SharedHost` 锁时等待关闭；(2) 添加确定性宿主级测试，验证关闭等待期间宿主锁仍可取得，状态/结果/取消按既定语义快速返回；(3) 重跑 Rust 离线全测及本计划规定的桌面回归，更新单一证据索引和状态记录。
- **验收标准：** 生产窗口销毁入口不持有宿主锁执行耗时收尾；回归能覆盖该入口的并发关闭行为；计划规定检查通过，或将环境限制标记为未验收。
- **风险与边界：** 不改 `runtime/`、协议、模型路由、凭证或用户数据；保留全部既有工作区改动，不提交、不推送、不打标签。
- **状态：** 用户已批准并由 BUILDER 完成；ARCHITECT 按范围复核通过，结论见 REVIEW.md 顶部。窗口回调仍在事件循环线程同步等待关闭，此行为沿用既有设计；本次只修复不持有宿主锁的并发问题。
- **执行结论（执行方记录，非审查结论）：** 新增 `on_window_destroyed_shutdown(&SharedHost)` 复用共享入口
  `stop_engine()`，`WindowEvent::Destroyed` 回调只调它，不再 `state.lock()` 后直连 `bridge.shutdown()`；
  关闭入口唯一。新增确定性宿主级回归
  `window_destroy_shutdown_does_not_hold_host_lock_while_waiting`：8 s 关闭窗口期间宿主锁 <1.5 s 可取得、
  取消/状态/结果 <1.5 s 返回“关闭中、未发送”、关闭开始后引擎未收到取消。变异验证（改回持锁等待）令该测试以
  `关闭等待期间宿主锁应可立刻取得（等了 8.041135831s）` 失败。回归 **134/134**，详见
  [证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)「第四轮」。

**ARCHITECT 复核后的限定补测/修正任务（用户已批准；已于 2026-10-19 执行完成并复核通过）：**

> **执行结论（2026-10-19，执行方记录，非审查结论）：** 三项要求均已落实。并发边界改为：`EngineSupervisor` 在
> **supervisor 锁内**原子完成「置位“正在关闭” + 摘除通道」（`begin_stop()`），耗时的协议关闭与进程收尾
> （`finish_stop()`）在**锁外**执行；任务命令的判定与发送同在这一把锁内（`EngineBridge::rpc`/`with_channel`），
> 因此要么在关闭前送达、要么在锁内立即得到 `engine_shutting_down`（“未发送”），不存在“检查通过后被关闭抢先”的窗口。
> 确定性同步点测试把取消停在“已进入发送路径、尚未取得 supervisor 锁”，让关闭先完成 `begin_stop()`，取消仍 <1.5 s
> 返回明确结论且**未送达**引擎；变异验证（把耗时关闭放回锁内）令该测试以「等了 8.042621669s」失败。完整回归见
> [证据索引](tests/verification/desktop/evidence/od-arch-024/README.md)「第三轮」。

> 继续完成 OD-ARCH-024 尚未通过的并发验收，不要扩展产品功能。先记录 HEAD 与完整工作区状态并保留全部改动。只检查相关 Rust bridge、shutdown、任务状态/结果/取消实现及 OD-ARCH-024 证据。当前缺口：任务命令先检查关闭标志，再获取 supervisor 锁；关闭流程可能在两者之间先取得该锁，使请求排队到关闭之后。
>
> 分成三项集中完成：(1) 用确定性测试同步点卡在“关闭标志检查通过、请求尚未取得 supervisor 锁”之间，并建立真实合成在途任务；令关闭流程先取得 supervisor 锁，验证 status/result/cancel 的延迟、是否送达和错误语义。(2) 将任务命令与关闭操作整理成可线性化的最小并发边界：请求要么在关闭前送达，要么快速明确返回“关闭中、未发送”，不能等关闭结束后才普通失败。不得改 Runtime、外部协议、路由或凭证；若需要扩大架构则停止并报告。(3) 留存可复现证据，运行完整 Rust 离线测试、前端构建、桌面链/取消/草稿/会话回归、路由自测、`git diff --check HEAD` 和 `runtime/` 无差异检查；更新 STATUS.md、PLANS.md 与单一证据索引，完成后停止等待 ARCHITECT 复核，不提交、不推送。

**交给 OpenCode 的执行说明（由用户在批准后转发）：**

> 执行 OD-ARCH-024。先记录 HEAD 与完整工作区状态并保留全部差异。先阅读 AGENTS.md、本计划 OD-ARCH-024、REVIEW.md 顶部 OD-ARCH-023 结论以及相关 Rust 源码；不要全仓库扫描。修复 with_engine_call / with_call_gate 的生命周期锁自锁，不能牺牲权限世代检查；为状态、结果、取消命令写明并落实权限变更和关闭期间的并发规则，特别确保取消不会被不合理地堵塞。用宿主包装器和假 bridge/引擎做确定性测试，覆盖成功、拒绝、权限撤销竞争、停止失败与恢复，以及状态/结果/取消行为；不能只加源码字符串测试或生命周期纯单测。再跑计划要求的 Rust 离线测试和相关前端、路由、桌面、取消、草稿、会话回归、diff 检查及 runtime/ 无差异检查。每项结果记录实际命令、精确通过数和证据文件，并区分从当前源码新编译的测试与旧预编译二进制。若 Rust 编译受系统依赖限制，如实记录，不提权、不安装系统依赖。更新 STATUS.md、PLANS.md 和单一证据索引，完成后停止等待 ARCHITECT 复核；不改 Runtime/凭证/模型路由/真实材料，不提交或推送。

### OD-ARCH-023：修复模型权限变更与引擎启动之间的并发竞态

- **状态：BUILDER 已执行；ARCHITECT 独立复核不通过；修复已由 OD-ARCH-024 完成，待复核。** 本条保留为执行范围记录；问题证据见 REVIEW.md 顶部，修复见本文件顶部 OD-ARCH-024。
- **目标：** 确保权限设置更新、准入判定和引擎生命周期操作有一致的并发边界；撤权开始后，任何拿着旧准入结果的命令都不能重新启动引擎或接收新请求。
- **影响范围：** 仅桌面 Rust 宿主的模型权限变更、准入与引擎启动入口、相关并发测试、OD-ARCH-022 证据/共享状态记录。不得修改 `runtime/`、模型路由、凭证、真实材料或第二 worktree；不得提交、推送或打标签。
- **实施步骤：** (1) 盘点所有会初始化/触达引擎的宿主命令，覆盖任务提交、描述、会话列表/恢复、任务台账/恢复；健康状态等只读快照单独确认不会启动进程。 (2) 为授权设置变更与所有引擎启动/请求建立可证明的串行化边界；准入结果必须在同一生命周期保护下校验并使用，权限变更过程中先拒绝新的启动与提交，不能靠稍后设置屏障来补救。避免仅加 UI 锁或只在 `engine_submit_task` 检查屏障。 (3) 增加确定性并发回归：暂停某调用于准入完成之后，执行撤权并确认旧引擎已停，再释放该调用；证明旧 grant 不会启动引擎、创建会话或提交任务。分别覆盖提交和至少一个非提交启动入口，并覆盖停止失败时持续拒绝、恢复后才允许。 (4) 验证权限持久化失败、撤权、重新授权和并发多次切换的结果一致，错误状态不会假报成功。 (5) 运行 Rust 完整离线测试、前端构建、现有准入/桌面/会话回归、`git diff --check HEAD` 与 `runtime/` 无差异检查，更新证据和状态；若 Rust 工具链不可用，提供可复现的替代运行证据并明确独立验证限制。
- **验收标准：** 并发测试能稳定复现并覆盖旧 grant 时序；撤权操作完成后，任何在其开始前取得的 grant 都不能启动或调用引擎；停止失败期间所有会启动/提交模型工作的入口均被屏障拒绝，确认停止且完成恢复后才重新开放；整个计划要求的回归通过，既有工作区改动完整保留。
- **风险与停止条件：** 若要改 DSH Runtime、协议、模型路由或读取凭证才能建立边界，停止并提交架构证据，不扩大范围。若无法构造确定性并发测试，不得以“竞态很窄”或纯函数单测替代验收。
- **交付方式：** 一次集中完成修复、测试与证据；BUILDER 完成后停止，由 ARCHITECT 独立审查源码、差异和运行结果。

### OD-ARCH-022：端点去向说明、权限切换一致性与安装验收

- **状态：已执行；ARCHITECT 独立复核为部分通过，暂不验收。** 端点类别、分类测试、前端构建与隔离安装证据通过；权限切换存在并发竞态，后续修复见 OD-ARCH-023。结论见 [REVIEW.md](REVIEW.md)；执行事实见 [STATUS.md](STATUS.md) 与 [证据索引](tests/verification/desktop/evidence/od-arch-022/README.md)。
- **目标：** 修正模型端点类别的过度承诺；确保权限切换失败时不能让旧引擎继续处理新提交；完成隔离安装验证，让 OD-ARCH-021 达到可验收状态。
- **影响范围：** 仅主工作区现有路由分类、准入/设置、引擎停止状态、相关前端说明、测试、OD-021 证据索引和共享状态文件。不得改 `runtime/`、模型路由或凭证，不调用真实模型、不使用真实材料、不改第二工作区、不提交或推送。
- **实施步骤：** (1) 将端点事实与网络归属分开表达：回环可标为本机；非回环地址不得仅凭 RFC1918、链路本地、`.local` 或公网域名声称“单位内部”或确定数据是否离开单位。采用最小且诚实的类别/设置文案；无法确认组织归属或实际转发路径时明确标“无法确认”，仍由用户显式选择是否允许普通任务。 (2) 调整权限更新顺序与宿主状态机：只有旧引擎停止并确认不再接收任务后才报告变更生效；停止失败时拒绝新任务、显示明确失败状态，并提供可恢复路径，不返回假成功。覆盖启用、撤销、运行任务中变更及停止失败。 (3) 更新分类测试与证据说明，按脚本实际结果校准为 18 项；补充私有地址不等于单位网络的断言。 (4) 在临时隔离根/用户级可控位置完成 `.deb` 安装、启动和卸载验证；禁止系统级安装或权限提升，若环境不支持则停止并如实保留“未完成”。 (5) 运行计划要求的前端类型检查与构建、Rust 完整离线测试、相关 Python/路由/桌面回归、`git diff --check HEAD` 及 `runtime/` 无差异检查，集中更新状态与证据索引。
- **验收标准：** UI 和宿主不把网络地址特征说成单位授权或网络归属；遇到无法确认的数据路径时清楚告知；权限切换期间没有沿用旧路由处理新输入的路径，失败后保持拒绝并可恢复；测试覆盖成功与失败边界；OD-021 的分类用例数与实际一致；隔离安装后可从应用入口启动（若系统限制阻塞，单列未通过）；完整回归通过且既有工作区改动保留。
- **风险与停止条件：** 端点分类若需要读取凭证、修改 DSH 路由、Runtime 或读取用户数据才能完成，停止并报告；安装若需要管理员权限或系统级改动，停止该步骤；发现权限绕行、真实材料或秘密时停止相关操作。不得为追求通过而把地址猜测写成确定事实。
- **交付方式：** 由用户选择 BUILDER；一次集中完成本计划和定向修复。执行完成后停止，由 ARCHITECT 按实际源码、diff、测试和安装证据复核。

---


### OD-ARCH-021：可配置模型权限与 AI 任务闭环、桌面安装验收

- **状态：用户已批准；BUILDER 已执行（2026-10-18）；ARCHITECT 复核为部分通过，整体未验收。** HEAD 仍为 `5ffc710`，未提交、未推送、未打标签。执行记录见 [STATUS.md](STATUS.md) 顶部，复核见 [REVIEW.md](REVIEW.md) 顶部，证据见 [tests/verification/desktop/evidence/od-arch-021/README.md](tests/verification/desktop/evidence/od-arch-021/README.md)。
- **目标：** 首次严格关闭模型权限且明确告知数据去向，同时让用户能自行决定已配置模型是否可处理普通任务；以合成端点验证周报、文案和项目任务的 AI 提交到结果、编辑、人工确认和保存闭环，并完成 Linux 安装入口验收。
- **影响范围：** 主工作区现有设置页、准入判断、Rust 宿主和既有 DSH ACP 适配链、合成测试及打包验收。不得修改 `runtime/`、读取/打印/复制凭证、改变现有模型路由、接入真实工作材料、调用真实外部模型、修改第二 worktree、提交或推送。
- **实施步骤：** (1) 只读查明现有宿主能否从非秘密配置中可靠获知有效模型类别与数据去向，不读取凭证值。 (2) 若可可靠识别，为本机/外部/未知类别实现首次关闭、用户逐项开启与可撤销设置；界面说明内容是否离开本机/组织；宿主在任务提交时强制执行；未知类别默认拒绝。不得只信任用户任意填写的 provider 名称。 (3) 如端点实际类别无法可靠判断，停止该部分并提交证据与最小架构选择，不实现伪分类或旁路。 (4) 用合成端点从自有桌面界面验证三类任务的提交、成功、错误、取消、编辑、确认保存；验证首次拒绝、开启、撤销、未知端点及禁入任务拒绝，并证明拒绝不启动引擎或泄露输入。 (5) 构建并安装本机 `.deb` 到隔离或用户级可控环境，验证启动器名称、图标和启动；不改系统级配置。若必须提升权限或改系统依赖，停止并报告。 (6) 执行完整前端/Rust/Python相关回归、`git diff --check HEAD`、`runtime/` 无差异检查，保留合成证据并更新状态。
- **验收标准：** 首次模型权限明确关闭且设置可持久化、可撤销；用户能理解并选择外部数据发送风险；宿主执行与 UI 一致；未知端点及禁入范围拒绝；合成端点证明完整任务闭环；桌面包安装后可启动；计划内回归通过且既有工作区差异保留。真实模型质量、真实业务材料授权和跨平台仍单独标为未验证。
- **风险与停止条件：** 若端点类别/数据去向只能通过读取凭证、修改路由或 Runtime 才能判断，停止该部分并报告；若安装需要系统级依赖或权限，停止安装部分；发现真实材料、秘密或计划外高风险差异时停止相关操作。
- **交付方式：** BUILDER 集中交付实现、测试结果、单一证据索引与遗留限制，之后停止等待 ARCHITECT 复核。

> OD-ARCH-021 已由 ARCHITECT 部分通过复核；整体验收待 OD-ARCH-022 修复并补齐安装证据。OD-ARCH-020 已由 ARCHITECT 部分通过：发行包产出、成果目录设置与离线保存/重开通过；模型权限和 AI 任务闭环已有合成端点证据，限制见 REVIEW.md。

## OD-ARCH-020 执行计划（已复核，部分通过）

### OD-ARCH-020：个人试用版闭环——安装入口、成果目录设置与 AI 工作流程（已部分复核）

- **状态：用户已批准；BUILDER 已执行；ARCHITECT 于 2026-10-18 部分通过复核。** 当前 HEAD 为 `5ffc710`，未提交、未推送、未打标签。主工作区原有未提交/未跟踪改动全部保留；`/mnt/data/Code/omniharness-ui` worktree 未改动、未合并。AI 准入与真实任务闭环未完成，后续列入 OD-ARCH-021。详情见 [REVIEW.md](REVIEW.md) 顶部。
- **实际执行结果（摘要，详细证据见 [STATUS.md](STATUS.md) 顶部与 [证据索引](tests/verification/desktop/evidence/od-arch-020/README.md)）：** Linux `.deb` 已产出（`omni-desk` 0.1.0），AppImage 因 `linuxdeploy` 失败未产出，`.deb` 未安装；设置页可用路径输入框选择成果目录，重启后保留，非法路径被拒绝，既有文件哈希不变；同标题再次保存按秒级时间戳新建文件而不覆盖；三个离线场景的人工确认与保存、只读重开已实测；AI 初稿在准入关闭时被宿主拒绝且未发送到任何模型；使用记录只含非正文字段。**未完成/未验证：** 真实模型提交、生成、取消与质量（准入仍为 `false`），原生目录选择器，AppImage，`.deb` 安装后启动，Windows/macOS。
- **分类：** 产品化功能与端到端工作流程。
- **目标：** 用户能从可识别的桌面入口启动 OmniDesk；在设置中选择并更改新成果保存目录；周报、一般材料写作、项目管理从现有独立界面进入 AI 协助流程，能提交任务、查看结果、继续修改、人工复核并保存。完成后再由用户评估实际工作体验，而非承担技术验收。
- **影响范围：** 以 `/mnt/data/Code/MyWorkStation` 为唯一实施目标，只改桌面前端、Rust 宿主/现有适配器必要接口、打包配置、合成样例和定向验证及共享状态文件。UI worktree 只读参考；按文件审查后择取必要前端设计，不整支合并。不得修改 `runtime/`、DSH 私有数据格式、现有用户数据、凭证或模型路由；不得提交、推送、打标签、安装系统级软件或使用真实工作材料。
- **实施步骤：**
  1. 复核并记录两工作区 HEAD、状态和差异边界；确认当前桌面入口、任务提交/取消/错误处理、保存路径和准入检查。以主工作区实际代码为准，先做短清单，不重扫无关文档。
  2. 补齐桌面发行入口：构建本机 Linux 可安装包，核对包内图标、应用名、启动器条目和启动命令；提供可复现的构建与安装说明。只产出安装包，不替用户安装，不改桌面或系统配置。
  3. 增加应用设置中的成果目录选择和当前路径显示。选择路径时继续拒绝仓库内、DSH/Agent/Codex 私有目录及不安全软链接；检查目录可写时使用一次性探针并清理。更改只影响之后新建的成果，清楚提示旧目录文件不会自动移动、覆盖或删除；提供打开当前目录的入口。应用自身配置目录与成果目录分开。
  4. 将三个优先场景接入一致的 AI 任务体验：表单收集用途/受众/素材，调用现有引擎适配链路，展示生成过程与结果，支持取消、失败重试、人工编辑/复核和保存；提示词、写作规则使用现有可编辑流程包，语言工作交给模型，日期/统计等确定性计算留在代码。不得新增通信层或多 Agent 平台。
  5. **客户可配置的模型准入。** 首次使用采用严格默认：本机和外部模型都不自动获得工作材料处理权限；用户必须显式选择端点类型、可处理的任务/材料范围和确认方式。设置页以通俗语言说明本机、单位内部和外部服务的差别，以及内容是否可能离开设备/单位环境；外部模型默认关闭，开启前由用户确认其授权与后果。用户可在已获授权或理解风险后自行放宽普通工作场景的设置，软件不得把外部模型永久一刀切禁用。端点类型未知时默认拒绝；案件、线索、人物画像及明确禁入内容仍按当前产品边界拒绝，除非未来用户正式调整产品目标并另行审查。端点身份不能只凭用户填写的名称判断；以实际 provider/endpoint 配置来源进行分类，不能确认时显示“未知”。设置与授权确认可在本地留最小审计记录，不保存提示词正文或凭证；准入由宿主执行，前端负责解释和交互。
  6. **真实模型测试边界。** BUILDER 不读取或写入凭证，不擅自修改现存模型路由。先用合成端点验证 UI 到适配层的请求、结果、取消和错误链路，并验证严格默认、用户逐项开启、关闭/撤销、未知端点拒绝及禁入范围拒绝。产品实现可支持客户自行配置获准端点；真实模型质量测试仅在用户指定端点、用途和环境并明确授权后执行。在测试前不得称 AI 写作已通过质量验证。
  7. 以虚构内容进行完整桌面纵向验证：安装包信息与启动入口、保存目录选择/重启后保留/拒绝非法路径、三场景任务提交与取消/错误/结果编辑/保存、未知端点和禁入任务默认拒绝。跑计划要求的前端构建、Rust 完整测试、Python 适配器定向回归、`git diff --check HEAD` 和 `runtime/` 无差异检查，证据不含私人路径、会话正文或凭证。
- **验收标准：** (1) 本机发行包包含 OmniDesk 桌面启动器与图标，构建和安装说明可复现；(2) 用户可在应用内改成果目录，设置重启后仍生效，非法/越界路径被拒绝，既有文件不被迁移或删除；(3) 三个场景走同一套清楚的 AI 任务交互，提交、取消、错误、编辑和保存均有真实窗口证据；(4) 宿主端按模型类型与任务范围执行准入，未知端点、外部端点未确认及禁入任务均被拒绝；(5) 所有测试通过且 `runtime/`、模型路由、凭证和用户数据无变化；(6) 真实模型调用与输出质量需在独立授权后验证，未验证前不得宣称 AI 功能完成或交付实际材料测试。ARCHITECT 独立 Review 通过后，再交用户做应用体验反馈。
- **风险与停止条件：** 桌面打包或目录选择可能需要新依赖；如需安装系统依赖或改系统配置，停止并报告；如果现有准入关闭使真实任务提交无法运行，保留关闭状态并用合成替身验证，不设隐藏旁路；若实现需改 Runtime、DSH 私有会话格式、触碰凭证/真实材料或覆盖任一工作区现存差异，立即停止相关部分。
- **交付方式：** 一次完成范围内的应用闭环和回归，集中提交差异清单、构建/测试命令、桌面证据和未验证项；不拆成多个微任务。完成后停止等 ARCHITECT 独立审查，不自行合并或提交。

## 最近复核完成

### OD-ARCH-019：可编辑离线流程包与轻量个人试用记录

- **状态：BUILDER 于 2026-10-18 执行；ARCHITECT 有条件通过复核。** 个人可开始用合成或公开材料试用离线步骤。真实模型写作、真实工作材料、跨平台和长期使用均未验证；未提交、未推送、未打标签。执行记录见 [STATUS.md](STATUS.md) 顶部，独立审查见 [REVIEW.md](REVIEW.md) 顶部，证据见 [tests/verification/desktop/evidence/od-arch-019/README.md](tests/verification/desktop/evidence/od-arch-019/README.md)。
- **分类：** 个人试用能力与本地数据最小化。
- **目标：** 让作者能根据实际使用调整周报、一般文案结构和项目跟进的离线流程内容，无需改代码或重新编译；以不收集正文的方式记录使用时长、返工和可用性，便于后续决定是否值得接入模型能力。
- **影响范围：** 当前主工作区的 `desktop/`、Rust 桌面宿主、必要的仓库内默认流程包与合成验证、桌面定向测试，以及 `PLANS.md`、`STATUS.md`、`REVIEW.md`；若形成新架构决定，再由 ARCHITECT 更新 `docs/decisions.md`。实施前须先核对现有数据目录和使用记录格式，优先复用现有目录。不得改 `runtime/`、第二 worktree、模型路由/准入、凭证、DSH 用户数据或已有用户成果；不得使用真实工作材料、调用外部模型、提交/推送/打标签。
- **实施步骤：**
  1. 记录 HEAD、完整工作区状态和暂存区；只检查当前离线流程的输入、计算、渲染、配置及数据落点，区分固定计算逻辑与可编辑文案，不重扫无关目录。
  2. 定义一个小型、带版本的声明式流程包，承载现有可配置内容（名称、字段说明、输出标题/章节等）；放在仓库外现有 OmniDesk 配置目录，默认文件可从仓库内合成样例初始化。首次创建后不覆盖用户修改。提供显式重载；格式错误时给出可读错误并拒绝该配置，不静默回退成看似成功。
  3. 让三个离线流程读取该配置并在界面和结果中反映修改；日期校验、状态计数、逾期判定等确定性计算仍由代码完成。配置只允许数据和文本，不允许运行脚本、加载外链、改变路径权限或触发模型调用；页面继续明确标注“不调用模型”。若实现需要通用工作流引擎、任意脚本执行或新增通信层，停止并报告。
  4. 增加明确可选的使用记录：仅记录场景枚举、开始/结束时间或用户确认的耗时、结果可用性枚举、返工次数和是否愿意再用；不得记录输入/输出正文、自由文本评论、标题、会话标识、路径、模型或凭证。记录留在仓库外本地目录，提供查看摘要和清除入口；不自动上传或同步。
  5. 用合成数据验证三场景：仅改流程包并重载即可改变可见流程/输出；未知版本、缺项和格式错误被安全拒绝；使用记录字段严格白名单且可清除；主计算结果与既有测试一致。完成一次隔离桌面窗口验收，并运行计划要求的完整前端、Rust、Python相关回归、`git diff --check HEAD` 和 `runtime/` 差异检查。保存脱敏证据后停止交 ARCHITECT 复核。
- **验收标准：** (1) 三个流程的规定可编辑内容可在不改源码、不重编译的情况下生效；(2) 配置不执行代码、不调用模型，非法配置不会静默产生误导结果；(3) 确定性计算、人工确认、成果保存/重开行为不回退；(4) 试用记录只含列出的非正文字段，默认可不记录，能本地查看与清除；(5) 桌面交互与计划要求的完整回归有可复核证据；(6) `runtime/`、端点准入、凭证、用户数据无变化，保留所有既有工作区差异，不提交或推送。
- **风险与限制：** 可编辑配置仅能调整已实现的离线流程表达，不能被描述为 AI Skill 执行或 AI 写作；使用时长由个人确认时可能不精确；试用元数据本身仍是本地记录，需可查看、清除且不入 Git。会话页因端点准入关闭仍不可用，旧 DSH 会话导入仍不支持，本计划不改变这两项边界。
- **停止条件：** 发现配置会触发模型/外部工具、需扩大授权、需要访问真实材料或旧会话、需修改 Runtime/其他工作区，或发现疑似凭证/真实业务内容进入仓库时，停止相关部分并报告；不通过放宽边界继续。
- **集中交付：** BUILDER 一次性交付代码差异、使用说明、单一证据索引和完整测试结果；不拆成多个小任务。ARCHITECT 独立复核前，不宣称该能力已实现或验收。

## 最近完成并复核

### OD-ARCH-018：个人试用准备与桌面交互验收

- **状态：BUILDER 已执行；ARCHITECT 于 2026-10-18 有条件通过复核。** 三个离线流程在 Linux 虚拟桌面完成保存与重开；Rust 92/92 与窗口证据由 BUILDER 留存，ARCHITECT 因环境无 `cargo` 未能独立重跑 Rust。真实模型生成、真实窗口取消、真实工作材料、跨平台和长期使用均未验证。未提交、未推送、未打标签。执行记录见 [STATUS.md](STATUS.md) 顶部，独立审查见 [REVIEW.md](REVIEW.md) 顶部，证据索引与截图见 [tests/verification/desktop/evidence/od-arch-018/README.md](tests/verification/desktop/evidence/od-arch-018/README.md)。
- **分类：** 个人工作台可用性、安全边界收口与合成/公开材料场景验收。
- **目标：** 让用户能在自己的桌面界面中安全地试走三个优先场景；不把离线流程称为 AI 写作，不开放模型或真实工作材料；收紧会话与任务列表显示的元数据，并验证取消交互的完整状态。
- **影响范围：** 仅当前主工作区的 `desktop/`、`engine/omnidesk_engine/` 必要字段、相关定向测试、合成样例与 `tests/verification/desktop/evidence/od-arch-018/`，以及 `STATUS.md`、本计划和 `REVIEW.md`。保留所有现有工作区改动。不得修改 `runtime/`、其他 worktree、模型路由/准入常量/凭证、DSH 用户数据或旧会话；不得接入真实工作材料、调用外部模型、提交/推送/打标签，也不得安装系统依赖或更改用户环境。
- **实施步骤：**
  1. **最小化会话/任务信息。** 会话列表默认仅显示稳定标识和不含绝对路径的工作区名称；任务台账不向界面返回或显示自由文本 `intent`。恢复所需的内部路径仍由受控 IPC 使用，不进入界面文案、错误提示或证据。用合成敏感哨兵验证 API 响应、可见界面与日志/证据均不泄露完整路径、项目名或 intent。
  2. **验证取消按钮交互。** 以可控的前端宿主替身覆盖首次取消、等待期间重复点击、超时后重试、任务先完成/失败、取消确认与取消未确认；断言每个阶段实际发出的 IPC 次数和用户看到的状态。不得只验证状态纯函数，也不得启动已准入关闭的真实模型链路。
  3. **准备三个可复用桌面试用样例。** 为周记录汇总/周报、一般文案、轻量项目管理各准备一组虚构或公开输入；确认用户可从自有界面找到入口、完成离线流程、人工编辑并将成果保存到统一的仓库外成果目录。若现有流程只是模板/确定性整理，应在界面明确标注“不调用模型”，不得暗示具备生成式写作或真实工作能力。
  4. **一次性桌面纵向验收。** 在本机隔离的合成数据根中实际打开桌面窗口，完成以上三个场景、保存与重新打开成果，并检查取消交互及会话页的隐私显示。只要桌面构建受当前系统依赖限制，就记录具体限制、可复现命令和未完成项；不得用浏览器构建或脚本通过冒充窗口验收。
  5. **集中回归与交接。** 执行相关前端构建/测试、Rust 与 Python 定向回归、三个场景的端到端检查、`git diff --check HEAD` 与 `runtime/` 差异检查；确认准入仍关闭、数据均在仓库外且仓库没有合成正文残留。统一更新状态与证据索引，停止等待 ARCHITECT 复核。
- **验收标准：** (1) 会话页不展示绝对工作区路径，任务页不展示自由文本 intent，敏感哨兵在响应/UI/证据中不可见；(2) 取消状态的实际 IPC 次数符合“等待中不重复、明确超时才可重试”；(3) 三个场景均从桌面窗口完成输入、编辑、保存、重新打开，产物位于仓库外统一目录，且界面如实说明离线能力；(4) 计划要求的测试通过，或将环境限制明确记录为未验收，不得降格替代；(5) `runtime/`、准入常量、凭证和用户数据无变化，不提交、不推送。
- **风险与限制：** 当前执行环境此前缺少 GTK/GDK/Cairo/Pango/ATK 开发库，因此真实窗口构建可能仍受阻；未解决该环境阻塞前，不得宣称桌面可用。三个离线样例只能验证流程、保存和交互，不能证明模型写作质量、实际节省工时、单位授权或真实工作适用性。旧 DSH 会话导入仍不支持，不在本计划内自行开发格式转换。
- **停止条件：** 若测试必须读取真实会话/工作材料、连接外部端点、放宽准入或修改 Runtime，立即停止该部分并报告；其余隔离合成检查可完成。
- **BUILDER 完成后交付：** 一份集中执行记录、一个证据索引、实际差异及完整测试结果；不拆成多个小任务交接。完成后由 ARCHITECT 基于源码、差异和可复核结果独立审查。

**执行记录（BUILDER；独立复核结论见 REVIEW.md）：** 三个离线场景均在隔离合成数据根的真实 Tauri 窗口中完成输入、本地处理、人工确认、保存与重新打开；成果落盘在仓库外，界面明确标注“不调用模型”。会话页与任务页在准入关闭时只显示拒绝原因，未显示绝对路径、自由文本 intent 或正文。实测中修复了一个真实缺陷：成果列表原先因文件名解析错误恒为空，现已能列出并重开全部成果。`cargo test --offline` 92/92、前端类型检查与构建通过、会话连续性 37/37、取消状态与 IPC 34/34，`runtime/` 无差异、两侧准入常量为 `false`。取消按钮的真实窗口点击与真实模型生成仍为未验证项。

## 最近完成计划

### OD-ARCH-017：会话连续性纵向能力（已执行；ARCHITECT 有条件通过复核）

- **状态：已执行并经 ARCHITECT 有条件通过（2026-10-18）。** 合成适配层及前端构建通过；真实桌面窗口交互与完整 Rust 独立复验未完成。旧 DSH 会话导入因无公开导入接口而不支持。独立审查见 [REVIEW.md](REVIEW.md) 顶部；执行记录见 [STATUS.md](STATUS.md) 顶部；证据见 [tests/verification/desktop/evidence/od-arch-017/README.md](tests/verification/desktop/evidence/od-arch-017/README.md)。未提交、未推送、未打标签。
- **分类：** 桌面工作台会话/任务生命周期功能。
- **目标：** 让用户能在 OmniDesk 中找到并重新打开 DSH 已持久化会话；在隔离数据根中显式导入用户选定的旧 DSH 会话；重启后如实区分已完成、未完成和状态未知的任务；修正取消中重复发送问题。复用 DSH 原生持久化与 ACP，不另存一份逐消息正文日志，不自动重放中断任务。
- **影响范围：** 当前主工作区 `desktop/`、`engine/omnidesk_engine/` 的适配层、相关契约/测试、专项合成证据及 `STATUS.md`；由 ARCHITECT 更新 `REVIEW.md`。不得修改 `runtime/`、DSH 原生数据格式、模型端点/路由/凭证、第二 worktree、真实用户数据或现存旧会话；不得把会话正文、真实路径或凭证写入 Git、报告或通用日志；不得提交、推送、打标签。会话数据只在一次性仓库外合成根中验证，旧数据源只读。
- **实施步骤：**
  1. **一次性确认接口与数据路径。** BUILDER 只查相关锁定版 DSH ACP/持久化入口、当前 Adapter 与 Tauri 调用链，确定 `session/list`、`session/resume` 和旧数据导入可用的原生路径；不得遍历用户数据或读取真实会话正文。沿用已确定决策：原生 DSH 会话是持久数据，禁止擅自删除、覆盖；不新建重复正文日志。
  2. **会话清单与重新打开。** 在 OmniDesk 界面显示会话必要元数据并按规范化工作区归组；可重开既有会话。列表遗漏、工作区路径别名、会话不可恢复和无效/不兼容记录须有明确状态；只读清单不应擅自修改来源文件或注册表。
  3. **旧 DSH 会话导入。** 提供由用户明确选择来源和目标的导入路径，先预览可导入元数据、版本兼容及冲突，再经明确确认导入。导入须保留原始来源只读、避免覆盖目标、支持重复导入检测，并记录可核验的数量/完整性结果，不把会话正文复制进仓库或模型。优先使用锁定版 DSH 已支持的机制；如果只能自行复制/改写其发布格式、需要改 `runtime/`，或无法保证来源不变与目标可恢复，则停止导入实现，提交兼容性结论，不自行发明迁移格式。
  4. **重启状态与取消闭环。** 从已有 OmniDesk 任务记录恢复可证实的终态；无终态证据的任务标为 `unknown/interrupted`，允许用户打开其原会话查看，但不自动重放或声称续跑。取消按钮仅在首次请求前、或明确超时后允许重试；请求已发出但仍在取消中的任务不得再次发送。用确定性宿主替身或隔离合成 ACP 端点验证取消请求次数、终态竞争、超时重试和重启状态。
  5. **完整纵向验证。** 在一次性仓库外合成数据根中，从会话创建、写入、列表、关闭、重启恢复、旧会话导入到再次打开完整走通；覆盖重复/冲突/不兼容/损坏元数据和中断任务。验证来源未变、目标不被覆盖、仓库无会话正文、端点准入仍关闭。执行前端构建、完整 Rust/Python 相关回归及计划内桌面交互；保存不含真实正文或私人路径的证据后停止，等待 ARCHITECT 审查。
- **验收标准：** (1) 合成会话可在 OmniDesk 列出并按工作区找回；(2) 选定旧 DSH 合成会话可按支持的原生机制导入、重开，来源保持只读且哈希/清单未变；重复导入不覆盖、不产生含糊副本；(3) 不兼容或不确定内容明确拒绝并说明原因；(4) 重启后任务状态可信，未知状态不自动重放；(5) 取消等待期间只发一次请求，只有明确超时后可重试；(6) 前端、宿主、适配层及相关回归通过，隔离桌面关键路径有运行证据；`runtime/`、模型路由和用户数据无差异；无真实会话或正文进入 Git；未提交、未推送。
- **风险：** DSH ACP 列表与本地持久化、workspace 索引可能有过滤差异；会话元数据中的路径和标题本身也可能敏感；旧会话格式虽为发布格式，跨版本导入未必受支持；重开会话不代表中断任务续跑。报告仅记录脱敏后的计数与结果。
- **停止条件：** 若必须修改 DSH Runtime/格式、导入要求读入或发出真实会话正文、无法在仓库外隔离数据、来源可能被修改/覆盖、模型端点或凭证被触达，立即停止相关部分并报告；其余独立合成部分可完成。不得以范围扩大或放松安全条件来达成验收。
- **BUILDER 完成后交付：** 一份集中状态记录、一个统一证据索引、实际差异和全部测试结果；不分多个小交付，不在中途要求逐点 ARCHITECT 复核。遇硬停止条件才提前交接。

## 最近完成计划

### OD-ARCH-010：封住当前工作区准入缺口，并启动优先工作场景的离线验证

- **状态：已执行并完成 ARCHITECT 独立验收（2026-10-18），仅离线合成流程按范围通过。** 用户在本轮明确下达「OD-ARCH-010 授权执行」。执行记录见 [STATUS.md](STATUS.md) 顶部，证据见 [tests/verification/desktop/evidence/od-arch-010/README.md](tests/verification/desktop/evidence/od-arch-010/README.md)；真实业务与模型使用仍未验收。
- **分类：** 安全边界修复 + 个人工作场景流程验证。
- **目标：** 先在用户指定的唯一工作区确认并封住引擎描述、任务提交、目录准备和直接引擎启动路径的宿主拒绝入口；随后仅用合成/公开材料验证周记录与周报、一般文案撰写、轻量项目管理三个场景的本地流程。复杂数据分析暂缓，不作为本阶段重点。
- **影响范围：** 只允许在用户指定的单一工作区修改当前桌面宿主、必要的准入测试、场景流程和合成样例，以及 STATUS.md、证据材料。不得复制或整体合并另一 worktree 的改动；不得修改 runtime/、端点常量、模型路由、凭证、真实业务材料或既有用户数据；不得调用云端/单位模型、解除准入关闭、提交/推送/打标签或启动用户正在使用的实例。
- **实施步骤：**
  1. 执行前记录唯一目标工作区的 HEAD、状态与差异清单；确认其与另一工作区差异不被覆盖。只核对所需规则、入口源码和相关测试。
  2. 通过宿主代码在任何引擎子进程启动、描述、提交任务、创建工作目录之前统一拒绝；测试直接 Tauri 命令入口和底层启动入口，拒绝路径不能创建目录、进程或任务。保持端点准入关闭，不新增可配置旁路。
  3. 做三个离线合成场景的最小纵向验证：每日事项记录汇总为周报；按用途/受众要求组织一般文案并标注缺失信息；轻量项目管理包含事项、负责人代号、期限、状态和下一步。不得将真实业务内容用于夹具。
  4. 在端点闸门保持关闭时，只验证本地表单、流程状态、确定性校验和本地成果保存；模型撰写步骤标为“受准入阻塞、未验证”，不得声称已完成真实生成或真实模型质量评估。
  5. 复杂数据分析只登记为后续候选，不新增功能、不扩大评测；记录场景测试是否能开始、各自阻塞点和下一步需用户提供的条件。
  6. 更新 STATUS.md 与证据索引；由 ARCHITECT 按实际差异和测试独立审查后决定是否进入用户个人试用。
- **验收标准：** 唯一工作区身份与改动边界清楚；准入关闭时所有宿主启动/提交入口都 fail-closed 且有定向测试；runtime/、模型路由和凭证未改；三个合成场景可重复走完已实现的本地步骤，模型步骤明确标未验证；复杂分析未扩围；证据区分静态检查、离线运行和模型未验证；未提交、未推送。
- **风险：** 两个 worktree 均有大量不同未提交改动，误选或整体复制可能覆盖工作；闸门保持关闭会阻止模型生成，因此本阶段只能完成无模型的流程测试。用户转述的授权仅限特定条件下的个人数据分析与生态研判，不能推定周报、一般写作、项目管理或某个模型端点已获准。
- **停止条件：** 若无法指定唯一工作区、发现未覆盖的子进程启动入口或有任何未经允许的真实材料/端点调用路径，停止相关实现并报告，不得放开闸门。

**已完成并独立验收。** 模型调用与真实业务材料准入保持关闭。

## 最近完成计划

### OD-ARCH-012：补齐成果写入失败清理验证，关闭 OD-ARCH-011 验收

- **状态：已执行并经 ARCHITECT 独立验收通过（2026-10-18）。** 定向测试 45/0，完整 Rust 回归 lib 72 / host_integration 13 / e2e 3 全通过；前端构建与离线示例通过，`runtime/` 无差异。两次受控变异证明失败测试有效，源码已还原。证据见 `tests/verification/desktop/evidence/od-arch-012/`。未提交、未推送，不构成合并授权。
- **分类：** 缺失验收证据补齐与范围性复验。
- **目标：** 仅补足 OD-ARCH-011 要求的写入失败和刷盘失败清理证据，并完成该计划要求的回归；不顺带审计或合并其他 worktree，不实现会话恢复、旧会话导入或 Skill 加载。
- **影响范围：** 仅限当前主工作区成果写入器及其 Rust 测试、必要的状态/证据记录。不得读取旧会话或旧成果正文，回显凭证，提交/推送/打标签，修改 `runtime/`、模型路由或凭证，启动用户实例，或改动第二工作区。
- **实施步骤：**
  1. BUILDER 为保存写入器补最小测试专用故障注入，分别验证写入失败与刷盘失败；注入不得从 UI、环境变量、普通配置或生产请求触发。检查本次新文件清理结果、既有同名文件未改动、错误不含内容。
  2. 运行完整 Rust 离线测试、前端构建、离线场景示例、差异空白检查及 `runtime/` 无差异检查；任一失败则记录并停止，不扩大修复范围。
  3. BUILDER 仅更新 `STATUS.md` 和本计划相关证据索引；完成后停止，由 ARCHITECT 独立检查源码、测试和实际差异，再决定是否在 `REVIEW.md` 关闭 OD-ARCH-011。
- **验收标准：** 写入失败与刷盘失败各有可重复测试；失败时只清理由本次新建的半成品，既有文件不变；测试注入不能由 UI、环境变量或普通生产配置触发；计划要求的 Rust、前端和离线流程回归通过；`runtime/` 无差异；模型准入继续关闭；未提交、未推送。
- **风险：** 测试注入若进入生产可触发路径会制造绕行；写入失败后的文件删除本身也可能失败，测试和报告必须如实区分“清理成功”与“清理失败”，不得承诺超出文件系统保证的原子性。
- **停止条件：** 若需修改生产配置才能注入故障、发现疑似真实材料/凭证或准入绕行，立即停止相关步骤并报告；不得通过放宽准入、读取用户数据或修改 `runtime/` 完成测试。

**OD-ARCH-012 已关闭。双工作区保护盘点与 UI 评审尚待单独制定并获用户批准；本计划未授权审计第二工作区、合并、提交或推送。**

## 当前计划状态

### OD-ARCH-014：以主工作区为基线选择性整合 UI 外壳（已执行；有条件通过）

- **OD-ARCH-013 状态：** 只读审计已由 BUILDER 执行，并经 ARCHITECT 独立复核按范围通过（2026-10-18）。两边改动保持原样；整分支合并不通过。审查结论见 [REVIEW.md](REVIEW.md) 顶部，执行事实见 [STATUS.md](STATUS.md) 顶部。
- **分类：** 前端整合与产品体验验证。
- **状态：已执行；ARCHITECT 独立复核有条件通过（2026-10-18）。** 结论与执行偏差见 [REVIEW.md](REVIEW.md) 顶部；执行事实见 [STATUS.md](STATUS.md) 顶部。未提交、未推送、未打标签。
- **目标：** 保留主工作区安全闸门、离线场景、成果保存和现有宿主行为，选择性接入 UI worktree 的视觉外壳，使用户能在同一桌面窗口进入首页、任务/分析工作台、离线周报与项目流程、只读引擎诊断，并用构建、测试和窗口交互证明没有功能回退。
- **影响范围：** 仅在 `/mnt/data/Code/MyWorkStation` 主工作区实施；UI worktree 只读，作为文件来源参考。允许改动主工作区 `desktop/` 前端、锁文件、必要启动器和 UI 专项测试/证据，以及 `STATUS.md`、`REVIEW.md`。默认可引入 `theme.css`、`workbench-ui.css`、`components/`、`shell/` 中与现有视图相容的组件、自托管字体依赖和 Vite Tailwind 插件、前端统一提交提示。不得整体覆盖主工作区 `App.tsx`、`api.ts`、`types.ts`、`workbench.tsx`、Rust 宿主、Python 适配层、离线流程或成果保存实现。不得改 UI worktree、`runtime/`、准入常量、模型路由、凭证、真实材料或旧用户数据；不得提交、推送、打标签。
- **实施步骤：**
  1. 记录两工作区执行前 HEAD、状态和计划文件清单；确认交接文档已校正为旧后端基线。逐个选取文件，不做分支整体合并。
  2. 在主工作区接入视觉令牌、兼容组件、字体和构建插件，保留主工作区现有 IPC 契约与页面逻辑；不接入第二套首页引擎对话，避免重复维护任务提交、取消和终态状态。
  3. 接入新导航与视觉外壳，并保留任务工作台、分析工作台、离线流程和只读技术诊断入口；接入前端准入提示，宿主拒绝仍为最终边界。
  4. 比较既有启动器和 UI worktree 的 `scripts/dev-ui.sh`，只吸收经核验的隔离检查；明确唯一开发入口，不保留可能串载其他 worktree 页面或误停其他进程的路径。
  5. 保持端点准入关闭，运行完整前端构建、Rust 离线回归、离线流程评估，检查 `runtime/` 无差异和旧成果未受影响。
  6. 在隔离环境启动真实 Tauri 窗口，验证导航、离线周报/文案/项目流程、手动确认与保存、引擎拒绝提示；将截图和结果存入专项证据。BUILDER 完成后停止，等待 ARCHITECT 基于实际 diff 和运行证据审查。
- **验收标准：** 主工作区前端构建、Rust 测试、离线场景评估通过；窗口关键路径可交互；首页、任务、分析、离线流程和诊断入口均可到达且未丢失既有功能；引擎准入仍 fail-closed，界面拒绝与宿主一致；启动器不误停其他实例；`runtime/` 无差异；原有改动未被覆盖；仅用合成/公开数据；未提交、未推送。缺少运行证据的项目不得标通过。
- **风险：** 两边多个前端和宿主文件已分叉，整段搬运会回退主工作区修复；Tailwind 级联层可能影响旧样式；新增依赖可能需要在线安装；当前窗口证据仅适用于本机平台。
- **停止条件：** 交接文档仍误称已包含主工作区成果；出现准入放宽、离线保存逻辑丢失或端点配置变化；需改 runtime、路由或真实数据；启动器可能终止非本次启动的进程。停止并报告，不自行扩大范围。

### OD-ARCH-015：核对 UI 整合边界并收口共享状态（已独立复核通过）

- **状态：只读核对按范围通过（2026-10-18）。** 未改应用代码、UI 工作区、`runtime/`、用户数据或证据原件；未提交、推送或打标签。BUILDER 执行事实见 [STATUS.md](STATUS.md)；独立审查及限制见 [REVIEW.md](REVIEW.md)。取消按钮缺陷未在本计划内修复。

- **分类：** 只读审查与计划状态校准。
- **目标：** 不改业务代码，基于可用的 OD-BUILD-005 证据、当前源码与 OD-ARCH-014 执行记录，核对 `workbench.tsx` 引擎支路与既有手动/保存流程的边界；如缺少执行前快照，明确标为无法追溯，不猜测。完成 014 的偏差记录与共享状态校准，为之后安排用户合成数据试用或会话持久化工作提供可信基线。
- **影响范围：** 只读检查主工作区相关代码、当前 Git 差异、OD-BUILD-005 与 OD-ARCH-014 证据；只允许更新 `PLANS.md`、`REVIEW.md` 和必要的 `STATUS.md` 交接摘要。不得修改应用代码、UI worktree、`runtime/`、用户数据或证据原件；不得提交、推送、打标签；模型准入保持关闭。
- **步骤：** (1) 核对工作台引擎支路、手动解释、人工确认保存与准入调用点；(2) 对照现存 OD-BUILD-005 报告和测试，列明已证实及无法恢复的执行前行为；(3) 检查共享文档中 014 状态和停止条件记录一致；(4) 写入短审查结论，若发现实际功能回退或安全边界问题则停止并提出单独修复计划。
- **验收标准：** 代码事实与历史报告分开陈述；关键路径没有发现准入放宽或保存路径丢失，或明确列出具体问题；无法取证的事项明确标为未验证；共享状态一致；应用代码和用户数据未变。
- **风险：** 未保存的执行前工作区快照可能无法恢复，历史报告只能证明其记载范围，不能替代源码。
- **停止条件：** 发现端点闸门被放开、保存功能回退、疑似真实材料/凭证，或需要改代码才能继续核验时，停止并报告，另立修复计划。

- **执行结论（BUILDER，待独立复核）：**
  - 准入未放宽：前端与宿主两个 `ENDPOINT_ISOLATION_CONFIRMED` 均为 `false`；界面提交点只有 `workbench.tsx` 一处，仍经过 `api.ts` 的前端拒绝与宿主 `admission.rs` 的最终拒绝。
  - 人工确认与保存边界仍在：界面必须勾选确认且解释非空才可保存；宿主再次校验 `user_confirmed`、来源、统计指纹、仓库外路径和禁止覆盖。
  - 发现一项真实缺陷：`runEngineExplanation` 在提交后一直保持 `engineBusy = true`，轮询到终态或 180 秒超时才结束；取消按钮同时以 `engineBusy` 和“非终态”为可点击条件。结果是在运行时被前者禁用，运行结束后被后者禁用，界面上的取消按钮实际无法使用。底层取消能力存在（OD-BUILD-005 历史脚本记录过取消成功），缺的是界面触发点。该问题不是端点准入放宽，也不能证明是 OD-ARCH-014 引入的回退（执行前 HEAD 没有引擎解释支路）；需要单列小范围修复计划，本计划未改代码。
  - 无法逐行证明 OD-ARCH-014 未改变 `workbench.tsx` 原有业务逻辑：执行前未提交版本没有找到快照或 stash；现有 OD-BUILD-005 截图与报告只能证明引擎支路在 014 之前已经存在，不能替代逐行差异。
  - 角色分工冲突：本计划允许更新 `REVIEW.md`，但根目录 `AGENTS.md` 规定 `REVIEW.md` 是 ARCHITECT 的独立审查记录。BUILDER 未写 `REVIEW.md`，将核对结论记入 `STATUS.md`，由 ARCHITECT 决定是否补写审查。

OD-ARCH-015 已执行并独立复核通过。OD-ARCH-016 已由 BUILDER 执行完毕并停止，等待 ARCHITECT 独立复核；剩余两项工作「会话持久化」与「旧 DSH 会话导入」暂未排期。模型端点继续 no-go。

### OD-ARCH-016：修复工作台引擎任务取消入口（复核未通过，纳入 OD-ARCH-017 修正）

- **状态：BUILDER 已按批准执行完毕，等待 ARCHITECT 独立复核；审查通过前不宣称验收完成。** 用户本轮明确「批准 OD-ARCH-016 执行」。执行记录见 [STATUS.md](STATUS.md) 顶部，证据见 [tests/verification/desktop/evidence/od-arch-016/README.md](tests/verification/desktop/evidence/od-arch-016/README.md)。
- **分类：** 工作台可用性缺陷修复与取消状态验证。
- **目标：** 让正在执行的工作台引擎任务可以发出取消请求，并在正常完成、取消确认、取消未确认/超时等情况下如实显示状态；保持“请求取消”不等于“取消成功”的既有语义。
- **影响范围：** 仅限 `desktop/src/workbench.tsx`、必要的前端状态测试/测试夹具、专项运行证据，以及 `STATUS.md`。如发现必须改 `desktop/src/types.ts`、IPC 契约或 Rust/Python 适配层才能完成，应停止并报告，不在本计划内扩展。不得改 `runtime/`、准入常量、模型路由或凭证；不调用真实模型、不输入真实业务材料、不启动用户正在使用的实例；不得提交、推送或打标签。
- **实施步骤：**
  1. 记录执行前 HEAD、工作区状态及目标文件差异；检查现有取消阶段和任务状态契约，只针对取消按钮生命周期设计最小改动。
  2. 允许活动任务在运行中点击取消；避免重复取消请求，并保留任务轮询，以宿主回读状态为最终事实。不得让取消按钮因等待函数仍在运行而失效。
  3. 处理取消请求与正常完成竞争：若任务已进入终态，显示真实终态；只有宿主明确返回 `confirmed_cancelled` / `cancelled` 才显示取消成功；请求失败或超时须显示未确认，不能伪称成功，也不能覆盖已有解释内容。
  4. 用隔离的合成任务或确定性前端测试覆盖：活动任务可点击取消、重复点击受控、取消确认、任务先正常完成、取消未确认/超时、任务输出与现有解释状态不被误写。测试不依赖外部模型凭证。
  5. 运行定向测试及完整前端构建；若变更触及共享状态机或 IPC，再运行计划要求的 Rust 离线回归。检查端点准入仍关闭、`runtime/` 无差异、`git diff --check HEAD` 通过，保存精简证据后停止，等待 ARCHITECT 独立审查。
- **验收标准：** 运行中的任务可触发一次取消；取消与成功完成竞争时 UI 以宿主终态为准；超时/失败明确为未确认；原解释文本不被取消路径破坏；前端构建与定向测试通过；端点仍 fail-closed，`runtime/` 无差异，既有改动保留；无凭证/真实材料访问，无提交推送。
- **风险：** 取消 IPC 可能等待最多配置超时，UI 需保持可响应且避免重复提交；当前模型准入关闭，无法在产品端点上复验模型任务，只能用隔离合成任务验证状态交互。
- **停止条件：** 若需新增 IPC/通信层、修改引擎契约、开放端点或接触真实数据才能修复，立即停止并交由 ARCHITECT 重新规划。

## 已完成并独立审查

### OD-ARCH-011：修正离线流程的日期与保存边界，准备安全的个人合成数据试用

- **状态：已执行并按范围通过独立审查（OD-ARCH-011 主体及 OD-ARCH-012 失败清理补充项）。** 模型写作、真实工作材料用途、长期使用与跨平台体验仍未验证。
- **分类：** 离线场景可靠性修复与用户边界澄清。
- **目标：** 修正影响周记录/项目逾期结果的日历日期校验；为新保存成果明确 OmniDesk 的规范位置，同时保护旧 YvanAI 路径内所有现存文件；让使用者一眼看清当前仅可用合成、公开或明确获准内容，以及红线扫描的局限。
- **影响范围：** `desktop/src-tauri/src/task_shell.rs`、`desktop/src/OfflineFlows.tsx` 及所需定向测试/证据、`docs/decisions.md`、数据布局文档、`STATUS.md`。不得修改 `runtime/`、端点准入、模型路由、凭证或引擎通信；不得读取/迁移/覆盖/删除既有用户成果；不得提交、推送或打标签。若发现新路径会造成旧数据读写歧义，停止并报告，不自行迁移。
- **实施步骤：**
  1. 在不读成果正文的前提下，复核现有数据路径决策和代码默认值；明确只影响未来新文件的规范默认位置，旧路径维持原样。
  2. 修正日期解析，按实际月份天数及闰年规则拒绝非法日期；周记录、期限和判定日共用校验。覆盖闰年、非闰年、月末、非法日期及跨年比较。
  3. 在离线流程醒目提示：当前模型不可用；输入框不等于授权入口；只能输入合成、公开或已明确获准的内容；红线扫描不能可靠识别所有个人身份和工作细节。避免把启发式扫描描述成数据保护。
  4. 使用合成样例验证三个流程、日期边界和新成果保存路径；证明旧路径文件未被改动、保存不覆盖同名成果、失败时无半成品。不得调用模型或输入真实业务内容。
  5. 更新执行证据与 STATUS.md 后停止，由 ARCHITECT 独立审查。
- **验收标准：** 非法日历日期全部拒绝、合法闰日接受；逾期判断边界正确且可复现；新文件只写入明确的 OmniDesk 位置，旧成果未读取正文、迁移或变更；界面准确说明适用范围和扫描局限；Rust 测试、前端构建及计划要求的完整回归通过；模型闸门保持关闭。
- **风险：** 真实成果可能仍在旧目录；任何路径更改都不能隐式迁移或宣称资产统一完成。UI 文案只能降低误用，不能构成代码级授权或完整检测。真实工作场景的模型效果仍取决于后续有效授权及端点准入独立审查。
- **停止条件：** 若实现需要改写旧数据、开放模型调用、读取真实工作材料，或无法确认新保存路径不覆盖既有成果，立即停止相关步骤并交由 ARCHITECT 重新规划。

**已执行；BUILDER 停止并等待 ARCHITECT 独立审查。审查通过前不得据此计划宣称验收通过或进入下一阶段。**

## 历史计划与记录

> **历史状态快照（OD-ARCH-009 执行后）：** 当时其结论为 no-go、待独立审查；随后审查与主工作区复核已记录在 [REVIEW.md](REVIEW.md)。OD-ARCH-008 已在 BUILDER worktree 的计划范围内通过独立复核；构建重跑限制与剩余验证见 REVIEW.md。该段不代表当前计划状态。

## 执行结果（待审查）

OD-ARCH-009 已按下列条文执行完毕，交付记录见 [STATUS.md](STATUS.md) 顶部与 [REVIEW.md](REVIEW.md) 顶部。

- 结论 **no-go**：宿主无法据代码确认模型端点、工具或数据去向已限定在批准范围内；`ENDPOINT_ISOLATION_CONFIRMED` 保持 `false`。
- 已确认的旁路：子进程与 Python Adapter 继承完整宿主环境（`DEEPSEEK_BASE_URL`、`DEEPSEEK_API_KEY`、代理变量）；`OMNIDESK_DSH_PATCHES`、`OMNIDESK_DSH_CLI`、`OMNIDESK_DSH_PROFILE` 无二次校验。
- 已确认 DSH 内置 `web`/`web-search`/`web-fetch`/`bash`/`fs` 与遥测通道默认存在；ACP 请求的 `mcpServers: []` 只约束 MCP，不约束内置工具。
- 建议的最小 fail-closed 方向：宿主白名单环境 + `env_clear()`、显式端点与工具配置、patch 内容校验、回环与真实凭证互斥、无批准记录即拒绝，并配套环境继承/patch 注入/真实端点拒绝等测试。
- 下一步需由用户或单位提供端点、服务商、设备、运行环境与用途的书面证据，再决定是否立项实现端点守卫。本计划不授权该实现。

## 历史待批准计划

### OD-ARCH-009：审计端点准入所需证据并形成最小可执行方案（已审查）

- **状态：已执行并完成静态审查（2026-10-18）；no-go。未改代码、未开放准入。** 审查发现执行报告所述宿主拒绝闸门只存在于另一 worktree，当前主工作区没有该代码接线；详见 REVIEW.md 顶部。
- **分类：** 安全边界架构审计与方案决策，不是开放模型调用的授权。
- **目标：** 查清当前 DSH 引擎启动链中，端点、配置和子进程环境如何影响数据外发风险；明确什么可由宿主代码验证、什么仍需单位或用户提供依据；提出最小 fail-closed 准入方案，使后续是否启用本机合成测试或许可范围内个人分析有可审查依据。
- **影响范围：** 只读检查桌面宿主、Engine Adapter、启动配置与相关技术文档；交付限 STATUS.md、REVIEW.md、PLANS.md 和必要的 docs/decisions.md 记录。不得改 ENDPOINT_ISOLATION_CONFIRMED、模型路由、凭证、DSH Runtime、用户数据或运行实例；不得读取或输出秘密/凭证值、真实业务材料；不得新增准入旁路、调用模型、合并或提交。
- **实施步骤：**
  1. 沿代码追踪 Tauri 宿主到 Python Adapter、DSH 子进程的启动环境和配置来源，只记录路径、变量名与控制流，不读取配置/凭证内容。
  2. 列出可能改变数据去向的入口（模型端点、MCP/工具、配置继承、日志与任务保存），区分源码事实、可运行验证、用户/单位授权证据及未知项。
  3. 对照现有宿主拒绝测试，评估可核实的最小准入判定与无法由本机代码证明的前提；检查是否存在环境变量、普通配置、UI 或测试模式可绕行的风险。
  4. 只输出一个明确的 go/no-go 设计：准入事实来源、宿主强制点、默认拒绝条件、所需测试及启动前置条件。若证据不足，保持 no-go 并列出最少待补证据；不自行实施或开放入口。
  5. 更新执行记录和审查文档，明确方案不会扩大已获授权的用途、设备、模型服务商或环境范围。
- **验收标准：** 启动链路与潜在数据去向有代码路径依据；秘密值及真实业务内容未读取/记录；所有已知准入旁路得到检查；交付可供 ARCHITECT 作出下一阶段批准/否决的短方案；代码中的准入常量保持关闭，且没有任何模型调用。
- **风险：** DSH 可能从仓库外配置或继承环境加载端点/工具设置，单靠静态审查无法证明隔离；不得把“本机运行”或“脱敏”表述为端点已隔离或已获授权。此时结论应为 no-go，而不是放宽检查标准。

**完成后停止并交 ARCHITECT 审查；该计划本身不批准实现准入或调用模型。**

### OD-ARCH-008：端点准入关闭时阻止引擎描述命令启动子进程

- **状态：已执行，计划范围内通过；独立重跑限制见 REVIEW.md。**
- **分类：** 架构安全边界审查与最小生命周期修复。
- **目标：** `engine_describe` 当前会初始化并启动 DSH 子进程，且应用启动时会自动调用。端点准入固定关闭期间，应让窗口可正常打开并展示明确的“引擎未启动/端点未确认”状态；不得因读取能力描述而启动引擎。
- **影响范围：** `desktop/src-tauri/src/lib.rs`、必要的 `engine_bridge.rs` 或描述符类型、前端应用启动/技术诊断显示、宿主准入测试、OD-ARCH-008 隔离证据与 `STATUS.md`。不得修改 Runtime、端点常量、模型路由、凭证、IPC-1/IPC-2 既有字段契约（除非先在计划内说明兼容调整）、用户数据或其它运行实例；不得提交、推送、合并、打标签或清理工作区。
- **实施步骤：**
  1. 只读追踪所有 `engine_describe` 调用点及当前引擎生命周期，确认窗口初始渲染实际依赖的描述字段。区分纯静态能力元数据与必须通过启动子进程才能取得的运行时信息。
  2. 设计最小 fail-closed 行为：在宿主准入关闭时，启动/描述调用不得启动 Python/DSH 子进程，不得创建 DSH/Agent 数据根或任务状态；前端改为显示清晰的未准入状态。保留健康状态读取和安全关闭能力。不得加环境变量、UI 参数或普通配置旁路。
  3. 增加宿主层直接调用回归，证明拒绝时进程未启动、路径/数据状态未变化、返回错误可由 UI 识别；若可从静态数据返回描述符，证明该做法不虚报运行能力或可用状态。
  4. 只改应用启动与诊断所需的 UI 状态处理；不扩大为完整分区、端点许可机制或工作台重构。
  5. 运行 Rust 定向与完整离线测试、前端类型检查和构建、准入检查及必要的桌面启动/E2E 回归；更新证据和 `STATUS.md`，不触碰 Runtime/用户数据。
- **验收标准：** 准入关闭时，直接调用描述/启动相关宿主入口不启动引擎子进程，不创建数据目录或任务；UI 可正常打开并准确说明引擎不可用原因；健康读取及关闭不回归；无配置旁路；计划要求的定向与完整回归通过，未验证事项如实记录。
- **风险：** 当前前端可能假设 `engine_describe` 总能返回真实初始化描述符。若无法在不虚报能力、不改变既有协议语义的前提下实现安全空态，停止并提交选项供 ARCHITECT 重新规划，不自行放宽准入。

**完成后停止并交 ARCHITECT 独立复核；不自动进入提交或合并。**

### OD-ARCH-007：封闭工作目录准备命令的宿主绕行

- **状态：计划范围内通过，见 REVIEW.md 顶部 OD-ARCH-007 审查。**
- **执行证据：** `/mnt/data/Code/omniharness-ui/tests/verification/desktop/evidence/od-arch-007/`；独立定向复跑 3/3 通过。

### OD-ARCH-007：封闭工作目录准备命令的宿主绕行（原计划，已执行）

- **状态：已执行并通过计划范围审查；当前状态见本文件上方与 REVIEW.md。** 以下保留原计划条文供历史对照。
- **目标：** 当宿主端点准入关闭时，直接调用 `task_prepare_workspace_dir` 也必须在创建目录或触达其他数据前拒绝；保持默认拒绝与零副作用一致。
- **影响范围：** `desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/admission.rs`（或已有共享宿主闸门）、对应 Rust 测试、必要的前端错误呈现、OD-ARCH-007 验证证据与 `STATUS.md`。不触碰 `runtime/`、凭证、模型路由、真实业务材料或既有用户数据；不提交、推送、合并、打标签或清理工作区。
- **实施步骤：**
  1. 只读核对所有 Tauri 命令中会创建工作区/任务目录或启动引擎的入口，确认最小受影响范围；不要把只读状态查询或用户明确触发的保存操作无差别封锁。
  2. 让 `task_prepare_workspace_dir` 在调用 `prepare_local_data_root()` 前经过宿主准入；拒绝时返回清晰、无敏感输入的结构化错误，并确保不初始化引擎、不创建目录、不写任务或用户数据。不得添加环境变量、UI 参数或普通配置旁路。
  3. 增加直接覆盖宿主命令顺序的回归：使用不存在的一次性临时路径，调用拒绝路径后断言路径仍不存在、引擎未运行且无任务副作用。测试不得删除或改写既有用户数据。
  4. 确认前端在提交前准备工作目录的流程能正确显示拒绝原因，且不会把失败显示为成功；如宿主拒绝后该路径不可用，保持端点未获准时的预期禁用状态。
  5. 运行 Rust 定向与完整离线测试、前端构建及相关准入/E2E 回归；更新 `STATUS.md` 记录实际命令、结果和证据，留待 ARCHITECT 独立 Review。
- **验收标准：** 直接调用宿主工作目录准备命令时，拒绝先于文件系统副作用；断言目标目录不存在、引擎未启动且没有任务创建；无普通配置旁路；现有 UI 显示准确；相关测试通过；保留所有既有工作区改动，不提交、不推送、不合并。
- **风险：** 此修复会使当前端点未获许可时的 UI 引擎解释路径在准备目录步骤被拒绝，这是预期的 fail-closed 行为；不能为通过合成模型测试而提供生产绕行开关。

**完成后停止并交 ARCHITECT 独立复核，不自动进入提交或合并。**

### OD-ARCH-006：宿主提交默认拒绝、开发工作区隔离和状态文档校准

- **状态：OD-ARCH-006 与 OD-ARCH-007 的拒绝边界修复已通过各自计划范围审查；应用启动时引擎描述仍会拉起子进程，后续审查计划见 OD-ARCH-008。**

### OD-ARCH-006：原批准计划（已执行，审查暂缓验收）

- **状态：已执行；提交拒绝、端口隔离和架构文档校准有执行证据。目录准备绕行由 OD-ARCH-007 修复；应用启动时引擎描述的生命周期问题列入 OD-ARCH-008。** 本节保留原计划内容供对照。
- **目标：** 在端点隔离事实缺失期间，让 Rust 宿主也默认拒绝 UI 引擎提交；防止并行 worktree 的桌面窗口误加载另一工作区的前端；校准过时的桌面架构现状说明。
- **影响范围：** `desktop/src-tauri/src/lib.rs` 及对应 Rust 测试、必要的前端错误提示、`scripts/dev-ui.sh` 及其隔离测试、`docs/OmniDesk-Desktop-Architecture.md`、`STATUS.md`。只允许合成数据与隔离临时目录。不修改 IPC 形状、模型路由、凭证、`runtime/`、真实业务材料或用户数据；不提交、推送、合并、打标签或清理工作区。
- **实施步骤：**
  1. 在 Tauri `engine_submit_task` 命令的宿主入口增加 fail-closed 检查：目前没有宿主可核实的端点许可证明，故应用命令必须在初始化/提交引擎任务前拒绝，并返回清晰、非敏感的原因。不得新增可由环境变量、UI 参数或普通配置打开的绕行开关。专用合成回归脚本是否仍能直接验证引擎链路，先检查现状，避免为测试留下生产绕行口。
  2. 增加宿主层回归：直接覆盖命令拒绝路径，并证明未启动/调用引擎、未创建任务或写入用户数据；确认所有 Tauri 暴露的提交命令都受同一拒绝规则。前端提示应与实际宿主行为一致。
  3. 修复 `dev-ui.sh` 固定开发端口冲突：若 `127.0.0.1:5273` 已被非本次启动器拥有的服务占用，启动前清楚失败，不启动 Tauri 窗口、不向占用服务发送信号，也不复用其页面。用两个隔离临时 worktree/受控服务复现并验证；不得停止用户的其他实例。
  4. 仅根据现有代码和已运行检查，更新桌面架构文档中“工程尚未创建/工具链缺失”等已过时陈述；区分当前 Linux 实测、其他平台未验证和端点未确认。
  5. 跑完定向测试及计划要求的完整回归；将命令、通过数、证据路径、失败/未验证项写入 `STATUS.md`，不把静态检查写成运行验证。
- **验收标准：** Rust 宿主直接调用提交命令时也 fail-closed，且没有普通配置旁路；拒绝前不启动引擎或产生任务副作用；端口被其他工作区占用时启动器安全退出且不碰占用进程；文档准确反映现状；Cargo 离线测试、前端构建及准入/启动器/E2E 相关检查通过，或逐项给出可复核的环境阻塞；所有既有改动保留。
- **风险：** 所有桌面引擎提交（包括合成端点的 UI 手动路径）都会保持关闭，直到另行设计并验证宿主可核实的端点许可来源；这是当前授权事实不足下的预期行为。固定端口策略意味着两个 worktree 不能同时启动桌面 UI，但会明确提示而不是串载。
- **停止条件：** 若无法在不改变 IPC 形状、不增加端点/路由或凭证的前提下做宿主拒绝，停止并报告；若现有测试依赖可由普通构建打开的提交开关，不得自行加白名单或弱化拒绝。

## 已完成并复核的近期计划

### OD-ARCH-005：封闭技术切片自由文本绕行并校准启动器身份核验

- **状态：** OpenCode worktree 的计划范围内通过；宿主提交入口仍未强制拒绝，见 REVIEW.md。证据在 `tests/verification/desktop/evidence/od-arch-005/`（OpenCode worktree）。

### OD-UI-002：独立审查工作台视觉改造

- **状态：** OpenCode worktree 的计划范围内通过；截图与证据在 `tests/verification/desktop/evidence/od-ui-002/`（OpenCode worktree）。未验证跨平台和真实模型输出。

### OD-ARCH-005：封闭技术切片的自由文本提交绕行并校准启动器身份核验

- **状态：待用户批准；批准后由用户指定 BUILDER。**
- **目标：** 确保所有可调用引擎的 UI 入口都受同一准入边界保护，并使启动器实际身份检查与其报告一致。
- **范围：** `desktop/src/shell/EngineTechSlice.tsx`、必要的现有前端准入组件/测试、`scripts/dev-ui.sh`、OD-ARCH-004 证据说明、`STATUS.md`。不改变 IPC-1，不增加新的模型端点或配置，不触碰凭证、runtime、真实业务材料、用户数据或其他运行实例。
- **步骤：**
  1. 技术切片改为只读诊断，或复用与首页完全相同的提交准入门槛；未得到宿主可核实的端点隔离事实时，自由文本输入与提交均禁用，并在处理函数内二次拒绝。
  2. 审计前端全部 `host.submitTask` 调用点，证明每条 UI 路径都受相同限制；合成测试仍可通过专用验证脚本调用引擎，不需保留面向用户的绕行入口。
  3. `dev-ui.sh` 停止进程时核验记录中的命令身份，或删去“命令行已验证”相关记录并使用更强且可测试的身份核验；严禁身份不符时发送信号。
  4. 校准执行报告与代码实际行为；补测技术切片禁用/二次拦截与启动器身份不符场景。运行必要前端构建和既定回归，受环境限制的检查如实登记。
- **验收：** 所有 UI 提交入口在未获准时禁用并有处理层二次拦截；扫描无未受保护的 UI 提交调用；启动器不会停止命令身份不符的进程；报告与代码一致；保留全部现存工作区改动，不提交/推送/打标签。
- **风险：** 技术切片将不能再手工发起任务；这是端点准入事实缺位时的必要限制。若实现需要新增宿主端点证明协议，停止并报告，另立架构计划。

### OD-UI-002：独立审查 OpenCode 工作台视觉改造

- **状态：待用户批准；可由用户交 OpenCode 执行。** 这不是代码合并授权。
- **目标：** 在不碰业务流程的前提下，验证并校准「本地数据分析」工作台与首页的视觉一致性及可读性。
- **范围：** 仅 `desktop/src/workbench.tsx` 的样式挂载点与 `desktop/src/workbench-ui.css`，以及新增的截图/简短证据说明。不得改五阶段状态、文本/数字处理、IPC、Rust、准入逻辑、共享设计令牌或既有首页样式。
- **步骤：**
  1. 在可写构建环境验证 `npm run build`；若当前依赖目录只读，不删除或重装依赖，改用仓库既有可写构建方式或报告阻塞。
  2. 以合成样例启动隔离窗口，切换到分析工作台并留截图；检查中文对比度、页面滚动、窄窗口、统计表格、错误/禁用提示及五阶段入口可见性。
  3. 必要时只调整局部 CSS；确认未隐藏边界提示、数字单位、来源说明、失败信息和人工确认门槛。
  4. 记录改动文件、构建结果、截图与未验证项。
- **验收：** 构建通过；窗口截图证明工作台视觉清晰且局部样式不泄漏到首页；五阶段行为与 IPC 不变；仅合成数据；不提交、不推送、不合并。
- **风险：** 字体回退与 Tauri WebView 在其他平台的呈现未必一致；本项只验当前 Linux，不作跨平台承诺。

### OD-ARCH-004：收紧 UI 输入边界并修复开发启动器（已执行，部分验收）

- **状态：已执行；ARCHITECT 独立复核发现遗留绕行，未整体验收。** 实施证据见 STATUS.md 和 `tests/verification/desktop/evidence/od-arch-004/`；审查结论见 REVIEW.md。
- **分类：安全边界修正 + 可靠性修复；不改产品方向。**
- **目标：** 在决定是否保留 UI v2 前，确保首页对话不会让人误以为可输入任意工作内容；开发启动器仅能安全使用隔离合成环境；随后独立验收 UI v2 与 OD-BUILD-005。
- **影响范围：** HomeChat.tsx、必要的提示文案/组件；dev-ui.sh；针对准入提示、轮询故障和启动/停止行为的定向测试或验证记录；STATUS.md。不得触碰 runtime/、真实材料、模型路由、凭证、上游或其他运行实例；不得提交、推送或打标签。
- **实施步骤：**
  1. 在首页对话入口清楚显示当前用途及准入限制，明确仅供公开/合成内容；说明用户转述的有限许可不覆盖一般对话、写作或项目管理。不得把文案冒充代码级隔离；若无法确认本入口实际使用模型/运行环境边界，则默认禁用提交，先只保留 UI 预览。
  2. 修复对话状态与结果轮询的静默吞错：有限重试/超时后显示可理解的失败状态和恢复动作，不伪报成功、不无限显示生成中。
  3. 修订 dev-ui.sh：移除会写入仓库 runtime/ 的软链接回退；不得默认复用不明来源的 5273 服务；PID 文件须按运行目录独立、记录进程身份并在停止时验证归属，避免覆盖或误杀其他实例；仅保留隔离合成端点路径，移除或拒绝 --real 以免绕过边界。脚本失败时应明确停止，不留下半启动实例。
  4. 对 UI worktree 与主工作区分别核对差异；仅在有可写构建目录的条件下运行前端构建。独立重跑合成引擎路径和 Tauri 窗口验收；若受环境限制无法运行，标记未验证，不以另一 worktree 的产物替代证明。不得因验证失败直接清理或重建既有 node_modules、运行时或工作区。
  5. 将 UI v2、OD-BUILD-005 引擎切片、边界限制及全部未验证项分开记录；保留未跟踪设计资产，不删除、不擅自纳入提交候选。
- **验收标准：**
  1. UI 明示准入范围和边界；没有被确认的分区与端点执行隔离时，不允许把通用输入提交模型。
  2. 轮询中断与结果读取失败都有可见、有限、不会误导的结果；构建通过。
  3. 启动器的成功、端口占用、重复启动和停止场景有隔离验证；不修改 runtime/、不覆盖他人 PID 文件、不停止非本任务进程、不连接真实端点。
  4. Rust 回归、前端构建及合成 E2E 按可运行条件重跑；证据逐项注明是新复跑还是既有报告。窗口内路径有新证据；若环境不能运行，明确列为未验收项。
  5. 保留所有工作区既有差异；不提交、不推送、不打标签；审查结果仅由 ARCHITECT 在 REVIEW.md 独立记录。
- **风险：** 为安全起见，首页对话可能暂时只能展示而不能提交，直到确认模型和运行环境适用范围；启动器变更会影响开发体验，故须覆盖端口冲突和停止场景。UI 设计稿与应用素材的版权/入库决定仍未解决，本计划不删除或擅自提交这些资产。

**本计划完成后停止并交 ARCHITECT 复核；不自动进入提交/合并。**

### OD-BUILD-005：把已实现的引擎链路接进日常任务流程（第一个能真用的纵向流程）

- **状态：已获批并实施；ARCHITECT 审查暂缓整体验收。** 原计划如下留存供对照，当前缺陷与复核条件见 REVIEW.md 顶部及 OD-ARCH-004。
- **分类：功能实现（既有能力的接线与可用性），非架构变更。**
- **立项依据（均为已核实事实）：**
  1. 引擎链路本身已通：`engine_submit_task` → Python `serve.py` → ACP → DSH，隔离桌面 E2E chain（提交/失败/取消）已通过。证据等级：script。
  2. 但**任务外壳 UI 从不调用引擎**：`desktop/src/workbench.tsx` 只使用 `shell.*` 本地命令（统计、提示词、保存、使用记录），全文未出现 `host.submitTask`；引擎调用只存在于技术切片视图 `App.tsx` 的事件流面板。证据等级：static。
  3. 后果：用户看到的「无法选数据、无法下指令、看不到结果」不是能力缺失，而是**已实现的能力没有连到日常流程上**；模型解释目前只能靠人工复制提示词、再把结果手工贴回来。
  4. 与用户明确的首要目标一致：先做到「我自己愿意天天用」，再谈后续扩展。
- **目标：** 让作者在桌面应用内，用合成/公开/已批准数据走完一次完整任务：输入 → 确定性统计 → 由应用发起引擎解释 → 人工确认 → 本地保存；失败或未配置端点时明确报错，不出现假成功。
- **影响范围：**
  - `desktop/src/workbench.tsx`（在「阶段 3 · 解释」接入引擎调用与结果回填）。
  - `desktop/src/api.ts`、`desktop/src/types.ts`（仅在确需新绑定时）。
  - `desktop/src-tauri/src/lib.rs`与`task_shell.rs`（仅在需要把已冻结的提示词交给引擎时；优先复用现有 `build_explanation_prompt`，不新建提示词来源）。
  - 桌面测试与验证脚本（`desktop/src-tauri/tests/`、`tests/verification/desktop/`）及其证据。
  - `STATUS.md`。
  - **不得**修改 `runtime/`、模型路由、凭证、DSH 数据或任何真实业务材料；不得改变 IPC-1 契约形状。
- **实施步骤：**
  1. **确认落点（只读）**：核对 workbench 阶段 3 的状态机（`analysis` 指纹、`promptText`、`explanation`、`source`、`confirmed`），确定唯一插入点；保持「输入变化即失效下游产物」的现有约束。
  2. **最小接线**：在阶段 3 增加一个「用引擎生成解释」动作，把 `build_explanation_prompt` 产出的**冻结提示词**提交给引擎（`intent` 用该任务类型标识，`workspace_dir` 指向隔离数据根），订阅事件，任务到达终态后把 assistant 文本回填到解释框；`source` 仍记为 `model_unverified`，不得引入「已复核模型」来源。
  3. **失败与无端点路径**：未配置凭证/端点不可用时按现有错误语义明确呈现（复用 `normalizeError`）；保留现有「生成提示词 → 手动粘贴」路径作为回退，两条路径并存且互不污染状态；超时与取消复用现有 `cancelTask` 语义，不得把「已请求取消」写成取消成功。
  4. **证据（关键）**：用**本机回环合成端点**（不接真实模型、不用凭证）跑通完整路径并留证据：输入合成样例 → 统计 → 引擎解释回填 → 人工确认 → 保存；随后在**真实 Tauri 窗口**里重做一次并留截图。若窗口路径走不通，如实记录阻塞，不降低标准。
  5. **回归**：`cargo test --offline`、`npm run build`、桌面 E2E chain 全部重跑；确认 workbench 仍只经 IPC-1，界面代码不出现 ACP/DSH 私有概念。
  6. 更新 `STATUS.md`：按文件事实 / 静态检查 / 脚本运行 / 真实模型（预计仍为未验证）/ 未验证 分级记录。
- **本计划明确不做（附理由）：**
  - 不做任务列表、项目管理、周报等 `reserved` 类型——尚未授权且未实现。
  - 不做本地文件选择器（打开磁盘上的 CSV）。它会引入新的 Tauri dialog 能力与安全边界评审，属独立决策；**最适合作为 OD-BUILD-006 候选**，本计划先用粘贴与内置合成样例。
  - 不做会话持有、追加日志、备份恢复演练（v0.4 议题，另有触发条件与审计结论）。
  - 不缩短非 ACP Profile 的 30 s 等待（裁决见 REVIEW.md）。
  - 不把「已实现任务类型」从 1 个扩展为多个。
- **验收标准：**
  1. 在真实 Tauri 窗口内，用合成样例可走完「输入 → 确定性统计 → 引擎解释 → 人工确认 → 保存」，每步有可追溯运行证据（脚本输出与截图）。
  2. 统计数值仍**只**来自 `task_run_analysis`；解释阶段保持指纹比对，不得重算或补齐数字。
  3. 无端点/失败时界面明确显示原因，不显示成功；解释来源下拉不出现任何「已复核」选项；未人工确认不得保存。
  4. workbench 与前端代码不新增对 ACP、DSH 私有格式或 DSH 配置的直接依赖；IPC-1 形状不变。
  5. `cargo test --offline`、`npm run build`、桌面 E2E chain 全部通过；`git diff --stat -- runtime/` 为空；既有未提交差异全部保留，未提交、未推送、未打标签。
- **风险：**
  - 第一次在窗口内接引擎会暴露事件订阅、终态判定与状态时序问题（流式文本与状态卡刷新竞争）。范围止于「解释」这一步；出现时序问题单独记录并最小修复，不顺手重构 workbench。
  - 若本机没有可用模型端点，本次只能验证到「提示词 + 手动粘贴」并如实标记模型段未验证；仍是有效进展，不得为凑结论而调用未获批准的端点。
  - 合成端点通过不等于真实模型输出质量合格；质量与合规仍按 GOALS.md 与 REVIEW.md 的限制另行判断。

**原计划已由用户批准并执行；不构成 OD-ARCH-004 的批准。**

## 最近执行计划

### OD-BUILD-004：闭合引擎 Profile 启动与错误验收

- **状态：已通过（计划范围）。** ARCHITECT 独立复核结论见 REVIEW.md 顶部同名章节。执行事实见 [STATUS.md](STATUS.md) 顶部。
- **实施结果摘要：** 缺口 1 由新增单测 `engine_config_default_profile_chain_is_acp` 直接以 `EngineConfig::from_env()` 证明默认链产出 `acp`（`child_env()`、`initialize_params()` 均传递），不依赖脚本注入；缺口 2 在 `acp_adapter.py` 的 TIMEOUT 分支写入所配 Profile、预期协议（ACP）与可读原因（落于 `detail`，因 `serve.py` 的 JSON-RPC `message` 字段被用作错误码），并与“Profile 不存在”错误可区分。等待时长按计划**未缩短**。
- **已裁决：** 启动前预检不作为当前验收阻断项，登记为低优先级后续项，理由见 REVIEW.md。

### OD-BUILD-004 原计划（原始条文，保留供对照）

- **分类：功能修复与验收补强（引擎启动配置）。**
- **目标：** 证明桌面宿主不设置 OMNIDESK_DSH_PROFILE 时，实际传给引擎的默认值为 acp；当显式配置了不能讲 ACP 的 Profile 时，在有界等待后返回包含 Profile 名称和预期协议的清楚错误，而不是只显示通用超时。
- **影响范围：** desktop/src-tauri/src/engine_supervisor.rs、与宿主配置/启动相关的必要测试、Profile 场景验证脚本及其证据、STATUS.md。确需改引擎错误映射时，只限现有 engine/omnidesk_engine/ ACP 初始化错误路径。不得修改 runtime/、DSH 数据、模型路由或凭证；不开发 GUI、不新增协议、不扫描或依赖 DSH Profile 内部 bundle 清单；保留现有未提交改动，不提交、不推送、不打标签。
- **实施步骤：**
  1. 增加隔离、可重复的宿主配置测试，直接通过 EngineConfig::from_env() 构造配置，并验证 child_env() 与 initialize_params() 都传递 acp；测试不依赖手工注入 acp，也避免并行测试争用全局环境变量。
  2. 检查当前 ACP 握手超时和子进程退出信息如何呈现。采用最小方案，让非 ACP/无响应 Profile 的失败信息指出所配 Profile、预期 ACP，并在现有有界超时后结束本次启动；只有在不影响正常 ACP 启动的证据支持时才缩短超时。不通过读取 DSH Profile 私有结构来预判。
  3. 用隔离数据根验证：默认 Profile 完成 ACP 初始化与 session/new；未知 Profile 和已存在但非 ACP 的 Profile 都得到可区分、可读的错误；确认子进程退出且不会遗留服务。
  4. 重跑相关 Rust 测试、Profile 场景及前端构建。E20 最近一次记录显示测试期间 `~/.dsh` 有原版 DSH 的后台写入且留有临时场景目录；若需重跑 E20，必须区分测试自身痕迹与并发后台变化，不得将其表述为全局零变化，也不得清理用户数据或旧临时目录。若隔离边界代码未变，可引用当前 E20 证据，避免无必要重跑。
  5. 更新执行记录和审查记录时，将较早章节中“待修复/待批准”的状态明确标注为当时的历史状态，保留原始审查事实，确保当前结论只以文档顶部的最新审查为准。
- **验收标准：**
  - Rust 宿主默认配置链经直接测试证明为 acp，不靠测试脚本手动传入该值；隔离运行中默认启动可完成 ACP 初始化和会话创建。
  - 未知 Profile 与非 ACP Profile 都不返回成功；后者失败时的错误说明所配 Profile 与 ACP 预期，并且启动子进程已结束。等待时间有上限；若调整上限，默认 acp 的实测启动必须证明不会被误判。
  - 原有 cwd 规范化代码及 R3/R5 验证保持通过；不引入 DSH 私有格式依赖。
  - runtime/ 与用户数据根无本轮痕迹；保留所有既有差异。
- **风险：** 缩短 ACP 握手等待可能误伤启动较慢的有效 Profile；需用默认 acp 的隔离真实启动回归证明新判断不会误报。跨平台行为仍不由 Linux 测试覆盖。
- **验收后：** ARCHITECT 根据实际 diff 与测试独立复核；本计划获批后仍由用户指定 BUILDER 执行。

## 已完成计划

### OD-BUILD-003：引擎 Profile 默认与 cwd 规范化最小修复

- **状态：实现完成；ARCHITECT 部分通过。** 具体审查见 REVIEW.md 顶部。
- 以下为**当时**状态，其余项已由 OD-BUILD-004 闭合：cwd 规范化修复由 R3/R5 和源码检查支持；默认 Profile 的源代码设置正确，但当时 R1 手动注入 acp 并显式传入该值，未验证 Rust 宿主默认配置链；R2 显示未知 Profile 可读报错，但已存在的非 ACP web Profile 约 30 秒后以通用超时失败，未满足原清晰报错标准。
- **后续：** 两项剩余缺口已由 OD-BUILD-004 聚焦闭合；Finding B、会话正文独立日志和桌面产品化不在范围内。

### OD-ARCH-002：真实 Tauri 窗口的合成数据任务路径验证（暂缓）

- 状态：未批准、未执行；因 v0.4 将会话持有与故障根因审计前移，暂缓至 OD-ARCH-003 后重新判断。
- 原目标：用合成数据验证真实 Tauri 窗口的输入、分析、解释和保存边界。
- 暂缓不构成通过或否决；原计划细节仍保留在本节之后供后续恢复。

### OD-ARCH-002 原计划（暂缓）

- **状态：待用户批准；未授权执行。**
- **目标：** 在不接触真实业务材料或凭证、不改变模型路由的条件下，校准项目当前状态记录，并取得真实 Tauri 窗口中使用内置合成样例完成任务外壳流程的可复核证据，为后续是否进入严格限定的个人自用试点提供依据。
- **影响范围：** `README.md`、`GOALS.md`、`docs/decisions.md`、`STATUS.md`、`REVIEW.md`、`PLANS.md`；必要时只读检查 `desktop/` 配置与 UI。不得修改 `runtime/`、凭证、真实用户数据或 DSH 原有数据。不得把本计划扩展成新功能或产品化开发。
- **实施步骤：**
  1. ARCHITECT 已在本轮将 `README.md`、`GOALS.md`、`docs/decisions.md`、`REVIEW.md` 和 `PLANS.md` 的当前状态校准，并保留 OD-BUILD-001/R4 历史审查。BUILDER 只需检查 `STATUS.md` 与其他入口无现行冲突；不得改写或删除历史执行事实。
  2. 静态核对 Tauri 启动配置、任务工作台状态转换、解释/保存桥接和应用数据根设置；只记录配置来源与当前默认行为，不读取密钥或凭证值。
  3. 在隔离的临时应用数据根中用**内置合成样例**启动真实 Tauri 窗口，记录从输入、确定性分析、解释提示词、人工确认到保存的实际可行路径。只有当现有模型端点已明确获准用于该次合成数据验证时才调用模型；否则停在提示词预览/人工输入路径，不读取凭证、不调用外部模型、不改路由。
  4. 核对产物落点在仓库外、保存确认按预期生效、使用记录不含输入输出正文；清理仅限本次创建且路径明确的临时测试数据。
  5. 根据证据更新 `STATUS.md`、`REVIEW.md` 和共享状态入口，作出“可进入何种受限个人试用 / 尚不能进入”的明确结论，并列明用户授权摘要不能覆盖的范围。
- **验收标准：**
  - 当前状态入口对 OD-BUILD-001、OD-BUILD-002 和 MVP 验收状态没有相互矛盾的现行陈述，历史审查保留且清楚标为历史。
  - 真实 Tauri 窗口路径有可追溯运行记录及截图或等价证据；不以 Vite 浏览器截图代替。模型调用是否执行、端点是否已获准必须单独记录。
  - 保存位置、人工确认、非覆盖行为及使用记录数据范围均由实际代码/运行证据支持；任何未完成模型步骤均如实标为未验证。
  - 全程不读取/打印凭证，不调用真实业务材料，不改变路由，不触碰 DSH Runtime 或原有用户数据。
  - 最终审查只批准被证据覆盖的合成数据或许可范围内个人数据分析与生态研判路径，不推定分发、周报/材料写作/项目管理、特定模型服务商、工具、设备或环境已获授权。
- **风险：** 当前桌面切片可能无法在真实窗口内完成端到端任务；模型端点授权也可能不足以执行真实调用。此时只记录已验证的本地流程和阻塞，不临时扩展实现或更改路由。桌面截图只能证明界面与流程，不证明模型输出质量或合规授权。

**批准后由用户指定 BUILDER 执行；当前 ARCHITECT 不直接实施。**

## 已完成计划记录

### OD-ARCH-003：v0.4 架构校准与会话丢失机制审计

- 状态：已执行完成（只读审计 + 隔离一次性合成验证）。结论与证据见 [REVIEW.md](REVIEW.md) 顶部同名章节。
- 交付：会话丢失六类归因；原生持久化能力确认（历史可读、可列出、可 resume、恢复后可持续写入）；独立逐消息追加日志维持 **no-go（暂缓）** 并给出采用条件；登记 Finding A（默认 Profile 非 ACP）与 Finding B（分析功能对人不可用），**只登记不修复**；v0.4 边界校准（V4 已发布格式 → DSH 内部状态非可丢弃；合规政策须绑定下层）。
- 边界：未扩大用户转述授权；未运行真实模型；未修改 Runtime、业务代码或用户数据。
- 后续：OD-BUILD-003 已获批并完成实现；独立审查部分通过，补充要求与下一项计划 OD-BUILD-004 见本文件顶部。

### OD-BUILD-002：拒绝桌面应用数据根落入仓库

- 状态：已实施，ARCHITECT Review 通过。
- 目标：在 `AppPaths` 数据根解析和目录创建阶段拒绝仓库内路径、指向仓库的符号链接和仿真禁区，同时允许干净的仓库外路径。
- 实施文件：`desktop/src-tauri/src/app_paths.rs`、`STATUS.md`；无须修改 `lib.rs`。
- 独立验证：本轮 `cargo test --offline` 60 项通过；`npm run build` 通过。checkpoint 的 `git diff --check HEAD^ HEAD` 有历史行尾空白/末尾空行问题，详见 `REVIEW.md`。
- 限制：常规路径校验不能消除检查与创建之间的并发替换竞态；当前仅在 Linux 实测，不代表跨平台已验证。

### OD-BUILD-001：个人 MVP 保存确认、来源真实性与输入处理边界

- 状态：已实施；独立 Review 中 R1–R3 通过，R4 经 OD-BUILD-002 补齐后通过。
- 详细结论保留在 `REVIEW.md`，执行事实保留在 `STATUS.md`。

### OD-ARCH-068：平台中立规则资产化（红线/铁律迁出代码 + 随手记轻装 + 隐私脱敏）（已执行完毕，ARCHITECT 复核通过，2026-10-23）

**批准依据：** 用户明确本轮定调——OmniDesk 是通用个人工作台平台（DSH 底座 + 自有 UI），纪检内容只属于 Discipline Profile；装 profile = 纪检工作台，不装 = 纯净底座。当前两处业务规则硬编码在平台代码里（红线词、写作铁律），违反平台中立；同时用户实测反馈：随手记不需要铁律/章节（应快）、身份证/手机号不应拦截（应自动脱敏后放行）、思考等级按场景默认。

**目标：**
1. 平台代码零业务内容：红线词表与写作铁律从 Rust/TS 硬编码迁出，写入 Profile 规则资产文件；无 profile 时平台为纯净底座（零红线、零铁律），行为与官方 DSH-desktop 无实质区别。
2. 隐私策略调整（用户明确）：身份证号、手机号不再拒绝提交，改为自动脱敏（替换 * 号，保留前 3 后 2 位）后放行；界面提示「已脱敏」；本地线程原文保留不变；纪检关键词（案件/线索/立案等 16 词）仍拦截不变。
3. 随手记轻装：工作记录场景（weekly）不经 buildDraftPrompt 的铁律/章节组装，改为一句轻提示词（整理指令），思考档默认 off；出周报才带完整规则+铁律+章节+本周已归档条目，思考档默认 low。
4. 思考档按场景默认：场景配置文件增加 `reasoning_effort` 字段；用户手动选档优先于场景默认；清除 `~/OmniDesk/app/config/omnidesk-model.yml` 遗留的 `reasoningEffort: high`（065 用户追加项，现应回归场景控制）。
5. Profile 规则资产用户可编辑：设置页「对话分类」或新增「规则资产」分页中可查看/编辑红线词表、写作铁律、场景规则；保存后写入 profile 规则文件（不重启）；文件缺失或为空时平台零规则放行（纯净底座行为）。

**影响范围：**
- Rust：`desktop/src-tauri/src/task_shell.rs`（红线词表外移）、`desktop/src-tauri/src/flow_pack.rs`（如果铁律在此）
- TS：`desktop/src/aiDraft.ts`（铁律迁出）、`desktop/src/shell/HomeChat.tsx`（场景默认思考档、脱敏提示）、`desktop/src/conversationPrompt.ts`（轻提示词路径）
- Python：`engine/omnidesk_engine/task_core.py` 或 `web_api_adapter.py`（如引擎侧还有规则逻辑）；`desktop/src-tauri/src/lib.rs`（配置加载透传）
- 配置：`~/OmniDesk/app/config/offline-flows.v2.json`（新增 reasoning_effort + 脱敏策略 + 红线词表 + 写作铁律字段，v1 迁移）、`~/OmniDesk/app/config/omnidesk.model.yml`（清理 reasoningEffort 遗留）
- Profile：`~/OmniDesk/app/profiles/discipline.rules.json`（新文件：global.writing_rules、global.redline_keywords、scenes.weekly.rules/sections/thinking、scenes.writing.*、scenes.project.*）
- 文档：`docs/decisions.md` 新增 D-028「平台中立与 profile 规则资产化」；`GOALS.md` 若有平台定位表述则校准；PLANS/STATUS/REVIEW

**Profile 规则资产布局（用户可编辑，文件不存在时=纯净底座零规则）：**

```json
{
  "schema": "omnidesk.profile-rules.v1",
  "profile": "discipline",
  "global": {
    "writing_rules": ["铁律 1", "铁律 2", ...],
    "redline_keywords": ["案件", "线索", ...],
    "privacy": {
      "id_card": "mask",        // mask=脱敏后放行；reject=仍拒绝
      "phone": "mask"
    }
  },
  "scenes": {
    "weekly":  { "rules": [...], "sections": [...], "thinking": "low"  },
    "writing": { "rules": [...], "thinking": "low" },
    "project": { "rules": [...], "thinking": "off" }
  }
}
```

**步骤：**
1. **S1 规则资产定义与迁移：** 定义 `omnidesk.profile-rules.v1` schema；创建 `discipline.rules.json` 并把现有 16 红线词、7 条铁律、weekly/writing/project 的现有 rules+sections 全部迁入；平台代码里的对应硬编码改为「读此文件，无文件=零规则」；v1 offline-flows 仍保留（它管的是周报章节结构等可编辑流配置），与 profile-rules 分工——offline-flows 管「周报怎么组装」，profile-rules 管「什么不能说 + 什么规则必须带上」。
2. **S2 隐私脱敏：** 身份证/手机号识别正则已存在（task_shell.rs 已区分 id_card/phone_number/case_keyword 三类），把这两类的动作从「拒绝」改为「掩码后放行」：保留前 3 后 2 位（如 `310*********45`），中间用 `*` 填充；掩码在引擎层执行（不动本地线程原文），界面 toast/批注提示「已自动脱敏」；`privacy` 字段允许用户改回 reject；case_keyword 行为不变（仍拒绝）。
3. **S3 随手记轻装 + 场景思考档：** conversationPrompt.ts 新增「轻提示词」路径：场景=weekly 且模式=随手记时，不带铁律/章节/已归档条目，只带一句整理指令；HomeChat.tsx 场景默认思考档（weekly=low、writing=low、project=off），用户手动选档优先；清除 omnidesk-model.yml 遗留 `reasoningEffort: high`（该项已在 065 被用户追加，但按本轮定调应回归场景控制，不由模型全局默认）。
4. **S4 设置页规则资产分页：** 「对话分类」分页新增或独立「规则资产」分页：红线词表、写作铁律、每场景 rules/sections/thinking/privacy 均可查看与编辑；保存后写入 profile-rules 文件（不重启）；文件损坏时只读报错不覆盖；恢复默认按钮（重新写入出厂默认值）。
5. **S5 测试与证据：**
   - 单测：词表空=放行（纯净底座）、16 词行为不变、掩码样例（310***45 类）、场景思考档默认、v1→profile-rules 迁移无损。
   - verify 脚本：真实窗口跑一遍（无规则文件 → 纯净底座放行；有规则文件 → 纪检词拦截 + 隐私脱敏 + 周报组装）。
   - ttfb 对比：同一随手记任务在「轻装 vs 完整铁律」两种模式下的 ttfb_seconds 记录，落 evidence/od-arch-067/。
   - 回归：cargo test / pytest / tsc / npm build 全绿；runtime/ 零差异。

**验收标准：**
1. 把 `discipline.rules.json` 临时移走 → 重启 → 平台对任意输入零拦截零铁律（纯净底座行为），界面对应提示「未加载 profile 规则」。
2. `discipline.rules.json` 在位 → 案件词仍拒、身份证/手机号掩码放行且提示「已脱敏」。
3. 随手记模式下 ttfb 明显低于带完整铁律模式（记录两组 ttfb_seconds）。
4. 设置页改规则（如删除一条红线词）→ 保存 → 不重启 → 下一任务生效。
5. 全量回归绿；runtime/ 零差异；`omnidesk-model.yml` 的 `reasoningEffort: high` 已清除。

**风险与停止条件：**
1. 纯净底座（无规则）在合规上可接受吗？——本轮定调：profile 缺失=纯净底座属预期行为，平台不内置任何行业规则；需在设置页「关于」或「权限」分页明示「当前未加载行业规则包」，避免用户误以为 OmniDesk 本身有合规拦截。
2. 掩码可能误伤（如把不是身份证的数字串掩掉）→ 正则先保持现有精度（已有 id_card/phone 识别），不扩大匹配范围。
3. 思考档默认值可能与用户习惯不符 → 用户手动选档始终优先，场景默认仅在新会话首次使用时生效。
4. 迁移只补字段不删字段：v1 offline-flows 中已有 rules/sections 保留在原文件，profile-rules 只额外管理铁律/红线词/思考档/privacy；两文件并存时以 profile-rules 为权威（红线词/铁律），offline-flows 继续管周报章节。
5. 同一问题连续两轮修复未解决即停。

**预计规模：** 半天 BUILDER 批次（BUILDER 由用户指定，建议 DSV4.1-flash 或同级）；完成后等用户批准提交推送（与 066 一起分批）。

**后续批次预告（待 068 验收后逐个规划，不在本计划内）：**
- **OD-ARCH-069（候选）：** 周报全链路用户实测（用户亲自操作，走合成或公开材料，按 user-testing-checklist.md 记录 ttfb、返工、卡点）；验收后决定是否进入真实周报试用（需单位书面许可）。
- **OD-ARCH-070（候选）：** 文案写作与项目跟踪的场景化打磨（根据 069 反馈收敛）。

---

### OD-ARCH-069：工具执行可见化 + 超时治理 + 用户实测清单更新（已执行，2026-10-23；ARCHITECT 复核通过，超时项被 OD-ARCH-071 作废）

**背景（用户实测反馈）：** 工作记录/项目跟进正常（ttfb 2.8–5s），但文案写作中模型发起联网检索、用户授权后界面卡住 3 分钟以上。台账证据 omni-21ca40e9：ttfb 2.86s、183.6 秒后落 unknown（timeout）、pending_approvals=0。诊断（REVIEW.md 2026-10-23 条目）：①工具执行期间前端不渲染 tool_activity，界面零活动可见性；②DSH 的非 approval 类 waterfall 询问（钩子/其他权限）被适配层静默忽略，引擎等应答 forever；③180 秒总超时对“搜索+抓取+长文”偏短，超时后无重试提示。三件事叠加造成“看起来死机”。

**目标：** 让工具执行期间界面可见、可理解、可恢复；把“引擎问了但没人答”从静默挂起变成显式提示。

**影响范围：** engine/omnidesk_engine/web_api_adapter.py（waterfall 处理、超时策略）；desktop/src/shell/HomeChat.tsx（tool_activity 渲染、unknown 态文案与重试入口）；desktop/src/types.ts（如需扩展 payload 类型）；测试与证据目录。

**实施步骤：**
1. **前端渲染 tool_activity：** HomeChat.tsx 事件订阅新增 tool_activity 分支，在生成中的助手消息下方显示“正在调用工具：{tool_name}（{status}）”，任务结束自动清理。不改数据结构，不加新组件文件。
2. **waterfall 静默忽略改为显式进度：** _on_frame 对非 approval 的 waterfall 事件发一条 PROGRESS（kind:"engine_ask"，附 event 原文类型），前端显示“引擎在等待一项未被识别的确认（{event}），如长时间无响应请取消重试”。不自动应答、不假装已答。
3. **超时策略调优：** writing/project 场景 default_timeout 180→300 秒（配置化，读 omnidesk.model-settings.v2.json 新字段 task_timeout_seconds，缺省 300）；超时落 unknown 时事件 note 附“已等待 N 秒；可点取消后重发或降低工具使用”。
4. **unknown 态界面文案：** 任务落 unknown 时，生成中的消息尾部显示“引擎在 {N} 秒内未返回结果，本次未自动重试”，提供“取消”按钮（已有）与“重发”按钮（复用现有发送路径）。
5. **回归测试：** 既有 pytest/cargo 全量回归；新增单测覆盖 waterfall engine_ask 事件映射与超时 note 文案。

**验收标准：**
- 工具执行期间界面显示工具名与状态（合成端点可测）。
- 非 approval waterfall 事件在界面可见，不再静默。
- 超时从 180 秒变为可配置（默认 300），unknown 态有明确文案与重发入口。
- 全量回归通过；runtime/ 零差异。

**风险：** waterfall 事件类型枚举来自 DSH 上游，若未来新增类型需继续“显式忽略 + 可见提示”策略，不自动应答未知询问（安全边界）。

**验收后：** 用户按 docs/user-testing-checklist.md 重跑文案写作 + 联网检索场景，确认体验改善。

---

### OD-ARCH-070：DSH 插件生态装载 + 上游错误透传（已执行，2026-10-23；ARCHITECT 复核通过，超时项被 OD-ARCH-071 作废）

**背景（用户对比实测 + 本机核查）：** OmniDesk 隔离环境的 web profile 只装 dsh-base + dsh-web-app，无任何搜索插件；用户原生 DSH 桌面 profile 装 dshmarket + dsh-free-search + dsh-cost-meter + web-search-deepseek patch，同题搜索 8 分 43 秒成功且质量高。OmniDesk 同题 300 秒超时零产出；原生 DSH 无插件版 3 分钟后 400「Content Exists Risk」（OpenDesign 网关内容过滤，非 OmniDesk bug）。**根因：配置差距，不是架构缺陷。**

**目标：** 让 OmniDesk 的隔离 DSH 具备与用户已验证环境相同的插件能力，且上游真实错误不再被吞。

**影响范围：** desktop/src-tauri/src/engine_supervisor.rs（profile 引导）、engine/omnidesk_engine/web_api_adapter.py（错误透传）、samples/profiles/（出厂资产）、~/OmniDesk/runtime/dsh-home/profiles/web/（运行时落盘）；不动 runtime/ 源码。

**实施步骤：**
1. **web profile 装载插件：** 在 OmniDesk 隔离 dsh-home 的 web profile 安装 dsh-free-search（免费多引擎，无 key 可用）与 dsh-cost-meter；pnpm 依赖写入该 profile 的 package.json（与用户 desktop profile 同版本）；cordis.patch.yml 增加 web-search-deepseek patch（apiKeyEnv DEEPSEEK_API_KEY，maxUses 20，用户后续可自行配 key 或用免费引擎）。dshmarket 视依赖体积与离线可用性决定，不阻塞本步骤。
2. **profile 资产化：** 出厂副本 samples/profiles/web.profile.json + web.profile.patch.yml（与 discipline.rules.json 同模式）；启动时若运行时 profile 缺失则从出厂副本落盘，已有则不覆盖（用户可改）。
3. **上游错误透传：** web_api_adapter.py 的 _frame_error/_raise_follow_error 把「HTTP 400 + code=upstream_error + message」作为 error_message 完整落台账（截断至 300 字符），事件发 task_failed 而非 unknown/timeout；前端 unknown/failed 均显示错误原文摘要。
4. **超时与搜索匹配：** 检测到任务使用搜索类工具时，task_timeout_seconds 建议值提升（读配置字段 search_task_timeout_seconds，默认 600）；不影响非搜索任务默认 300。
5. **回归与证据：** 全量 pytest/cargo 回归；证据落 tests/verification/desktop/evidence/od-arch-070/（profile 装载清单 + 错误透传单测 + 合成端点回归）。

**验收标准：**
- OmniDesk 隔离环境对话中模型可用 web_search（free-search 引擎回退链），无需用户配 key。
- 上游 400 错误在台账与界面可见完整原因（非 unknown/timeout）。
- 搜索类任务超时默认 600 秒，其他 300 秒。
- runtime/ 零差异；已有用户 profile 不被覆盖。

**风险：** dsh-free-search 引擎可用性随外网波动（已有回退链）；插件版本与 DSH 0.1.7-rc.1+ 的兼容性以实跑为准；DEEPSEEK_API_KEY 由用户自行配置，软件不读不存。

**验收后：** 用户在 OmniDesk 重跑“政治监督沿革”搜索题，对照原生 DSH 结果评估质量与耗时。
- **OD-ARCH-071（已归档）：** 用户直接指令——取消任务自动超时（任务终态由底座决定，用户手动取消；详见 STATUS.md 顶部与 REVIEW.md 复核，均通过）。
- **数据分析功能：** 继续延后（用户明确），取得使用证据与授权进展后再立项。

---

### OD-ARCH-072（合并版）：能力管理分页 + 首页信息架构改版（待批准，2026-10-23）

> 本版合并原 073（首页改版）并新增构建修复步骤；073 计划撤销，内容并入本计划。

**背景：** OD-ARCH-070 已把插件绑定进 profile（出厂播种、不覆盖用户改动），但应用内没有管理入口：Skill 只能改目录文件，插件只能手改 profile。用户选定方案 B，并要求兼容 DSH 原生插件安装方式。核查确认：DSH 原生安装通道即 dshmarket bundle（用户 desktop profile 已用 ^1.66.35 装过 multi_search/anysearch），可直接复用。

**目标：** 设置页新增「能力管理」分页，集中查看和管理 Skill 与插件；提供「接入 DSH 插件市场」开关；同时完成首页信息架构五项改版。修复当前「能力清单：Command capability_view not found」——根因已查明：用户桌面启动的是 10 月 19 日构建的 release 旧程序，该命令注册于其后，旧程序未注册所致（前端 dist 为 10-23 21:41 新构建；debug 二进制为 21:39 新构建；release 二进制停在 10-19 08:18）。

**影响范围：** desktop/src/SettingsPanel.tsx（新增分页）；desktop/src-tauri（新增 2–3 个只读/受控写命令）；~/OmniDesk/runtime/dsh-home/profiles/web/{package.json,cordis.patch.yml}（受控写）；不动 runtime/ 源码；不碰 .dsh / .agents / deepseek-harness / ~/.dsh。

**实施步骤（Part A：能力管理）：**
1. **只读列出能力清单：** Skill = 扫描 ~/OmniDesk/runtime/dsh-home/skills/ 目录（名称、一行说明（如有）、最近修改时间，不造假版本号）；插件 = 读 profile package.json 的 bundles + dependencies（名称与版本）。
2. **插件启用/停用：** 停用 = 从 bundles 移除该项（dependencies 保留），写前自动存 .bak；启用 = 加回 bundles。改动需重启 DSH 生效，界面注明「重启后生效」。仅允许写 ~/OmniDesk/runtime/dsh-home/profiles/web/ 两个文件，复用 seed_profile_if_missing 同款路径硬拒绝校验，原子写入。
3. **「接入 DSH 插件市场」开关：** 开 = bundles 加入 dshmarket（版本对齐用户 desktop profile 的 ^1.66.35）；关 = 移除。市场内的浏览/安装/更新由 DSH 原生界面承担，OmniDesk 不复制其功能。
4. **Skill 只读展示 + 打开目录：** 提供「打开 Skill 目录」按钮（宿主 open 目录能力已有先例），不做在线编辑器。

**实施步骤（Part B：构建与部署修复）：**
5. **重建并重启：** npm run build + cargo build --release（或按项目现有打包命令）；确认 release 二进制含 capability_view/capability_set_plugin/capability_market/capability_open_skills 注册（strings 或运行验证）；用户桌面启动方式指向新构建。此为用户报错的直接修复。

**实施步骤（Part C：首页信息架构改版，原 073 内容）：**
6. **品牌与 Profile 分离：** TopNav 左侧固定「OmniDesk」软件名；其后新增 profile 徽标（样式与软件名明显区分，如小胶囊/描边风格），默认显示「无 · 标准模式」，点击弹层列出可用 profile（当前已有：无/discipline；弹层预留后续扩展）；选择后确认弹窗说明「重启引擎生效」，确认后走现有 profile 切换链路。
7. **日程节律悬浮：** ThreeWeekCalendar 卡片从常驻布局移除；改为点击 TopNav 日期区域显示悬浮卡片，鼠标移开 1 秒后淡出；不改日历数据源。
8. **删除今日重点推进卡片：** HomeCanvas 移除卡片及相关代码路径；待办条目改由欢迎语「欢迎回到 OmniDesk」下方展示，仅显示一条，多条时逐条轮动（间隔约 5 秒，淡入淡出）；无待办时不占位。
9. **小机器人移位：** 从当前位置移到对话框上方、历史记录列表左侧；具体锚点随对话框落位联动。
10. **对话框落位：** 无对话历史时输入框垂直居中于首页中部（参考 DSH 原版首页）；产生历史后自动落到底部（历史区上方）；动画平滑（如 200ms），不改消息发送逻辑。

**回归与证据：** cargo/pytest/tsc/build 全量；新增单测覆盖 bundles 增删与路径拒绝；桌面 e2e 与截图脚本适配新布局后复跑；证据落 tests/verification/desktop/evidence/od-arch-072/（能力清单 + 插件启停 + 市场开关 + 首页四态截图：初始、有历史、悬浮日历、profile 弹层）。

**验收标准：**
- 设置页能看到全部 6 个 Skill 与当前 4 个 bundle（含版本）。
- 停用 dsh-cost-meter → 重启 → 启动日志无 dsh-cost-meter；启用 → 恢复。
- 打开市场开关 → 重启 → dshmarket bundle 出现在启动日志/界面；关闭 → 移除。
- 所有写操作有 .bak 备份；对 .dsh/.agents/deepseek-harness 的写入被硬拒绝；runtime/ 零差异。
- 桌面启动的不再是 10-19 旧 release；能力清单正常显示（不再报 Command not found）。
- 标题栏「OmniDesk」+ profile 徽标可点击切换、默认「无 · 标准模式」；日历悬浮；无今日重点卡片、待办轮动；机器人位于对话框上方历史左侧；对话框初始居中、有历史落底。
- 全量回归通过。

**风险与注意事项：**
- dshmarket 体积较大，打包阶段再评估是否默认带出；当前按需开关。
- 社区插件纪律不变：一次装一个、装完跑回归、核查来源与权限。
- 改 profile 仅动 bundles，不改 dependencies 与 cordis.patch.yml 内容，避免与 070 出厂播种逻辑打架（播种只在文件缺失时触发，用户已有文件不被覆盖）。
- 首页改版涉及布局重构，注意窗口失焦/最小化时暂停轮动计时器，避免无效渲染。
- profile 切换沿用现有链路，弹层交互与现有引擎状态冲突时以现有状态机为准，不重写。

**预计规模：** 一个 BUILDER 批次（合并后约 1–1.5 天）；完成后等 ARCHITECT 复核 + 用户批准提交推送。

---

### OD-ARCH-074：引擎进程生命周期修复（孤儿回收 + 所有权登记 + 实例互斥）（已执行，ARCHITECT 复核通过，2026-10-24；双组偏差认可，见 REVIEW.md 与 docs/decisions.md D-029）

**背景（Luna 审计 + ARCHITECT 根因核查）：** 宿主 spawn python 引擎、引擎 spawn node DSH（孙进程，未设进程组）。正常收尾链完整（Drop→terminate→atexit→杀 node），但异常路径（引擎被 SIGKILL、宿主崩溃）会让 node 成孤儿：继续监听端口、持有 dsh-home，历次测试累积过 22 个（约 3.5GB）。无所有权登记与实例互斥，多引擎可并发写同一 dsh-home（已实测干扰账本）。

**目标：** 引擎进程树在任何退出路径下都不留孤儿；同数据根同时只有一个活跃引擎；崩溃后下次启动能安全回收遗留进程；不误伤用户自己的 DSH。

**影响范围：** engine/omnidesk_engine/{web_api_adapter.py,acp_adapter.py,serve.py}（进程组 + 所有权文件）、desktop/src-tauri/src/{engine_supervisor.rs,engine_ipc.rs}（启动前回收 + 收尾兜底）、desktop/src-tauri/src/lib.rs（RunEvent 退出钩子已有，补异常路径）；不改协议、不改架构、不动 runtime/。

**实施步骤：**
1. **进程组绑定：** 引擎 Popen node 时设 start_new_session=True（POSIX 进程组）；AC P 适配器同理；引擎自身启动时（serve.py）也建组。清理时用 os.killpg 组杀，覆盖整棵进程树。
2. **所有权登记：** 引擎启动时在数据根写 owner.json（pid、进程组 pgid、boot_id（/proc/sys/kernel/random/boot_id 防 PID 跨启复用）、宿主侧无）、通道、时间戳、命令行指纹；引擎正常退出时删除。宿主启动引擎前读该文件核验身份（pid 活着 + pgid 匹配 + boot_id 一致 + 命令行含 omnidesk_engine），全部匹配才认定「遗留 OmniDesk 引擎」。
3. **启动前回收：** 宿主 spawn 引擎前执行回收：身份核验通过 → killpg(SIGTERM) → 等待 ≤5s → 不退则 SIGKILL → 删除 owner.json；核验不过（含文件缺失、字段不匹配、boot_id 不同）→ 视为非我方进程，不动它，若同数据根仍有活跃进程则停止启动并明确报错（防并发写）。
4. **实例互斥：** 引擎侧启动时若发现同数据根存在身份核验通过的活跃引擎（非遗留、非自己），拒绝启动并返回明确错误；宿主展示原因。合法多实例（不同数据根）不受影响。
5. **宿主收尾兜底：** 现有 Drop 与 RunEvent::Exit 之外，在引擎侧补 SIGTERM/SIGINT 处理器（serve.py），确保信号路径也走 shutdown；宿主侧不做名字模糊扫描，只信任 owner.json。
6. **测试（隔离数据根 + 可控假引擎进程，不碰用户真实 DSH）：**
   - 正常启动/退出：owner.json 建与删、node 随引擎退出。
   - kill -9 引擎 → node 成孤儿但组回收仍可达（进程组语义）→ 再次启动时被回收。
   - kill -9 宿主 → 引擎收 SIGTERM（前台进程组）→ 走 atexit → node 清理。
   - PID 复用伪造：owner.json 写假 pid/不匹配 pgid → 拒绝回收、停止启动。
   - 合法第二实例（不同数据根）正常启动互不干扰。
   - 既有全量回归（cargo/pytest/tsc/build/e2e）。
7. **证据：** tests/verification/desktop/evidence/od-arch-074/（各场景日志与回收结果，不含凭证）。

**验收标准：**
- 任何退出路径后，系统无 omnidesk_engine/node(bin.js) 孤儿（以 owner.json 与进程组核验为准）。
- 同数据根双引擎启动被拒绝且原因可见；不同数据根不受影响。
- 用户原版 DSH（~/.dsh、DeepSeek Desktop）全程零接触、零误杀（测试断言）。
- 全量回归通过；runtime/ 零差异；不提交不推送。

**风险与边界：**
- killpg 只对「身份核验通过」的进程组执行；boot_id 防跨重启 PID 复用误杀。
- ACP 通道的取消语义不完整属另一问题（Luna 列 P1/暂缓），本任务不处理。
- 不做通用进程管理框架，owner.json 是唯一所有权事实源。

**预计规模：** 一个 BUILDER 批次（约一天）；Phase 2（通道能力说明，P1）单独成计划，待本计划验收后另行批准。
- **提交推送：** 066 + 068 + 累积的 audit/ 目录 + README 改动一并分批入库，推送一次（推送前红线扫描 + 文件清单核对）。
