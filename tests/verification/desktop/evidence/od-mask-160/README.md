# OD-MASK-160 证据（2026-11-15）

范围：出站脱敏自然化（160a）、人名/机构自动识别增强（160b）、出站硬阻断（160c）、
启动期词典模板（160d）、设置页词典编辑（160e）。全部只使用虚构样例。

## 验证结果

| 项目 | 命令 | 结果 | 证据 |
| --- | --- | --- | --- |
| Python unittest | `cd tests/verification/engine-interface/adapter && python3 -m unittest` | 239/239 OK | `unittest.txt` |
| Rust lib tests | `cargo test --offline --lib -- --test-threads=1` | 1181/1181 OK | `cargo-test-lib.txt` |
| 前端构建 | `npm run build`（tsc --noEmit + vite build） | 成功 | `npm-build.txt` |
| 出站流向审计 | `python3 tests/verification/desktop/verify_masker_audit_142.py` | 15/15 | `audit-142.txt` |
| 全链路 e2e | `python3 tests/verification/desktop/verify_e2e_143.py` | 掩码/阻断相关 9/9；脚本整体 10/12 | `e2e-output.txt`、`e2e-facts.json` |

说明：Rust 默认并行跑 `cargo test --offline --lib` 时，3 个既有子进程测试偶发
`Permission denied`（`engine_bridge` 2 项、`engine_ipc` 1 项）；单项与串行运行均通过。
本任务新增的 `sensitive_dict` 4 项测试在并行/串行下都通过。

## e2e 覆盖

- 词典人名「林青禾」出站替换为自然掩码「林**」，出站请求无原文；
- 假模型回文「林**」经流式与非流式都还原为「林青禾」；
- 含虚构阻断词「示例阻断词甲」的提交返回 `invalid_argument`，且假模型请求增量为 0；
- `knowledge_ingest_e2e_143`：知识整理提示词同样得到自然掩码并能还原归档。

## 两个与本次无关的既有失败（未修改，供复核裁定）

1. `verify_worklog_089.py` 的 `note_has_index_data` 断言查找「当周已有记录」；当前
   `desktop/src/conversationPrompt.ts` 的实际文案是「已有记录（最近 14 天…）」。HEAD
   中同样不存在该字符串，属既有断言漂移。
2. `verify_weekly_093.py` 读取 `desktop/src/shell/WeeklyQuadrants.tsx`；该文件在 HEAD
   中已不存在（`git show HEAD:...` 返回 128），属既有重构后未同步的旧脚本。

## 口径与限制

- 映射仍只存在内存/返回值，不写日志、不落盘；设置页保存的只有词表本身。
- 机构识别是纯标准库正则：无词典时按后缀命中，并把名称前缀收敛到紧邻后缀的最后 2 个字，
  避免把「今天去」这类前文一起吞入；单字后缀「行」对进行/运行等常见动词尾字做了窄口径放行。
  仍可能误拦或漏拦真实机构名，属已登记的精度边界。
- 未验证：Windows 实机上的启动期建文件、设置页保存与真实模型请求；真实业务语料的误拦率。
- 设置页布局借鉴（OD-ARCH-035）：参考 Shopify 设置卡（标签在输入框上方、分组卡片）与
  TypeWhisper 设置行（重复行 + 行尾动作）；实现沿用现有 binding 行的直角、细分隔线
  与深浅色 token，不新增视觉组件或依赖。
