## 任务编号与名称

OD-READ-001 阶段 4：读书整理稿的确定性引文核验与保存边界

## 获批依据

- 计划位置：PLANS.md OD-READ-001「阶段 4（确定性引文核验 + 整理稿边界，BUILDER 执行）」。
- 批准时间与批准人：2026-11-18 用户（深夜裁决原话「如果你觉得可行或者还有什么需要完善的，就直接修改plan，然后授权你安排子agent去执行，你负责复核」）。
- 允许修改的文件或目录：`desktop/src-tauri/src/reading_store.rs`、`desktop/src/shell/ReadingPage.tsx`、`desktop/src/readingAi.test.mjs`（如需补断言）、`STATUS.md`（仅追加本任务小节）；如确需新测试文件，放 `tests/verification/desktop/` 下并命名 `verify_od_read_001_stage4.py`。
- 明确不做的事：不改 `runtime/`、`engine/`、模型路由或凭证；不接微信读书/GitHub；不做 OCR；不写知识库；不改版本号与 CHANGELOG；不提交不推送（提交推送由后续独立任务执行）。

## 开工前检查（只读）

- `git status --short --branch` 与 `git log -1 --oneline` 快照写入 STATUS.md；工作树已有未提交改动（OD-READ-001 初步实施）必须原样保留，不得 stash/reset/clean。
- 只读 `reading_store.rs` 现有结构（ReadingBook/ReadingAnnotation/save_note_at/render_book_markdown）与 `ReadingPage.tsx` 的 `saveOrganized`/`saveAiAnswer`。

## 实施要求

按 PLANS.md 阶段 4 五条逐条实现：

1. `reading_store.rs` 新增 `verify_organized_markdown(book, markdown) -> Vec<String>` 纯函数（公开给 lib.rs 或经 pub(crate)），提取整理稿中 `>` 引用行，规范化（trim、连续空白压成一个空格、去引号包裹）后与批注 quote、批注所在段落全文比对；返回未核验引文（最多 20 条，超过部分合并为一条「……另有 N 条」）。`save_note_at` 调用它，未核验 > 0 时在保存稿顶部插入提示块 `> ⚠ 引文核验：N 条未能在本书批注或原文中找到；本稿未通过核验，请人工复核。`（不阻断保存）。
2. `save_note_at` 在保存的 Markdown **没有** frontmatter（不以 `---` 开头行）时补写 5 字段 frontmatter（title/created/book/source/annotation_count，YAML 安全转义）；已有 frontmatter 原样保留。
3. `ReadingPage.tsx`：AI 整理结果 `organized` 进入展示/保存前，若整段被单个 Markdown 围栏（```` ``` ```` 或 `~~~`）包裹则剥去该层（只剥一层，内部围栏不动）；整理稿保存成功后，界面回执显示「已核验引文 x 条 / 未核验 y 条」（从保存函数返回值取数：给 `ReadingSavedNote` 增加 `verified_citations: usize` 与 `unverified_citations: Vec<String>` 字段，序列化到前端）。
4. Rust 单测（放 `reading_store.rs` tests 模块）：核验函数全部命中 / 部分未命中 / 中文引文空白差异 / 无引文；frontmatter 补写与不改写；现有 `bookmarks_thoughts_update_delete_progress_and_export` 等测试不回退。
5. 前端纯函数（剥围栏）若有独立实现，补进 `readingAi.test.mjs`。

## UI 参考（涉及界面时必填）

- 本次改动不新增界面结构，只改保存回执文案，不适用 UI 参考检索。

## 验证

- `cd desktop/src-tauri && /Users/yvan/.cargo/bin/cargo test --offline --lib -- --test-threads=1` 全过。
- `cd desktop && npx tsc --noEmit` 与 `npm run build` 通过。
- `cd desktop && node --experimental-strip-types --test src/readingAi.test.mjs` 通过。
- 每项结论在 STATUS.md 标注证据等级：文件事实 / 静态检查 / 脚本运行 / 未验证。
- 证据落盘：`tests/verification/desktop/evidence/od-read-001/stage4.md`（evidence 目录被 .gitignore 忽略，属正常）。

## 硬约束

- 不修改 `runtime/` 内 DSH Runtime；不动模型路由与凭证。
- 不接触真实业务材料、案件线索或未获准数据；测试只用合成内容。
- 不提交、不推送、不打标签。
- 不改 `desktop/src/types.ts` 以外的前端类型；`ReadingSavedNote` 字段新增保持 serde 兼容（`#[serde(default)]`）。

## 交付

- 改动文件清单、实际运行的测试与结果、证据路径、未验证项、计划偏差、Git 状态（保留未提交）。
- 完成后停下，等待 ARCHITECT 独立复核。
