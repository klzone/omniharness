 ## 任务编号与名称
 
 OD-READ-002 阶段 1 提交与推送：健脑室（读书页改名）+ RSS 新闻聚合融合
 
 ## 获批依据
 
 - 计划位置：PLANS.md OD-READ-002「执行与复核结果」段（REVIEW.md 复核结论：通过）。
 - 批准时间与批准人：2026-11-18 用户当轮原话「确认通过，你安排子agent提交并推送吧，我在Windows环境下测试新版本看看实际效果」。
 
 ## 开工前检查（只读）
 
 - `git -C /Users/yvan/.codex/worktrees/2221/MyWorkStation status --short --branch` 与 `git log -1 --oneline` 快照写入 STATUS.md 追加小节。
 - HEAD 必须仍是 `cb0a0b31f0`（如已被推进，停下报告，不得自行处置）。
 - 分支必须是 `od-read-002-brain-room`（本次提交推到该分支，不直接推 main）。
 
 ## 允许的动作
 
 - `git add`（仅下方清单内文件）
 - 一次 `git commit`
 - 一次 `git push origin od-read-002-brain-room`（不 push main、不合并、不 rebase、不打标签）。
 
 ## 暂存清单（只加这些，不用 git add -A / git add .）
 
 - `PLANS.md`、`REVIEW.md`、`STATUS.md`
 - `desktop/src-tauri/Cargo.toml`、`desktop/src-tauri/Cargo.lock`
 - `desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/reading_store.rs`、`desktop/src-tauri/src/rss_store.rs`
 - `desktop/src/App.tsx`、`desktop/src/api.ts`、`desktop/src/shell/Dock.tsx`、`desktop/src/shell/ReadingPage.tsx`、`desktop/src/types.ts`
 - `docs/od-read-002-stage1-dispatch.md`、`docs/od-read-002-stage1-commit-dispatch.md`（本文件）
 - `tests/verification/desktop/verify_od_read_002_stage1.py`
 
 **绝不暂存**：`runtime/dsh-home-seed`、`runtime/dsh-runtime`、`runtime/pnpm`、`runtime/python-vendor`（4 个指向主仓库的构建符号链接）；发现其他清单外改动一律不加并在报告里列出。
 
 ## 提交信息（第一行 + 正文，原样使用，经 git commit -F 保留多行）
 
 ```
 feat(reading): OD-READ-002 阶段 1 健脑室（读书页改名）+ RSS 新闻聚合融合
 
 - Dock 读书入口与页面文案改为「健脑室」；书架分「书籍」「订阅文章」双分组
 - 新增 rss_store：订阅存储（~/OmniDesk/reading/rss/）、49 条预置源（9 位精选作者 + 40 条独立博客，MIT 归属）、手写 RSS 2.x/RDF/Atom 解析、HTML 净化分段、去重不覆盖批注、20s 超时/2MB 上限/每源 50 条
 - lib.rs 注册 6 个 RSS 命令（订阅增删查、拉取、文章列表、标记已读）；reqwest(blocking+native-tls) 直接依赖
 - RSS 文章沿用既有阅读器与划线/想法/标段/问 AI 浮条；AI 走既有 reading_concept / reading_organize 链路
 - 参考项目：qiaomu-rss-dsh（GPL-3.0，只读借鉴思路，未复制代码）；思路与实现细节见 docs/od-read-002-stage1-dispatch.md
 - 验证：cargo test --lib 1243/0/1（OMNIDESK_REPO_ROOT 绕过 worktree 路径误伤）；tsc 0；build 0；verify 42 项全过；git diff --check 0
 ```
 
 ## 提交前必跑
 
 - `git diff --check` → 0
 - `cd desktop/src-tauri && OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation /Users/yvan/.cargo/bin/cargo test --offline --lib rss_store -- --test-threads=1` → 全过
 - `git status` 确认暂存区只含清单内文件（`git diff --cached --name-only` 与清单逐项比对）。
 
 ## 硬约束
 
 - 不读、不打印、不提交任何凭证或真实业务材料；提交前用 `git diff --cached` 目视核对无密钥、无个人正文。
 - 提交与推送各只执行一次；失败即停，不重试第二次、不改推送目标、不做 force push。
 - 不改 VERSION/CHANGELOG/tauri.conf 版本号（发版由用户决策）。
 - 不删改 REVIEW.md / STATUS.md 既有内容（只追加本任务小节）。
 
 ## 交付
 
 - 提交哈希、推送回执、`git status` 快照、清单外文件的说明，写入 STATUS.md 追加小节；然后停下等待 ARCHITECT 终检。
