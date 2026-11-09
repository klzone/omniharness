# OD-WT-606 大纲生成与确认 IPC 证据

- 日期：2026-11-09
- 基线 HEAD：`ab000dfb01`（工作区含 601/602/603/604/605 既有未提交改动，全部原样保留）
- 证据等级：文件事实（源码）、脚本运行（Rust 测试 / pytest / tsc）

## 实跑命令与结果

| 命令 | 结果 | 日志 |
| --- | --- | --- |
| `export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib` | 985 passed, 0 failed, exit 0 | `cargo-lib.log` |
| `/tmp/omnidesk-pytest-venv/bin/pytest tests -q` | 216 passed, 30 subtests passed, exit 0 | `pytest.log` |
| `cd desktop && npx tsc --noEmit` | exit 0 | `tsc.log` |

本轮新增测试 19 项（966 → 985），覆盖：模板默认值四套与校验、无文件零写入、
用户配置读取与拒绝坏版本、十二类文稿模板映射、大纲结构校验、
素材/知识引用必须来自任务 `selected_*`、`expected_words` 正数、
章节编号唯一、场景四各章目标/重点/素材映射、提示词内容、
生成状态与空素材包/未确认素材拒绝、Markdown 往返、
确认落盘 `outline.md` 与 outline 快照、两段状态迁移、
已写作状态拒绝覆盖、未确认读取报错、三条 IPC 的闸门顺序。

## 改动文件

- `desktop/src-tauri/src/writing_outline.rs`（新增）
- `desktop/src-tauri/src/lib.rs`（追加模块声明、三项命令与实现、闸门测试）
- `samples/workflows/writing-templates.v1.json`（新增四套类型模板）
- `desktop/src/types.ts`（追加大纲类型）
- `desktop/src/api.ts`（追加三项封装与类型导入）

## 未验证事项

- 真实模型生成大纲的章节质量与 JSON 稳定性。
- 真实窗口中的大纲编辑器交互（留待 OD-WT-611）。
