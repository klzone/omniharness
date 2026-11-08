# OD-KM-509 第四阶段接口 material_query 证据

日期：2026-11-09
HEAD：836c230fb2e7c4a2c6057b1922948f68add0e7f7

## 交付范围

- 新增 `desktop/src-tauri/src/material_query.rs`：只读查询条件与回执结构、
  标题/正文本地词法匹配、任务/项目/知识关联实体定位、7 项模块测试。
- `desktop/src-tauri/src/lib.rs` 追加模块声明、`material_query` 命令与注册项。
- `desktop/src/types.ts` / `desktop/src/api.ts` 追加查询类型与调用封装。
- 未改动 501–508 已有文件的字段结构与行为；未改动 `runtime/`。

## 实跑命令与结果

### cargo test --offline --lib

```
export PATH="$HOME/.cargo/bin:$PATH"
cd desktop/src-tauri
cargo test --offline --lib
```

结果：`883 passed; 0 failed; 0 ignored`，退出码 0。
基线（PLANS.md 记录的上一批）：876 passed。本任务净增 7 项。
原始输出：`cargo-lib-test.txt`。
定向复跑：`cargo test --offline --lib material_query::tests` 为 `7 passed; 0 failed`，
原始输出：`material-query-tests.txt`。

新增测试：

- `defaults_to_confirmed_and_explicit_status_overrides`
- `filters_type_date_and_project_together`
- `keyword_matches_title_and_content_case_insensitively`
- `keyword_query_does_not_rewrite_material_files`
- `scene12_payload_contains_content_source_location_status_and_related_entities`
- `invalid_filters_are_rejected_and_broken_files_are_counted`
- `missing_related_entities_are_reported_without_failing`

场景十二断言覆盖：正文、来源四项（类型/编号/引用原文/引用位置）、审核状态、
任务/项目/知识三类关联实体；并用 `serde_json::Value` 校验序列化后的字段存在。

### npx tsc --noEmit

```
cd desktop
npx tsc --noEmit
```

结果：退出码 0，无输出。原始输出：`tsc-noemit.txt`。

## 裁决记录

- 决定：把查询逻辑放在新模块 `material_query.rs`，不向 `material_store.rs` 增加函数。
  理由：任务卡允许调用 501–508 交付文件但不改其结构；新模块只调用公开读取接口。
  错了的代价：多一个薄模块，后续第四阶段需同时维护模块边界；无数据风险。
- 决定：时间范围沿用 506 `MaterialFilter` 的 `event_date` 闭区间语义。
  理由：同一素材篮已有唯一时间筛选口径，避免查询与列表给出不同结果。
  错了的代价：若第四阶段需要按创建时间检索，需再增显式条件。
- 决定：`review_status` 省略或为空时固定为「已确认」；显式传入时按该状态过滤。
  理由：任务卡要求默认只返回已确认素材，同时保留审核页/第四阶段按状态取数能力。
  错了的代价：没有「全部状态」快捷值；调用方需按状态分别查询。
- 决定：关键词只匹配标题与正文，检索条件不落盘、不走 `gate_user_text`。
  理由：任务卡点名范围与 506 裁决 R5；查询为只读本地处理。
  错了的代价：不会命中标签或来源引用；需要时另加显式条件。

## 未做与已知限制

- 未接入第四阶段文稿生成 UI/模型调用；本卡只交付接口层。
- 未做真实窗口目视与真实模型输出验证。
- 未测数百条素材下关联实体逐条读取的性能。
- 关联实体读取失败会以 `found=false` 和原因返回，不让整条查询失败。

## 豁免声明

未新增 lint/测试豁免，未跳过或删除既有测试，未改低验收阈值。
测试数据全部为虚构样例；未改 `runtime/`；未 commit、未 push。
