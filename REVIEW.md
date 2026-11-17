# GitHub 历史重写与发布清理审查（2026-11-17）——通过（附外部副本限制）

# OD-ARCH-173 0.2.30 发布验收（2026-11-17）——通过（附未验证项）

**审查对象：** PLANS.md OD-ARCH-173 全部阶段执行记录与证据（tests/verification/desktop/evidence/od-arch-173/）、release 提交 1d07d91bba、远程 main ac46822eff、实际源码 diff。

**逐阶段复核（源码 diff + 测试输出，非自述）：**

1. **阶段 0 基线：** diagnose-omnidesk.py 新增只读会话解析；12 个真实历史会话确认 system 5396 字符中 web-search 段占 1919、Skill 目录 2518；397 秒拆分缺原始会话，如实标未验证。
2. **阶段 1 提示词：** web-search-free 从 discipline patch 移除，persona 换台账助手底座；mock 复测 web-search 段归零、去 PTC 口径 5396→2983（−44.7%）；PTC 与预算冲突经裁决保留 PTC（D-105 记录取舍）。
3. **阶段 2 Skill：** 先用 mock 探针验证 tool-skill 全文可达（原生 + PTC run_code 两路均验证），才改 conversationPrompt；四动作 user 段降 61–72%；3 个旧断言脚本随阶段 6 更新后通过。
4. **阶段 3 工具面：** dsh-free-search 依赖从 profile.json 与 profile_assets.rs 清除；输出上限显式钉住（read 2000 行/50KB、grep 250 条/单行 500 字节、glob 100）；钩子/审批/abort 审计完成，缺口记入 tool-matrix.md。
5. **阶段 4 预检：** save() 前置档位校验（6 单测）；401/402/403 阻止保存、404 提示放行；切换路径模型目录校验显式档位；聊天期才暴露的 402/403 如实记录为缺口。
6. **阶段 5 诊断页：** diagnostics_metadata 只读命令 + 设置页「配置解释」页；7 新单测；真实窗口截图因锁屏未成，黑图证据保留。
7. **阶段 6 发布：** tsc/build/cargo check/tauri build --no-bundle 全过；cargo test 1200 全过；54 个验证脚本 41 过、13 失败均复现于 HEAD 之前快照或属环境限制；版本 0.2.30 三处同步；CHANGELOG 收口；提交 1d07d91bba 已推送 origin/main。

**边界核对：** runtime/ 未改（全程）；凭证未读取/未打印；无 force push、无标签；未使用真实业务材料。

**未验证项（不阻塞验收，需后续确认）：**

- 真实模型 smoke test（Excel/周报/知识整理）：账户 402 余额不足，无法执行；提示词结构变化已由 mock 验证，真实模型是否先读 Skill 再输出待额度恢复后实测。
- Windows 安装包构建：本机无环境。
- verify_skill_flow_injection_108.py 真实窗口运行：macOS 无 Xvfb。
- 配置解释页真实窗口目视：锁屏未成，由用户日常使用确认。
- 已有安装 Profile 不会自动移除 dsh-free-search：需在 Profile 资产页手动「恢复出厂内容」或重装。

**结论：** 通过。0.2.30 代码、版本号、变更日志已提交推送（1d07d91bba → origin/main ac46822eff）；macOS 侧 tauri build 产物已验证，可发布安装包。

**审查对象：** 重写后的 `main`/标签、当前树、远端 `main`、29 个旧 Release 与仓库外备份。

**核对结果：**

1. 重写前 bundle 完整，`git bundle verify` 通过，约 480 MB。
2. `git filter-repo` 完成重写；个人工作区、设计参考、测试证据、历史报告与旧计划在全部历史路径中命中 0。
3. 当前树个人路径/旧 Profile 名扫描命中 0（许可证署名除外）；工作树中的本次清理改动已提交。
4. 远端 `main` 为 `bf7c7fd836`，全部标签已强推；旧 Release 29 个已删除，GitHub Releases 当前为 0。
5. Rust 定向测试 6/6、91/91 通过；本次未跑完整构建。

**限制：**

- 所有提交哈希已改变，已有 clone 必须重新 clone。
- 旧标签仍存在但指向重写后的提交；旧 Release 下载已不可用。
- GitHub fork 或平台缓存可能仍保留旧对象，非本仓库能强制清除。
- `.git` 仍约 513 MB，主要是上游历史与大文件；当前树已不含个人资料。

**结论：** 通过。0.2.30 需从清理后的源码重新构建安装包并发布。

# 证据与历史报告全面归档审查（2026-11-17）——通过（当前树；历史待重写）

**审查对象：** `tests/verification/**/evidence/`、`tests/verification/desktop/od-*/`、`tests/reports/`、`docs/archive/`、`docs/plans/`、`audit/`、`.gitignore` 与归档说明。

**核对结果：**

1. 移动 37 个目录、1065 个文件，归档到 `MyWorkStation-archive/2026-11-17/test-and-process-evidence`，体积约 11 MB，并生成 SHA-256 清单。
2. `tests/verification` 下已无 `evidence` 目录与 `od-*` 目录；测试脚本与夹具保留，运行时会本地重新生成证据。
3. `tests/reports`、`docs/archive`、`docs/plans`、`audit` 已移出；历史报告链接改为归档说明。
4. `.gitignore` 改为忽略整个 evidence 与 `od-*` 目录，避免后续截图、日志和任务证据再次入库。
5. 全树扫描已无个人路径、旧 Profile 名或旧用户名（许可证署名除外）。Git 历史中的旧个人文档和证据仍在，需单独重写。

**结论：** 当前树通过。归档位置 `MyWorkStation-archive/2026-11-17/test-and-process-evidence`；历史清理另行执行。

# 公开仓库个人资料清理审查（2026-11-17）——通过（当前树；历史待重写）

**审查对象：** 个人工作区、个人数据根配置、个人初始化脚本、个人写作资产、开发 Profile 目录、`data-template/`、`docs/ARCHITECTURE.md`、`scripts/setup.sh`、`scripts/omnidesk-dev.sh`、`scripts/omnidesk-isolation-check.sh`。

**核对结果：**

1. 15 个个人资料文件与 7 个 `_design` 设计参考文件已移出工作树，归档到 `MyWorkStation-archive/2026-11-17/personal-files`，清单记录 SHA-256、大小与相对路径。
2. 个人发言稿所在 `workstations/` 已整体移出；个人写作风格库已替换为 5 个不含个人内容的通用模板。
3. 个人数据根配置与初始化脚本已移出；`data-template/` 不再引用个人配置，`scripts/setup.sh` 不再调用个人初始化脚本。
4. `profiles/omnidesk-dev/` 已改为 `profiles/omnidesk-dev/`，包名与开发启动默认值、隔离自检断言同步更新。
5. 未改产品业务逻辑；`bash -n` 通过；改名后的 Rust 定向测试 6/6、91/91 通过。GitHub 历史仍包含旧个人文件，待单独重写。

**限制：** 当前只清理工作树；Git 历史中的个人发言稿、旧写作风格和截图仍可被检出。要满足“GitHub 历史也干净”，必须重写历史并强制推送，这会改变所有提交哈希并影响已有 clone，尚未执行。

**结论：** 当前树通过。归档位置 `MyWorkStation-archive/2026-11-17/personal-files`；历史清理另行执行。

# 测试截图归档审查（2026-11-17）——通过（同日由 D-103 扩展）

**审查对象：** `tests/verification/desktop/evidence` 与 `tests/verification/desktop/od-*` 下的截图、`.gitignore`、`AGENTS.md`、`CHANGELOG.md`、`docs/decisions.md`、`STATUS.md` 与归档说明。

**核对结果：**

1. 移动 1087 张截图与任务截图，归档目录共 1088 个文件（含 1087 行 `MANIFEST.sha256`），归档体积约 152 MB；工作区 evidence 目录中图片数量为 0。
2. `tests/verification` 从约 165 MB 降到约 13 MB；`tests/verification/desktop/evidence` 保留 603 个已跟踪文字证据文件，本地另有 56 个被忽略的日志/证据文件。
3. `.gitignore` 对 evidence 与 `od-*` 目录的图片、视频扩展名生效；`git check-ignore --no-index` 抽样两条路径均命中。
4. `AGENTS.md` 的证据规则改为「文字证据与测试脚本入库，截图和视频默认仓库外归档」，与 D-101、STATUS 和归档说明一致。
5. 未改产品代码，未运行构建；`git diff --check` 通过。工作区有 1087 个截图删除，未提交未推送。

**限制：**

- `.git` 仍约 542 MB；这次只移出当前树，历史中的截图、视频和重复 schema 未清理。要缩小 clone 体积必须重写历史并强制推送，未执行。
- 历史 `STATUS.md`、`REVIEW.md`、`PLANS.md` 以及部分 dispatch 文档仍引用已归档的 PNG，链接会失效；当前决策记录与用户测试清单已改为引用文字证据，归档说明已登记。

**结论：** 当时通过。归档位置为 `MyWorkStation-archive/2026-11-17/test-evidence-screenshots`；同日 D-103 将文字证据与历史报告也移出 Git。

# 授权变更审查（2026-11-16）——通过（文件级）

**审查对象：** `LICENSE`、`COMMERCIAL-LICENSE.md`、`THIRD-PARTY-NOTICES.md`、`README.md`、`desktop/package.json`、`CHANGELOG.md`、`docs/decisions.md`。

**核对结果：**

1. `LICENSE` 的主体是 SPDX 官方 PolyForm Noncommercial 1.0.0 文本，未改动条款；顶部加入 `Copyright (c) 2026 Yvan` 与 `Required Notice`。
2. `THIRD-PARTY-NOTICES.md` 明确列出 DeepSeek Harness 的 MIT 许可证、版权、锁定 tag 与提交；`runtime/` 未被重新授权。
3. `README.md` 写清自有代码与 runtime 的分层授权、商业授权入口，以及 0.2.30 的生效边界和 0.2.29 及更早版本的 Apache-2.0 历史授权。
4. `desktop/package.json` 增加 SPDX 标识 `PolyForm-Noncommercial-1.0.0`，JSON 解析通过。
5. `COMMERCIAL-LICENSE.md` 只描述申请与授权范围，明确商业授权不自动授予，具体条款以书面协议为准。

**验证：** 许可证文本与 SPDX 官方文本逐字比对通过；`git diff --check` 通过；本次 diff 无 `runtime/` 路径。未改代码，未运行构建。

**结论：** 文件级授权变更通过。未提交、未推送。

**未验证：** GitHub 的许可证识别结果、安装包内的第三方许可证清单是否完整、商业授权合同的实际法律条款。

> 更早的审查记录已归档到仓库外 `MyWorkStation-archive/2026-11-17/process-docs/`。
