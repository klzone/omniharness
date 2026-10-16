# ARCHITECT 审查记录

## OD-BUILD-002：桌面应用数据根拒绝仓库内落点

**验收结论：通过（本计划范围）。** OD-BUILD-001 遗留的 R4 漏项已关闭：桌面应用数据根在词法路径和解析后的实际落点两侧检查 OmniDesk 仓库边界；`ensure()` 在创建目录之前重新执行隔离校验；回归用例覆盖普通仓库路径、软链接指向仓库、软链接指向仿真禁区、仓库外路径和拒绝后无仓库残留。

### 审查基线与独立验证

- 审查时 HEAD：`17efd20be01b2a01cacf11f9523ccff302acce61`。工作区保留此前全部修改、删除和未跟踪内容；暂存区为空。`desktop/` 是未跟踪目录，普通 `git diff` 不会包含它；因此以实际源码、测试、BUILDER 状态记录进行审查，并将该可审计性限制保留在记录中。
- 独立运行 `cargo test --offline`：**60 passed，0 failed**（44 单元、13 宿主集成、3 E2E；doc-tests 0）。Rust 使用已配置的用户级工具链及本机 Tauri 依赖前缀。
- 独立运行 `npm run build`：TypeScript 检查与 Vite 构建通过（22 modules）。
- `git diff --check` 与 `git diff --cached --check` 通过。由于实现目录未跟踪，这两项不能替代其源码审查。
- 未运行真实模型、未读取凭证或真实工作材料，未修改 `runtime/`。

### 按验收条件复核

- **词法仓库路径拒绝：通过。** `assert_outside_repo_boundary()` 先对词法归一和原始路径做前缀比较；普通仓库路径回归测试覆盖。
- **符号链接真实落点拒绝：通过。** `assert_isolated()` 先调用 `resolve_real_path()`，仓库边界函数再对数据根真实落点与仓库根解析结果做组件级 `starts_with`；符号链接指回仓库测试用词法干净的别名路径。
- **禁区软链接与正常外部路径：通过。** 仿真 `.dsh` 真实落点被既有真实路径禁区检查拒绝；干净仓库外临时根通过。
- **创建前拒绝：通过。** `AppPaths::ensure()` 首先调用 `assert_isolated()`，失败映射为 `PermissionDenied` 后才会执行目录创建；回归测试确认仓库内目标路径不存在。
- **错误处理边界：通过但记录一项非阻断限制。** 数据根真实落点解析失败即拒绝；仓库根解析失败会退回词法归一比较。实现当前会优先读取运行时 `OMNIDESK_REPO_ROOT`，其值必须被当作可信部署配置；若启动者可任意将它设为与实际仓库无关的位置，就能改变此边界检查所依据的仓库根。该实现沿用本计划要求的 `task_shell` 仓库根约定，适用于当前自用开发场景，但不构成抵御可控制进程环境者的安全边界。未来若把此检查作为对不可信启动环境的硬性保护，应另立计划，将仓库根固定或验证为可信路径。

### 剩余限制

- `resolve_real_path()` 与 `create_dir_all()` 之间仍有检查后路径替换竞态；没有使用目录句柄和禁止跟随链接的原子创建方案。本计划明确未要求消除该风险，因此不阻断本次验收，但不得把它描述成可抵御同机恶意并发替换。
- 符号链接行为仅在当前 Linux 环境验证；Windows/macOS 及真实 Tauri 窗口仍未验收。
- 本次验收只关闭 OD-BUILD-002 和 OD-BUILD-001 R4 的仓库边界遗漏，不代表整个个人 MVP 已可接入真实工作材料。启发式红线扫描不构成完整准入或合规控制；须继续遵守经用户转述的用途和不得分发边界。

### 结论与后续

OD-BUILD-002 在其计划范围内通过。当前没有已批准或执行中的下一项实施计划；[PLANS.md](PLANS.md) 仅记录后续待评估领域，不授权 Builder 开始工作。

---

## OD-BUILD-001：修复个人 MVP 的保存确认、来源真实性与处理边界

**验收结论：不通过（R4 尚未完整满足）。** R1–R3 通过本轮源码审查；R4 的任务产物路径检查已处理词法路径与解析后的实际落点，但桌面应用数据根 `AppPaths` 未验证仓库边界，仍能将数据目录指向仓库。MVP 继续保持未验收，仅可使用合成数据和许可范围内公开数据。

### 审查基线与证据

- 当前分支 `main`，HEAD `17efd20be01b2a01cacf11f9523ccff302acce61`。本轮开始时工作区 181 项，暂存区为空；沿用并保留所有现有差异。`desktop/` 源码文件目前为未跟踪文件，因此常规 `git diff` 不包含这些文件，也无法从 Git 基线确定 BUILDER 单独引入的逐行差异。本次以实际文件、BUILDER 状态记录、回归用例及 `git diff --no-index` 的文件内容为审查证据；此限制降低改动归属的可审计性。
- 已独立运行 `cargo test --offline`（使用仓库已有的用户级 Rust/Tauri 依赖）：55 passed，0 failed（39 单元、13 宿主集成、3 任务 E2E）。
- 已独立运行 `npm run build`：TypeScript 检查与 Vite 构建通过。`git diff --check` 通过，但不覆盖未跟踪文件。
- 本轮未调用真实模型，未读取凭证、真实工作材料或修改 `runtime/`。

### 按发现项复核

#### R1 — 通过（按本计划定义的显式确认合同）

- `SaveArtifactRequest.user_confirmed` 是必需的反序列化字段；缺失时 JSON 解析失败，`save_artifact()` 对 `false` 拒绝；注释不能代替布尔值。前端保存请求传入该值。
- `save_requires_explicit_confirmation`、`save_request_rejects_missing_confirmation_field` 与宿主边界回归测试均通过。
- 限制：布尔值表示调用方声明已确认，不是对本地 IPC 调用者身份或真实鼠标操作的密码学证明。本轮满足 OD-BUILD-001 的接口合同，不应将其表述为抵御同机恶意调用者的安全认证。

#### R2 — 通过

- 可保存来源只允许 `user` 和 `model_unverified`；`model` 与客户端提供的 `task_state` 路径已移除，前端没有“已验证模型”选项。测试拒绝 `model` 来源并允许明确标为未复核的来源。
- 可信模型结果仍未与真实引擎终态绑定，状态如实标为未实现。

#### R3 — 通过（限已识别的启发式命中）

- 宿主分析命令使用 `analyze_input()`；分析、解释提示词和保存入口均先执行同一红线扫描闸门。命中用例不会返回统计结果或提示词，错误不回显命中原文；干净合成输入统计指纹保持一致。
- 扫描器仍是启发式，不能识别全部身份、岗位或工作细节，也不等于授权或合规判断。

#### R4 — 不通过（P1）桌面应用数据根缺少仓库边界拒绝

- `desktop/src-tauri/src/app_paths.rs:106-121` 的 `AppPaths::assert_isolated()` 检查词法路径及 `resolve_real_path()` 的解析落点，并拒绝若干 DSH/agent 禁止目录；但没有检查真实落点是否位于 OmniDesk 仓库内。该模块也没有指向仓库或模拟禁区的符号链接回归测试。
- `desktop/src-tauri/src/lib.rs:284-290` 的 `build_state()` 在 `AppPaths::resolve()` 后调用 `ensure()` 创建目录。由于仓库根本身不包含 `FORBIDDEN_FRAGMENTS` 中的路径片段，把 `OMNIDESK_APP_DATA_ROOT` 设为指向仓库的干净别名（或仓库内普通目录）可通过当前隔离校验，随后 `ensure()` 会在仓库内创建 `ui-state/`、`logs/` 等目录。
- 产物/使用记录侧的 `task_shell::assert_outside_repo()` 已检查解析后的落点，且测试覆盖软链接指向仓库；但它不能替代 `AppPaths` 自己的数据根校验。两类数据根边界不一致，OD-BUILD-001 的“所有数据根入口”要求未满足。
- 证据等级：静态源码审查。尚未运行桌面启动级攻击复现；无需通过真实用户数据验证即可确认漏掉仓库边界比较。
- 修复要求：桌面应用数据根必须对词法路径及解析后的真实落点都执行仓库外检查；`AppPaths::ensure()` 不得在未通过此检查的路径上创建目录；新增普通仓库路径、软链接指向仓库、软链接指向仿真禁区、普通仓库外路径的测试。

### 当前适用限制

- OD-BUILD-001 的 R4 未通过前，MVP 仍未验收；不得因 R1–R3 通过而扩大真实材料使用。真实 Tauri 窗口验收、真实模型解释质量及跨平台行为仍未验证。
- BUILDER 报告中的 55 项结果经本轮复跑确认；独立复跑未执行 `cargo build --offline`，构建覆盖由 `cargo test --offline` 和已报告的构建记录分别支持。

### 下一步

`PLANS.md` 已拟定 OD-BUILD-002，限于补齐桌面应用数据根的仓库边界检查及回归测试，当前待用户批准。

---

## OD-ARCH-001：个人 MVP 独立审查与文档基线收敛（历史审查）

## OD-ARCH-001：个人 MVP 独立审查与文档基线收敛

**验收结论：MVP 审查不通过；OD-ARCH-001 文档工作完成。** 个人 MVP 当前不可作为允许粘贴工作材料的日常工具。R1–R4 均为 P1，须由单独获批的 BUILDER 计划修复并复验。当前结果仅支持合成数据与明确许可范围内的公开数据测试；用户转述的有限许可不得扩大解释，且不得分发。

### 审查基线与验证

- 仓库分支：main；审查时 HEAD：17efd20be01b2a01cacf11f9523ccff302acce61。
- 本轮开始时 Git 状态：175 项（11 项已修改、164 项未跟踪，暂存区为空）。所有原有差异均保留；本轮文档调整以外的改动未重置、覆盖或提交。
- 已检查 desktop/src-tauri/src/lib.rs、task_shell.rs、app_paths.rs、desktop/src/workbench.tsx、API/types 及相邻测试。
- 既有实测：cd desktop && npm run build 通过；cd desktop/src-tauri && cargo test --offline 49/49 通过（34 单元、13 集成、2 E2E）。这说明已有测试断言通过，不代表下列安全路径已满足产品要求。
- 未运行真实模型调用，未使用真实工作材料，未读取凭证或修改 DSH Runtime；真实 Tauri 窗口视觉验收和跨平台验收未完成。

### 阻断发现

#### R1 — P1：Rust 保存边界未强制人工确认

- 证据：desktop/src/workbench.tsx 的 95 行、304 行以 UI 状态 confirmed 控制按钮；desktop/src-tauri/src/task_shell.rs 的 SaveArtifactRequest 655–666 行没有确认布尔字段；save_artifact() 680–683 行及其校验没有确认状态。Tauri 保存命令 task_save_artifact（desktop/src-tauri/src/lib.rs 228–233 行）直接接受该请求。
- 影响：绕过 UI 直接调用已注册命令即可保存未确认产物；可选备注不能证明用户确认。
- 证据等级：静态代码审查；此前测试未覆盖该绕过。
- 修复要求：Rust 保存入口必须要求显式确认，缺失或 false 均拒绝；增加直接保存函数/命令路径测试。

#### R2 — P1：解释来源与模型成功状态可由 UI 伪造

- 证据：desktop/src/workbench.tsx 217–237 行在保存时自行提交 explanationSource 和 taskState；desktop/src-tauri/src/task_shell.rs 652–665 行允许 model 与调用方提供的 task_state，695 行只按字符串 succeeded 判断，未绑定真实引擎任务或结果。
- 影响：手写文本可被记为模型成功结果，来源记录不可信。
- 证据等级：静态代码审查；当前测试不能证明 UI 所报状态来自引擎。
- 修复要求：删除客户端可伪造的 model/succeeded 路径，只允许 user、model_unverified；同步 UI 和产物记录。可信模型来源另行设计。

#### R3 — P1：分析和提示词入口未统一执行红线闸门

- 证据：desktop/src-tauri/src/lib.rs 195–215 行的 task_run_analysis() 收集 red_line_scan() 命中后仍调用 deterministic_analysis()；desktop/src-tauri/src/task_shell.rs 624–646 行的 build_explanation_prompt() 未调用 assert_input_allowed()。保存入口 701 行才调用闸门。
- 影响：已被扫描器识别的输入仍可在分析及提示词生成路径中被处理。扫描本身为启发式，不能证明完整识别个人信息或合规性。
- 证据等级：静态代码审查；未用真实业务输入测试。
- 修复要求：所有处理输入的 Rust 入口统一先执行闸门，命中时不返回统计或提示词；补直接调用测试，并明确提示扫描局限与准入责任。

#### R4 — P1：禁区检查未验证规范化后的真实路径

- 证据：desktop/src-tauri/src/task_shell.rs 287–305 行 normalize_path() 在 canonicalize() 失败时退回词法归一；322–338 行 assert_local_root_allowed() 只检查词法路径字符串；341–352 行仓库边界检查虽比较规范化路径，但未将该路径再交给禁区校验。保存前 710–725 行创建根后仍按传入路径做词法禁区检查。独立桌面路径 desktop/src-tauri/src/app_paths.rs 105–138 行同样只检查根路径字符串。
- 影响：符号链接指向禁区或仓库时存在绕过风险。
- 证据等级：静态推断；未实际尝试符号链接绕过，不把风险写成已复现攻击。
- 修复要求：所有数据根入口在读写前校验解析后的实际落点；无法安全解析时默认拒绝；用临时符号链接指向仿真禁区和仓库的测试覆盖两类路径。

### 其他验收限制

- 真实 Tauri 窗口曾在虚拟显示环境呈空白；浏览器或 Vite 预览不能替代桌面窗口验收。
- 真实模型解释质量未验证；跨平台未验证。
- 仓库未保存单位授权原件。现有许可范围仅来自用户转述，限于满足所述脱敏条件、用于个人数据分析与生态研判且不得分发；不得据此推定周报、材料写作、项目管理、模型服务商、工具、设备或运行环境均已获准。

### OD-ARCH-001 文档基线收敛验收

- 当前协作基线集中在仓库根目录 AGENTS.md、GOALS.md、PLANS.md、STATUS.md、REVIEW.md 与 docs/decisions.md；README.md 提供索引。
- 五份旧路线/阶段计划已移至 docs/archive/ 保留，不作删除。归档说明将它们标为历史快照；详细契约、审计报告、阶段结果和运行证据保留在 docs/ 与 tests/ 下。
- 活动文档未发现对已归档规划文件的规范性引用。报告中作为当时记录的旧文件路径保留为历史事实。
- 验证：当前 Git 状态重新清点为 181 项，暂存区为空；空白差异检查通过；runtime/ 无差异。本轮没有修改业务代码，因此未重跑代码测试。
- 结论：OD-ARCH-001 的文档收敛部分完成。旧归档内部的历史相对链接不保证都可用；归档文件不作为当前项目状态来源。

### 下一步

PLANS.md 中的 OD-BUILD-001 是独立的待批准 BUILDER 修复计划。获批之前不执行代码修改；该计划通过后仍须由 ARCHITECT 基于真实源码差异与测试独立复核。
