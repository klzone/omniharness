## 任务编号与名称

OD-READ-001 提交与推送：读书阅读笔记分页（初步实现 + 阶段 4 引文核验）

## 获批依据

- 计划位置：PLANS.md OD-READ-001「派工方式（本轮用户已授权）」第 3 条。
- 批准时间与批准人：2026-11-18 用户（「授权你安排子agent去执行，你负责复核，最后再交由子agent将项目提交至github并推送最新版本」；ARCHITECT 已完成阶段 4 复核并写入 REVIEW.md 通过结论）。
- 允许的动作：`git add`（本清单文件）、一次 `git commit`、一次 `git push origin main`。
- 明确不做的事：不做 force push、不打标签、不改 VERSION/CHANGELOG/tauri.conf 版本、不 rebase、不 amend、不动 runtime/ 与 engine/。

## 开工前检查（只读）

- `git status --short --branch` 与 `git log -1 --oneline` 写入 STATUS.md 追加小节；HEAD 必须仍是 `b8b9357b9c`（如已被推进，停下报告，不得自行处置）。

## 实施要求

1. 精确加入以下文件（只加这些，不用 `git add -A`/`git add .`）：
   - `PLANS.md`、`REVIEW.md`、`STATUS.md`、`docs/decisions.md`、`docs/OmniDesk-Desktop-Data-Layout.md`、`docs/od-read-001-stage4-dispatch.md`、`docs/od-read-001-commit-dispatch.md`
   - `desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/task_shell.rs`、`desktop/src-tauri/src/reading_store.rs`
   - `desktop/src/App.tsx`、`desktop/src/api.ts`、`desktop/src/main.tsx`、`desktop/src/types.ts`、`desktop/src/shell/Dock.tsx`、`desktop/src/shell/ReadingPage.tsx`、`desktop/src/reading.css`、`desktop/src/readingAi.ts`、`desktop/src/readingAi.test.mjs`
   - `scripts/reading_attachment_extract.py`
2. 提交信息（第一行 + 正文，原样使用）：
   ```
   feat(reading): OD-READ-001 读书阅读笔记分页（划线/标记/想法 + AI 整理与概念问答）

   - Dock 新增「读书」内置入口；书架、阅读器、笔记/AI 侧栏（懒加载）
   - 导入用户显式选择的 md/txt/pdf/docx/epub（32 MiB 上限；EPUB/纯文本走 stdlib helper，PDF/DOCX 复用既有抽取链路）
   - 批注按章节+段落+code point 偏移+引文锚定，落盘前复核并拒绝歧义引文；进度单独落盘
   - AI 解释/查概念与批注整理只产草稿，用户确认才写 ~/OmniDesk/reading/notes/；整理稿经确定性引文核验（未核验插提示块）并补 frontmatter
   - 数据与决策：docs/OmniDesk-Desktop-Data-Layout.md §5.2、docs/decisions.md D-111；REVIEW.md 两轮复核记录
   - 验证：cargo test --lib 1232/0/1；tsc 0；build 0；readingAi.test.mjs 5/5
   ```
3. `git push origin main`；推送失败时原样报告错误，不重试第二次、不改推送目标。

## 验证（提交前必跑）

- `git diff --check` → 0
- `cd desktop/src-tauri && /Users/yvan/.cargo/bin/cargo test --offline --lib reading_store -- --test-threads=1` → 全过
- `git status` 确认暂存区只含清单内文件；发现清单外改动（如 HomeChat.tsx 等）一律不加，并在报告里列出。

## 硬约束

- 不读、不打印、不提交任何凭证或真实业务材料；提交前用 `git diff --cached` 目视核对无密钥、无个人正文（如有可疑内容，停止并报告）。
- 提交与推送各只执行一次；失败即停。

## 交付

- 提交哈希、推送回执、`git status` 快照、清单外文件的说明，写入 STATUS.md 追加小节；然后停下等待 ARCHITECT 终检。
