# OD-WT-607 分章节写作 IPC 证据

- 日期：2026-11-09
- 基线 HEAD：ab000dfb01（工作区含 601-606 既有未提交改动，全部原样保留）
- 证据等级：文件事实（源码）、脚本运行（Rust 测试 / pytest / tsc / npm build）

## 实跑命令与结果

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib | 1004 passed, 0 failed, exit 0 | cargo-lib.log |
| /tmp/omnidesk-pytest-venv/bin/pytest tests -q | 216 passed, 30 subtests passed, exit 0 | pytest.log |
| cd desktop && npx tsc --noEmit | exit 0 | tsc.log |
| cd desktop && npm --prefix . run build | built, exit 0（既有大 chunk 提示，非本次改动） | npm-build.log |

本轮新增测试 19 项（985 → 1004），覆盖：正文空值与标题行拒绝、
正文仅为引用拼接拒绝、素材/知识引用必须来自任务 selected_* 集合、
四分法 kind 白名单与 JSON 未知字段拒绝、正文段落往返与全文占位、
场景五（章节独立确认/读取/全文组合/未确认占位/引用追加/全部确认迁移待核验）、
倒序确认仍按大纲顺序落盘、全部确认只迁移一次、未确认大纲与错误状态拒绝生成、
未确认素材拒绝进入章节提示词、提示词含该章素材/知识/上下章摘要/四分法/禁止清单、
修订提示词段落定位与原文快照保留、修订类型/章节/段落越界拒绝、
四条 IPC 的写作编号/正文/引用原文/标注文本/修订说明闸门顺序。

## 改动文件

- desktop/src-tauri/src/writing_section.rs（新增）
- desktop/src-tauri/src/lib.rs（追加模块声明、四条命令与实现、章节稿闸门、IPC 测试）
- desktop/src/types.ts（追加章节稿/提示词/确认/修订类型）
- desktop/src/api.ts（追加四条封装与类型导入）
- tests/verification/desktop/od-wt-607/（本证据目录）

## 裁决记录

- Ruling: 章节素材按大纲该节 material_ids 取；若该节映射为空，回退任务 selected_material_ids。理由：避免大纲未填映射时章节提示词无任何事实依据；错代价：提示词可能比严格映射多带本章外素材，用户仍可在确认前核对。
- Ruling: 未确认章节不写入正文.md，全文组合时才放「（本章待写作）」占位。理由：避免占位被解析成已确认正文；错代价：正文文件本身不是完整全文，613 导出必须调用 compose_full_text。
- Ruling: 修订应用复用 writing_section_confirm，未知章节先校验再写；修订后再次确认会追加一条本章引用记录。理由：603 记录是追加式历史，不做覆盖；错代价：多次修订会在 citations.jsonl 留下多条历史引用，609 需按最新正文处理。

## 未做与已知限制

- 真实模型生成章节正文的质量、JSON 稳定性与长文稿上下文占用未验证。
- 章节编辑器界面（生成/确认/修订入口）留待 OD-WT-611。
- 全文组合的 DOCX/PDF/HTML 呈现留待 OD-WT-613。
- 修订后的引用记录是否应清理旧版本引用，未在本卡定义，留待 608/609 联调确认。

## 豁免声明

- 未新增 lint/测试豁免，未跳过或删除既有测试，未降低验收阈值。
- 未修改 runtime/，未新增模型通道，未提交/推送/打标签，未改动 STATUS.md、REVIEW.md、PLANS.md、docs/decisions.md。
