# OD-ARCH-081 界面验收证据

采集方式：一次性 Xvfb 显示 `:87` + 私有 dbus 会话总线 + XTEST 真实指针/键盘事件 + 窗口截图。
应用数据根为 `/tmp/od081-home`（一次性，每次运行重建）；前端复用同仓库 `127.0.0.1:5273` 开发服务（只读，不启停）。
结构化判定见同目录 `home-fixes-facts.json`（`checks` 全部为 `true`，`gate_errors` 为空，`passed` 为 `true`）。

| 文件 | 对应验收 | 判定依据 |
| --- | --- | --- |
| `01-home.png` | S1 基线 / S2 品牌标 | 问候「夜深了 · 欢迎回到 OmniDesk」与其下待办行同时可见；输入区「+」左侧为 O+D 品牌标，无旧 spark |
| `02-pointer-in-chat.png` | S1 指针移入对话区 | 与 01 的问候条带（y 158–187）逐像素比较，`changed_pixels = 0` |
| `03-input-focused-typing.png` | S1 聚焦并输入文字 | 同上比较，`changed_pixels = 0`（输入 `synthetic focus check`） |
| `04a-before-mark-click.png` / `04-after-mark-click.png` | S2 品牌标不承担动作 | 点击前后顶层窗口数均为 1，上部区域变化像素 0，仅光标微动 |
| `05-markdown.png` | S3 助手正文 Markdown | 表头+两行数据、表格横线 ≥3 条、代码块底色面板 27375 像素、列表圆点 54 行、代码块右上出现「复制」 |
| `06-user-markdown.png` | S3 用户问句 Markdown | 深色气泡内出现 2 个独立列表圆点（`(1083,336)`、`(1083,365)`），粗体生效 |

边界：本目录只含合成文本（`合成甲/合成乙`、`synthetic-code-line-*`），无真实业务材料、无凭证。
同一轮记录的 `source_before/after`、`user_dsh_before/after`、`user_agents_before/after` 三组指纹一致，说明隔离运行未触碰原版 DSH 数据根。
截图中的「先配置一个模型端点」提示来自未配模型的隔离实例，属预期，不是缺陷。

证据等级：真实桌面窗口运行 + 像素结构判定 + 数据根文件事实 + 源码静态核对。未做 OCR，也未执行真实模型生成任务。
