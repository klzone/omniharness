# 执行状态

## OD-ARCH-173 阶段 1：提示词装配（2026-11-17）

- 改动 `samples/profiles/discipline.profile.patch.yml`：移除顶层 `web-search-free` insert；`discipline-core` / `work-review-core` 的 persona prefix 换成 OmniDesk 台账助手一句；删除检索预算与 `advanced_search` 等工具选择规则；`tool-web` 保留 `fetch`、关闭 `search`；`preset-standard` 未动，作为回退基线。
- 未改 `desktop/src` TS/TSX、`engine/`、`runtime/`、`omnidesk-base`、模型路由与凭证。
- mock 复测（非真实模型）：system 提示词 5396 → 2983 字符（去掉 PTC 测量），`web-search` 段 1919 → 0，纪律规则 552 → 283；保留 PTC 时为 15252 字符，PTC 类型注入占 12098 字符，阶段 1 的 ≤1000 tokens 在保留 PTC 时不可达。
- 日期位置：`dsh-time-context` 以 `user/message`（`time-context`）注入，不在 system 段；会话元数据 `time-context=261` 字符。未改代码。
- 搜索入口：`desktop/src` 无 free-search/web-search UI 开关；缺口是 `discipline.profile.json` 与 `profile_assets.rs::DISCIPLINE_PLUGINS` 仍列 `dsh-free-search`（不在本阶段获批范围，未改）。
- 验证：`OMNIDESK_PYTHON=/usr/bin/python3 ~/.cargo/bin/cargo test --offline --lib` → 1187 passed / 0 failed；未设该环境变量时 3 个 spawn 测试因 `python3` PATH 权限失败（环境问题）。未改 TS，`npx tsc --noEmit` 未跑。
- 证据：`tests/verification/desktop/evidence/od-arch-173/stage1.md` 与同目录三个 mock 日志、`cargo-test-lib.log`。该 evidence 目录已被 `.gitignore` 忽略，并曾于 2026-11-17 被归档；本任务从 `MyWorkStation-archive/2026-11-17/test-and-process-evidence/` 取回 od-arch-173 后继续追加，未恢复其余截图。
- 未提交、未推送、未打标签。

## 证据与历史报告全面归档（2026-11-17）

- 将 `tests/verification/**/evidence/`、`tests/verification/desktop/od-*/`、`tests/reports/`、`docs/archive/`、`docs/plans/` 与 `audit/` 移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/test-and-process-evidence`，共 37 个目录、1065 个文件、约 11 MB，并生成 `MANIFEST.sha256`。
- Git 保留全部可复跑测试脚本、夹具、架构契约、数据政策与当前决策；测试运行产物改为本地生成并忽略。
- `.gitignore` 从「忽略 evidence 图片」收窄为「忽略整个 evidence 目录与 `od-*` 目录」。
- `engine/README.md` 与 `docs/ARCHITECTURE.md` 的历史报告链接改为归档说明；`tests/verification/EVIDENCE-ARCHIVED.md` 记录归档位置。
- 全树扫描已无个人路径与旧 Profile 名（许可证署名除外）；Rust 定向测试 6/6、91/91 通过；GitHub 历史尚未重写，未提交未推送。

## 公开仓库个人资料清理（2026-11-17）

- 将个人发言稿、个人写作风格库、个人数据根配置和初始化脚本移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/personal-files`，共 15 个个人资料文件，另将 `_design` 7 个设计参考文件移出，并生成 `MANIFEST.sha256`。
- 移除个人工作区、个人数据根配置、个人初始化脚本和旧 `knowledge/writing-style/`。
- `knowledge/writing-style/` 重建为 5 个通用模板，不含个人作品、案例或身份信息。
- 开发 Profile 目录改为中性名称 `profiles/omnidesk-dev/`，包名改为 `dsh-profile-omnidesk-dev`；开发启动脚本与隔离自检同步改名。
- `scripts/setup.sh` 去掉个人数据初始化步骤；`data-template/` 与 `docs/ARCHITECTURE.md` 改为通用外部数据根说明。
- 全树扫描已无个人路径与旧 Profile 名（许可证署名除外）；`bash -n` 通过；GitHub 历史尚未重写，未提交未推送。

## 测试截图归档（2026-11-17）

- 将 2026-11-17 之前入库的 1087 张测试截图与任务截图移出 Git，归档到仓库外 `MyWorkStation-archive/2026-11-17/test-evidence-screenshots`，并生成 `MANIFEST.sha256`。
- 归档体积约 152 MB；当时保留 603 个已跟踪的文字证据，同日 D-103 已将文字证据与历史报告一并移出；测试脚本与夹具保留。
- `.gitignore` 已加入 evidence 与 `od-*` 目录的图片、视频扩展名，后续测试截图不再入库。
- `AGENTS.md` 的证据保留规则同步改为「文字证据与测试脚本入库，截图和视频默认仓库外归档」。
- 未改产品代码，未运行构建；未提交未推送。

## 授权变更（2026-11-16）

- 根 `LICENSE` 改为 PolyForm Noncommercial 1.0.0，并加入项目版权与 Required Notice；新增 `COMMERCIAL-LICENSE.md`、`THIRD-PARTY-NOTICES.md`。
- `README.md` 的 License 小节、`desktop/package.json` 的 `license` 字段、`CHANGELOG.md` 与 `docs/decisions.md` 的 D-100 已同步。
- `runtime/` 未改；DeepSeek Harness 继续按上游 MIT（Copyright (c) 2026 DeepSeek）分发。
- 验证：许可证文本与 SPDX 官方文本逐字包含；package.json 可解析；`git diff --check` 通过；diff 无 `runtime/` 路径。未改代码，未运行构建。
- 未提交未推送。

> 更早的执行记录已归档到仓库外 `MyWorkStation-archive/2026-11-17/process-docs/`。
