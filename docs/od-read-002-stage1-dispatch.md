 ## 任务编号与名称
 
 OD-READ-002 阶段 1：健脑室（读书页改名）+ RSS 新闻聚合融合
 
 ## 获批依据
 
 - 计划位置：PLANS.md「OD-READ-002：健脑室——读书页融合 RSS 新闻聚合」。工作树内的 PLANS.md 已含该计划段落（本轮 ARCHITECT 已写入并获用户当轮授权派工）。
 - 批准时间与批准人：2026-11-18 用户当轮原话（把 RSS 融合进读书界面、读书改「健脑室」、保留划线工具栏、参考 qiaomu-rss-dsh 源码与信息源，指定派工模型与档位）。
 - 工作树：/Users/yvan/.codex/worktrees/2221/MyWorkStation（分支 od-read-002-brain-room，基于 main HEAD cb0a0b31f0）。所有改动只在这个 worktree 里做。
 
 ## 开工前检查（只读）
 
 - `git -C /Users/yvan/.codex/worktrees/2221/MyWorkStation status --short --branch` 与 `git log -1 --oneline` 快照写入 STATUS.md 追加小节；确认工作树干净（应只有刚加的 PLANS.md 段落和本任务书文件）。
 - 只读理解既有文件：`desktop/src/shell/ReadingPage.tsx`（1456 行）、`desktop/src-tauri/src/reading_store.rs`（1455 行）、`desktop/src-tauri/src/task_shell.rs` 的 read_reading_source、`desktop/src/api.ts` 读书相关段、`desktop/src/types.ts` 读书相关接口、`desktop/src/readingAi.ts`。
 - 参考项目只读借鉴：`/tmp/qiaomu-rss-dsh`（如已不在本机，`git clone --depth 1 https://github.com/joeseesun/qiaomu-rss-dsh /tmp/qiaomu-rss-dsh`）。只读，不复制代码，GPL-3.0 与本项目许可证边界遵守。
 
 ## 允许修改的文件或目录
 
 - `desktop/src/shell/ReadingPage.tsx`（更名文案、书架分组、订阅管理 UI、拉取按钮）
 - `desktop/src/shell/Dock.tsx`（读书入口 label 改「健脑室」）
 - `desktop/src-tauri/src/rss_store.rs`（**新增**）
 - `desktop/src-tauri/src/lib.rs`（注册新命令）
 - `desktop/src-tauri/Cargo.toml`（如需把 reqwest 加为直接依赖）
 - `desktop/src/api.ts`、`desktop/src/types.ts`（新增 RSS 相关接口与类型）
 - `STATUS.md`（只追加本任务小节）、`PLANS.md`（如实际实现与计划有偏差，更新对应行，不删已有段落）
 - `tests/verification/desktop/` 下新增验证脚本（如 verify_od_read_002_stage1.py）
 
 ## 明确不做的事
 
 - 不改 `runtime/`、`engine/`、模型路由或凭证。
 - 不接微信读书/GitHub 书架/乔木服务；不做播客与公众号源；不做 OPML 导入导出；不做搜索/收藏/未读过滤（书架只有分组展示）。
 - 不写知识库；不改版本号与 CHANGELOG；不提交不推送。
 - 不把 GPL 代码复制进仓库；只借鉴结构思路。
 
 ## 实施要求（按计划段落逐条落地）
 
 1. **入口更名**：Dock.tsx 中 BUILTIN_READING_ITEM 的 label 改「健脑室」；ReadingPage 内部「读书」相关用户可见文案（页头、空状态、按钮提示）同步改名。模块路径不改。
 2. **RSS 订阅存储**：新增 `desktop/src-tauri/src/rss_store.rs`；数据落 `~/OmniDesk/reading/rss/subscriptions.json`（阅读目录复用 `reading_store::resolve_reading_root` 的根 + `rss/` 子目录；复用其目录校验/创建/0755→0o700 逻辑，可以抽公共函数也可以在 rss_store 里重复实现，只要不复制粘贴超过 30 行）。schema `omnidesk.rss.v1`。字段：id（sha8）、url、name（用户可编辑）、group（预置清单里的分类，用户新加默认「我的订阅」）、added_at、last_fetched_at、last_error、read 状态不在这层（read 放文章层）。增删改查命令：`rss_subscription_add(url, name?)`、`rss_subscription_remove(id)`、`rss_subscriptions_list()`。
 3. **预置清单**：Rust 常量数组，写在 rss_store.rs 顶部（或独立 `rss_preset.rs`，BUILDER 自定）。内容：精选作者 9 条（潮流周刊/阮一峰/云风/和菜头/张鑫旭/小众软件/月光博客/Reorx/pseudoyu，URL 从 /tmp/qiaomu-rss-dsh/src/client/Discover.jsx 顶部 featured 数组取）+ 独立博客不超过 40 条（从 /tmp/qiaomu-rss-dsh/src/data/independent-blogs.json 挑 tag 含「编程」或「读书」的高质量源，总数不超过 40，控制数组源码 <20KB；文件头注释注明「来源：timqian/chinese-independent-blogs，MIT License」）。**首次启动 rss_store 时把预置清单写入 subscriptions.json（幂等，已存在的 url 不重复添加，不覆盖用户已删过的项——用 removed 集合或直接不重写已存在文件；实现方式 BUILDER 自定，但必须保证用户删除预置订阅后不会被下次启动加回来）。**
 4. **RSS 拉取与解析**：Rust 命令 `rss_fetch(subscription_id?)`：单请求 20s 超时、响应体上限 2MB、最多解析 200 条 entry、每 feed 保留最新 50 条（按 publishedAt 排序取前 50）。RSS 2.x/RDF/Atom 三格式归一化（entry 字段：key/guid/link/title/published_at/author/content_html/summary）。XML 解析手写小型解析器（含 CDATA、HTML 实体、命名空间前缀剥离），不引新 crate。网络客户端允许引入 reqwest（已在 Cargo.lock 传递依赖，加为直接依赖需启用 blocking + rustls-tls 或 native-tls 特性，选型理由写入 STATUS.md）或 ureq；不使用系统代理环境变量绕过（reqwest 默认行为即可）。失败时返回可读错误（超时/DNS/HTTP 状态/解析失败），不静默。
 5. **HTML 正文转 Markdown + 入库**：RSS 文章正文 HTML 走白名单净化 + 段落提取，转成 `ReadingChapter`（title=文章标题，paragraphs=分段后的正文）。复用 reading_store 的 `import_book_at`（构造一个临时 Markdown 文件走既有导入路径）或直接构造 `ReadingBook` 结构落盘（BUILDER 权衡，倾向于直接构造，避免临时文件）。`book_id = "rss-" + sha256(feed_url + entry_key)[0..8] + "-" + entry_key_hash`；同一 feed_url+entry_key 已存在时跳过，不覆盖。文章元数据落 `rss/articles/<book_id>.json`（含 read 布尔、fetched_at、subscription_id、原 guid/link）。`ReadingBookMeta.source_format="rss"`、`source_name=订阅名`、`author=文章作者或订阅名`。
 6. **UI 融合（ReadingPage.tsx）**：书架列表分两组：「书籍」「订阅文章」。订阅文章条目复用书籍条目样式 + 右上角小圆点（read=false 未读）。RSS 条目打开走既有 openBook（readingBook 命令）——RSS 文章在 books/ 目录下也是一份 ReadingBook，不需要新的打开路径。订阅管理：书架顶部新增「订阅」按钮，点开模态框列订阅（含预置）+ 输入框添加 URL + 每项删除按钮 + 「刷新全部」按钮。RSS 条目列表支持「拉取全部」按钮（调用 rss_fetch()）。不新增 Dock 项。
 7. **划线工具栏**：RSS 文章用与书籍一致的渲染（同一段落 data-paragraph 结构 + 划线/想法/标段/问 AI 四按钮浮条）。不改 readSelection/selection 逻辑。
 8. **AI 伴读**：RSS 文章选中文字问 AI 走既有 reading_concept；整理走 reading_organize（batch 传文章批注）。不加新 intent、不改 readingAi.ts 提示词。
 9. **隐私边界**：RSS 拉取只发 HTTP GET 到用户添加或预置清单里的 URL；不发文章正文给外部服务；AI 只发选中文本或批注（走既有 submitTask 红线扫描链路）；日志不记正文。
 
 ## UI 参考（涉及界面时必填，ARCHITECT 已代查）
 
 - 检索关键词：rss reader article list with sidebar / book reading app highlights notes
 - 参考产品与链接：My Reader AI（https://screenshots.designknowledgebase.com/Web/My%20Reader%20AI/）、Perplexity（https://screenshots.designknowledgebase.com/Web/Perplexity/）、Apple Books（https://screenshots.designknowledgebase.com/iOS/Apple%20Books/）
 - 借鉴要点（≤2 句）：左栏双分组（书籍在上、订阅文章在下）共用一条列表样式，用来源徽标与小圆点区分状态；阅读区保持单列排版与既有 4 按钮划线浮条，不因 RSS 引入双栏或新交互模式。
 
 ## 验证
 
 - `cd desktop/src-tauri && /Users/yvan/.cargo/bin/cargo test --offline --lib -- --test-threads=1` 全过（rss_store 新增测试至少覆盖：RSS2.x 解析含中文与 CDATA、Atom 解析、去重、条目上限、正文截断、预置清单幂等不复活已删项）。
 - `cd desktop && npx tsc --noEmit` 通过。
 - `cd desktop && npm run build` 通过。
 - 如新增 verify 脚本，运行并记录通过数。
 - 每项结论在 STATUS.md 标注证据等级：文件事实 / 静态检查 / 脚本运行 / 未验证。
 - 证据落盘：`tests/verification/desktop/evidence/od-read-002/stage1.md`（evidence 目录被 .gitignore 忽略属正常）。
 - **真实拉取验证（BUILDER 可做）**：用 `curl -s https://www.ruanyifeng.com/blog/atom.xml | head -20` 确认网络可达即可；不需要为每条预置源实际跑一遍（留给用户/ARCHITECT 复核时抽验）。
 
 ## 硬约束
 
 - 不修改 `runtime/` 内 DSH Runtime；不动模型路由与凭证。
 - 不接触真实业务材料、案件线索或未获准数据；测试只用合成内容与公开 RSS。
 - 不提交、不推送、不打标签。
 - 不复制 GPL 代码；借鉴结构思路即可，注释里写明「思路借鉴自 qiaomu-rss-dsh（GPL-3.0），未复制代码」。
 - 预置清单文件总大小（源码内数组+注释）<20KB。
 
 ## 交付
 
 - 改动文件清单、实际运行的测试与结果、证据路径、未验证项、计划偏差、Git 状态（worktree 内保留未提交）。
 - 完成后停下，等待 ARCHITECT 独立复核。
