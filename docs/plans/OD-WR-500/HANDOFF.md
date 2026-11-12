# OD-WR-500 批次交接（OD-WR-501～508）

日期：2026-11-11

范围：工作记录模块的原始素材、修订历史、当日摘要、时间线、周期复盘、复盘档案、AI 继续分析与收口回归。所有改动仍在工作区未提交状态，基线为 `eec28fd635`。

## 批次结论

- 501～507 的实现均已落地，并由 ARCHITECT 逐卡复检。
- 508 完成全量回归、安全静态核对与本文档；未修改实现代码。
- 回归门槛：cargo lib 1154 passed / 3 failed（仅 `process_owner` 沙箱误报）；cargo 集成测试 67 passed / 0 failed；pytest 230 passed / 4 failed（同一类 `process_owner` 沙箱误报）；`tsc --noEmit` 与 `npm run build` 均通过；新增安全脚本 21/21。
- 已发现的非阻塞问题集中登记在「已知限制」，本轮未修复。

## 501～507 各卡一句话

| 卡 | 结果 |
| --- | --- |
| OD-WR-501 | 记录创建时写入 `raw_input` / `raw_source`；补丁结构没有原始素材入口，更新只追加 `revisions.jsonl`，原始素材与旧记录的字节保持不变。 |
| OD-WR-502 | 记录详情增加只读原始输入折叠块、修订历史与「重新整理」入口；重新整理携带原文走首页对话，生成的新正文仍需用户确认。 |
| OD-WR-503 | 新增当日摘要：封闭 intent、`daily-summary` Skill、宿主汇集当天正式记录、五段与 `#wr-` 引用校验、`work/摘要/` 落盘与 `.bak` 备份。 |
| OD-WR-504 | 记录库改为按日分组时间线：组头显示日期、条数与摘要入口，卡片覆盖任务书七要素，保留搜索/筛选并兼容旧数据。 |
| OD-WR-505 | 新增周期复盘：周/月/季/年封闭词表、7/9/11/13 段模板、宿主汇集记录与任务、引用真实性校验、草稿/确认流与已确认拒绝覆盖。 |
| OD-WR-506 | 工作记录页新增「复盘」档案页：列表、周期/状态筛选、详情、引用与关联、导出 MD、草稿重新生成；记录详情增加复盘/写作反链。 |
| OD-WR-507 | 记录与复盘详情增加「交给 AI 继续分析」：跳到首页并预填对象上下文，不自动发送，也不扩大全局对话采集。 |
| OD-WR-508 | 全量回归、`verify_wr_security_508.py` 安全核对、证据归档与本交接文档。 |

## 数据流

```text
原始输入 raw_input + raw_source
        │  创建时写入（随手记 / 记录草稿），之后只读
        ▼
正式记录 work/工作记录/<record_id>.md
        │  修订：RecordPatch 不含 raw 字段
        └─ work/工作记录/<record_id>/revisions.jsonl
                 │
                 ├─ 宿主汇集「当天正式记录」
                 │        ▼
                 │  当日摘要 work/摘要/YYYY-MM-DD.md
                 │  五段 + 真实 #wr- 引用；旧版备份 .bak
                 │
                 └─ 宿主汇集「周期内记录 + 任务统计 + 既往复盘标题」
                          ▼
                   周期复盘 work/复盘/<period>/<start>_<end>.md
                   草稿 →（确认）→ 已确认 + confirmed_at
                   草稿可重生成并备份 .bak；已确认拒绝覆盖
                          │
                          ▼
                   复盘档案页（只读列表/详情/筛选/导出）
                   记录 ↔ 复盘/写作任务反向引用
```

要点：

- 摘要只汇集指定日期的正式记录，排除草稿；没有记录时必须如实写「当日无记录」，不得编造引用。
- 复盘汇集周期内正式记录与任务状态，并带上既往复盘标题作为对照；建议不自动创建任务。
- 摘要和复盘都由界面侧模型生成正文，宿主只负责结构、引用与落盘校验；AI 不直接写文件。

## 存储与接口

| 对象 | 路径 / 接口 |
| --- | --- |
| 正式记录 | `work/工作记录/<record_id>.md` |
| 修订历史 | `work/工作记录/<record_id>/revisions.jsonl` |
| 当日摘要 | `work/摘要/YYYY-MM-DD.md`，重生成备份 `*.bak` |
| 周期复盘 | `work/复盘/<period>/<start>_<end>.md`，重生成备份 `*.bak` |
| 摘要 IPC | `worklog_daily_summary_generate` / `worklog_daily_summary_read` |
| 复盘 IPC | `worklog_period_review_collect` / `generate` / `read` / `confirm` / `list` |
| 反链 IPC | `worklog_record_backlinks` |
| 记录 IPC | `work_record_create` / `read` / `update` / `worklog_revisions` |

## 安全边界

- 原始素材不可被修订覆盖：`RecordPatch` 没有 `raw_input` / `raw_source`；`update_record` 不写这两个字段；摘要与复盘模块不构造记录补丁，也不调用记录更新。
- AI 无确认态写通道：`HomeChat` / `conversationPrompt` 不直接 `invoke`；记录、摘要、复盘的写请求都经 `api.ts` 的宿主 IPC，宿主在落盘前校验。
- 复盘确认后冻结：`review_store::write_review` 对 `status=已确认` 直接拒绝；单测断言确认版字节不变。
- 引用必须真实：摘要校验 `#wr-`，复盘校验 `#wr-` 与 `#t-`，生成 IPC 路径必经 `validate_body`；不存在或段落缺引用都会拒绝落盘。
- 测试数据全部虚构：本轮新增测试使用甲、乙等合成对象与固定日期，不含真实业务材料特征词。

安全脚本：`tests/verification/desktop/verify_wr_security_508.py`，21 项全部通过；证据在 `tests/verification/desktop/od-wr-508/verify_wr_security_508.*`。

## 已知限制

1. 复盘确认目前只有宿主 IPC 与 `api.ts` 封装，档案页没有确认按钮；从界面无法把草稿标为已确认。已登记，未在本轮修复。
2. 未做真实模型质量验收：摘要/复盘的段落深度、引用选择、语言质量需要用真实模型在真实（或脱敏）材料上实测。
3. 时间线视觉未做真实窗口目视验收；现有证据是编译、构建、纯函数与静态契约。
4. 摘要引用失效只标记 `stale` / 缺号，不自动修复；复盘同理，删除记录或任务后需用户自行处理。
5. 旧记录没有 `raw_input` 时详情不显示原始素材块，「重新整理」不可用；这是兼容旧数据的预期行为。
6. 非当天日期组只读取已有摘要，不提供生成入口；生成入口只保留在当天组。
7. 既有 verify 脚本存在陈旧断言：`verify_weekly_093/097/099.py` 读取已被删除的 `WeeklyQuadrants.tsx`，无法运行；`verify_weekly_092.py` 的 `metric_uses_entry_count` 仍按旧 `ScenePage` 结构断言，28/29。它们与 501～507 改动无关，本轮未改脚本。
8. cargo 的 3 项与 pytest 的 4 项 `process_owner` 失败是受限沙箱读不到进程表导致的误报；沙箱外参考基线为 cargo 1157/0。
9. 全部改动未提交、未推送；交付前需按项目流程审查并提交。

## 用户实测项

- [ ] 真实模型生成当日摘要：分别在有 0 条、1 条、多条正式记录的日期触发，检查五段结构、引用编号、空记录声明与重生成 `.bak`。
- [ ] 真实模型生成周期复盘：覆盖周/月/季/年，检查段落深度、任务统计、引用记录/任务、既往复盘标题与空周期声明。
- [ ] 时间线视觉：检查按日分组、组头日期/条数/摘要入口、卡片七要素、搜索筛选、旧数据兜底与非当天摘要读取。
- [ ] 跳转体验：从摘要引用点开记录、从记录反链跳到复盘/写作、从记录或复盘点「交给 AI 继续分析」后确认预填不自动发送。
- [ ] 原始素材与修订：创建记录后检查原始输入折叠块；编辑正文后确认原文不变、修订历史追加；尝试「重新整理」并确认新正文仍需确认。
- [ ] 确认语义：通过宿主 IPC 确认一份复盘后，确认档案页「重新生成」被禁用、宿主拒绝覆盖；同时评估是否需要补确认按钮。

## 证据目录索引

- `tests/verification/desktop/od-wr-508/README.md`：本轮回归表、安全结果与已知失败说明。
- `tests/verification/desktop/od-wr-508/`：cargo / pytest / tsc / build 日志与退出码，18 个既有 verify 的日志与 JSON。
- `tests/verification/desktop/verify_wr_security_508.py`：批次安全静态断言。
- `tests/verification/desktop/od-wr-501/` ～ `od-wr-507/`：各卡实现与验收证据。
- `tests/verification/desktop/verify_wr_503.py` ～ `verify_wr_507.py`、`verify_wr_detail_502.py`：各卡契约验证脚本。

## 豁免声明

- 未提交、未推送、未打标签。
- 未修改 `runtime/`、知识库写路径、任务创建通道、`STATUS.md` / `REVIEW.md` / `PLANS.md` / `decisions.md`。
- 508 只新增 `tests/verification/desktop/verify_wr_security_508.py`、`tests/verification/desktop/od-wr-508/` 证据与本文档；未改实现代码与既有测试断言。
- `desktop/src-tauri/tests/profile_assets_host.rs` 的 8→10 断言由 ARCHITECT 亲自修改，508 未触碰该文件。
