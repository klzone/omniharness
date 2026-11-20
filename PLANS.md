# 当前实施计划

### OD-HARDEN-2043R：加固与改进任务书（汇总版，T1–T14）（待用户批准执行，2026-11-20）

> **触发：** 用户下发合并后的总任务书（原 A/C/B/E/D + G1/G2/G3 + F1/F2/F3 + 新项）。本轮只整理范围与现状，待批准后按批次执行；批准不扩大本节范围。
> **开工基线（2026-11-20）：** HEAD `2046d12d33c874131ba20319cf1b8686f7b401ad`，工作区 25 处既有未提交改动（前一轮 A/C/B/E 与 D 第 1 步），保留不动。`lib.rs` 15046 行、274 处 `#[tauri::command` 文本、277 个命令定义；`Cargo.toml` `rust-version = 1.77`；`release.yml` 顶层 `permissions` 仍在第 9 行；`desktop/package.json` 无 `test` 脚本，`src` 下 8 个 `.test.mjs`。

**已完成、只需按本任务书验收标准核对（不重做）：**
- **T1（原 A）**：`test` job + `pull_request` 触发 + `needs` 依赖图 + 缓存已落地（STATUS 2026-11-20 任务 A）。遗留：T12 完成后把 `npm run test` 接入 test job。
- **T3（原 C）**：`net_guard.rs`、RSS 逐跳 3 跳 + 禁止降级 + `resolve` 钉扎、媒体固定公网档、上限 8 MiB、内网开关、D-108 记录均已落地（STATUS 任务 C）。
- **T4（原 B）**：注册表 + `host_pick_files`/`host_pick_directory` + 八个命令授权门 + 四个前端入口 + `dialog:allow-open` 移除 + 11 项凭证黑名单 + 拖拽宿主登记均已落地（STATUS 任务 B）。遗留：Windows 实机与真实窗口手工清单未验证。
- **T8（原 E）**：写作子进程 10 秒超时、docx/epub 实体全文扫描、9 处非锁 unwrap 处理均已落地（STATUS 任务 E）。
- **T14（原 D）部分**：基线脚本 `tests/verification/desktop/export_commands_2043.py` 与基线 JSON 已建立，阅读/RSS 11 条命令已迁到 `src/commands/reading.rs`，迁移前后命令集合均为 277 且一致（STATUS 任务 D）。剩余六个领域未迁。

**待执行任务与该轮做法（按批次）：**
1. **T2（发布令牌与 Action 固定）**：`release.yml` 顶层改 `contents: read`，仅 `publish` 声明 `contents: write`；全部 `uses:` 固定 40 位 SHA（从各仓库对应 Release 标签查得，行尾注明原标签）；`dtolnay/rust-toolchain` 改为固定版本并与 `rust-version = 1.77` 对齐（若固定工具链无法编译现有代码，先报告再定版本，不擅自放宽）；新增 `.github/dependabot.yml`（github-actions 每周）。
2. **T5（导出目录并入授权）**：`export_engine.rs::resolve_target_dir` 要求注册表命中或位于成果根；`target_path` 改父目录 `canonicalize` 后比较；在 `is_safe_id` 调用处补注释。新增单测：未授权拒绝、授权放行、含 `..` 的 writing_id 仍拒、md/docx/pdf/html 四种既有导出测试全过。
3. **T6（Windows 打开链接）**：`platform.rs` 把「程序名 + 参数」抽成纯函数；Windows 分支改 `rundll32.exe url.dll,FileProtocolHandler`，目标用 `arg` 传。单测跨平台断言程序名不是 `cmd` 且 `%USERPROFILE%` 原样作为单参数。
4. **T7（引擎适配层）**：F1 启动后 DSH 输出改 `deque(maxlen=200)` 并继续读空管道；F2 `_parse_announcement` 只接受 `127.0.0.1`/`localhost`/`::1`，拒绝信息不含令牌。单测覆盖 10 万行保留上限、回环通过、非回环拒绝且 detail 无令牌。
5. **T9（会话缓存只存元数据）**：先读 `readLocal/writeLocal` 与 `MIGRATED_KEY` 迁移逻辑；确认迁移标记置位后清除旧键里的 `messages`，未迁移先迁移。单测（T12 后）断言 localStorage 无正文、未迁移样本先迁移后清理。
6. **T10（依赖可复现）**：DSH 运行时生成并提交 `runtime/dsh-runtime.package.json` 与对应 `package-lock.json`，`bootstrap-runtime.sh` 改 `npm ci`；各打包 profile 生成并提交 `package-lock.json`，`prepare-runtime-assets.sh` 改 `npm ci --omit=dev`；pnpm 固定到实际安装版本；Python 用 `pip-compile --generate-hashes` 生成 `scripts/python-requirements.lock`（含 macOS universal 与 Windows 哈希），`pip install --require-hashes`。做不到就报告，不降级为无哈希。
7. **T11（发布校验和）**：`publish` 生成 `SHA256SUMS.txt` 并作为 Release 资产，说明里写 `shasum -a 256` 与 `Get-FileHash` 校验命令。
8. **T12（前端单测基线）**：`package.json` 加 `test` 脚本（Node 原生、沿用 `--experimental-strip-types`），先本机跑通现有 8 个文件；为 `workIntent.ts`、`workIntentDecision.ts`、`sceneThreads.ts` 纯函数补表征测试；先核实 `recordReview.ts`、`displayText.ts` 覆盖情况。不引入新依赖、不修改被测源码。
9. **T13（复盘周期结构化）**：`PERIOD_REVIEW_KEYWORDS` 改为 `{ keyword, period, offset }` 表，去掉所有 `slice(...)` 下标判断；新增「上月复盘」等词条与正确起止/标签（跨年）。表驱动单测覆盖边界与「插入新词后既有用例仍过」。
10. **T14 续做**：余下领域按「任务、工作记录、阅读其余、知识库、写作、设置、引擎」逐个迁移；每步导出命令集与基线逐项比对（277 个，若本轮新增命令先更新基线并说明），每步前后跑 `cargo test --lib -- --test-threads=1`。

**决策点（默认值，请确认或改）：**
- P1 T10 的 Python 锁文件需要联网与 `pip-compile` 工具；默认按任务书执行，工具或平台哈希不可得时停下报告，不降级。
- P2 T2 固定 SHA 需要访问 GitHub 查询各 Action 对应 Release 标签；默认联网查证（不凭记忆填写）。
- P3 T12 的 `npm run test` 路径：默认 `node --experimental-strip-types --test src`（Node 递归发现 `.test.mjs`），先本机验证覆盖现有 8 个文件；若不可用再改显式文件列表。
- P4 T13 可选项「复盘提示词补传今天日期」默认**不做**（涉及 `conversationPrompt.ts` 与 period-review skill 口径），需另行批准。
- P5 T14 基线使用实测 277 个命令（任务书写的 281 是文本出现次数口径，含注释里的字样）。

**不在本节范围：** 代码签名与公证（T11 明确排除，需付费账号）、SettingsPanel.tsx / HomeChat.tsx 拆分、`runtime/` 内 DSH Runtime 修改、`lib.rs` 的 `.lock().unwrap()` 与 CSP `unsafe-inline`。

**验收与记录：** 每项实测命令与输出写 STATUS.md（静态检查 / 脚本测试 / 未验证）；每项完成后在 REVIEW.md 写独立审查结论；无法实测的（Windows 实机、真实发布、golden_tasks 40 个任务）标「未验证」。


### OD-HARDEN-2043：v0.2.43 加固任务书（A/C/B/E/D）（已批准执行；A/C/B/E 已完成，D 仅第 1 步，2026-11-20）

> **触发（用户当轮指示）：** 对 v0.2.43 做抽样代码审计后，用户下发加固任务书，含 A 发布测试门禁、C 外联 URL 校验、B 本地读文件宿主授权、E 小项加固、D 拆分 lib.rs。建议顺序 A → C → B → E → D，D 必须最后做。
> **状态：** 本节只是范围与验收的书面草案。用户批准前不修改代码、不运行改动型命令。批准不扩大本节范围。

**开工基线（2026-11-20 只读核对）**
- 仓库 `/Users/yvan/Code/MyWorkStation`，HEAD `2046d12d33c874131ba20319cf1b8686f7b401ad`（0.2.43 合并提交），`git status --short` 为空。
- 本机 `python3` 为 `/usr/bin/python3`，未安装 pytest；`actionlint`、`xvfb-run` 未安装。
- 源码事实（与任务书逐条核对）：
  - `release.yml` 只有 meta、macos、windows、publish 四个 job，没有测试 job，也没有 pull_request 触发。
  - `validate_feed_url`（rss_store.rs:229）只检查 http/https 前缀；RSS 抓取（rss_store.rs:711）用 reqwest 默认重定向。
  - `read_remote_media`（task_shell.rs:3182）已逐跳处理重定向，但没有地址过滤；`REMOTE_IMAGE_MAX_BYTES` 为 32 MiB，注释写 8 MiB。
  - 前端 `open as openFileDialog / openDirectoryDialog` 共四处：HomeChat.tsx、ReadingPage.tsx、KnowledgeVaultPage.tsx、SettingsPanel.tsx。`dialog:allow-open` 只在 `capabilities/default.json` 与 `tests/verification/desktop/verify_home_polish_078.py:198` 中出现。
  - 任务书所列 B 类 10 个命令均存在于 lib.rs。
  - E-2：`reading_attachment_extract.py` 对 docx 已拒绝 DOCTYPE 与 ENTITY，对 epub 已拒绝 ENTITY 并剥离 DOCTYPE。实体检查已存在，缺的是回归与恶意样本测试。
  - E-3：生产代码（第一个 `#[cfg(test)]` 之前）的实际计数与任务书不一致，见决策 D6。
  - `lib.rs` 共 14987 行，`#[tauri::command]` 共 281 处。

**任务 A：发布流程加测试门禁**
1. `release.yml` 新增 `test` job（ubuntu-latest），触发增加 `pull_request`。meta、macos、windows、publish 加 `if: github.event_name == 'push'`，使 PR 只跑 test。
2. 依赖图：macos、windows 的 `needs` 为 `[meta, test]`；publish 的 `needs` 为 `[meta, test, macos, windows]`。test 失败时 publish 被跳过。
3. test 步骤：安装 Tauri 2 Linux 依赖（libwebkit2gtk-4.1-dev、libgtk-3-dev、libayatana-appindicator3-dev、librsvg2-dev、patchelf）；`cd desktop/src-tauri && cargo test --lib -- --test-threads=1`（CI 去掉 --offline）；`cd desktop && npm ci && npm run build`；Python 验证按 D3、D4 执行。
4. 缓存 cargo 与 npm（方式见 D7）。
5. 本机先依次跑三组命令记录基线。基线失败则列出失败清单并停止报告，不用 continue-on-error，不删测试。

验收：三组命令的实际输出写入 STATUS.md；actionlint 不可用则标"未验证"，改为人工核对 needs 依赖图；apt 依赖与 Linux 运行只能在 CI 中验证，本机是 macOS，标"未验证"。

**任务 C：外联 URL 统一校验（RSS 订阅源与远程媒体）**
1. 新增 `desktop/src-tauri/src/net_guard.rs`，提供 `assert_public_http_url(url) -> Result<Vec<SocketAddr>, String>`。规则：只放行 http/https；拒绝用户名或密码；拒绝空主机；经 `ToSocketAddrs` 解析后，任一结果落入禁止段即拒绝；IPv4 映射的 IPv6 先还原为 IPv4 再判断。
2. 策略分两档。订阅源默认拒绝回环、链路本地、未指定、多播、私网与 CGNAT；私网与 CGNAT 受设置开关"允许内网订阅源"（默认关）控制。正文中的远程图片与媒体一律只允许公网地址，不受该开关影响。策略以参数传入，RSS 与媒体各自指定。
3. RSS 拉取改为 `redirect::Policy::none()` 加手动跳转，最多 3 跳，每跳重新校验，禁止 https 降级为 http。
4. 调用方用 `ClientBuilder::resolve(host, addr)` 钉住已校验的地址，防 DNS 重绑定。
5. 系统代理场景下应用无法钉住解析结果。保留逐跳校验，并在 docs/decisions.md 新增一条记录（状态写"已决定，残余风险待用户确认"），写明代理场景的残余风险。
6. `REMOTE_IMAGE_MAX_BYTES` 与注释统一为 8 MiB，错误文案同步。
7. "允许内网订阅源"开关属于界面改动。动工前用 `screenshots-design-inspiration` MCP 搜 2 至 3 个同类设置页，在 STATUS.md 写 2 句以内的借鉴要点。

测试：
- net_guard 表驱动单测，覆盖 IPv4 各禁止段、::1、fe80::/10、fc00::/7、::ffff:127.0.0.1、::ffff:169.254.169.254、100.64/10，以及公网地址放行。
- 测试放行入口用 `#[cfg(test)]` 实现，生产代码不留开关。现有 `fetches_local_http_feed_and_ingests_entries` 改走该测试入口。
- 新增用例：订阅源 302 跳到 169.254.169.254 被拒；图片地址指向 127.0.0.1 被拒；https 降级为 http 被拒；第 4 跳被拒。
- `cargo test --lib -- --test-threads=1` 全绿。

**任务 B：本地读文件改为宿主授权**
1. 新增宿主侧"已授权路径"注册表：`Mutex<HashMap<规范化路径, 授权时间>>`，过期 30 分钟，数量上限 256 条，超出时淘汰最旧项。
2. 新增命令 `host_pick_files`、`host_pick_directory`：在 Rust 侧通过 `tauri_plugin_dialog` 的 DialogExt 弹出选择框，结果规范化后登记，再返回前端。
3. 改造命令 task_read_attachment、task_read_material、reading_import_book、profile_import、knowledge_vault_set_root、task_set_library_root、task_set_working_root、task_set_artifact_root。规则：规范化后的路径必须在未过期的注册表内，或是成果根、库根的子路径；符号链接按真实落点判断。注册表命中时是否刷新过期时间见 D5。
4. task_open_local_file、task_read_reference 只核对，不改动。
5. 前端四处 `open` 改为调用 `host_pick_*`。全部替换后，从 `capabilities/default.json` 移除 `dialog:allow-open`。保留 `tauri_plugin_dialog::init()` 与 npm、Cargo 依赖，因为 DialogExt 仍需要它们。
6. 同步更新 verify_home_polish_078.py:198 的断言。全仓再搜 `dialog:allow-open` 与 `plugin-dialog`，更新引用它们的测试、脚本与文档。
7. 黑名单追加：/.ssh、/.aws、/.gnupg、/.kube、/.config/gcloud、/.docker、/library/keychains、/appdata/roaming/microsoft/credentials、id_rsa、id_ed25519、.env。

测试：单测覆盖未登记拒绝、登记放行、过期拒绝、指向 ~/.ssh 的符号链接拒绝、`..` 穿越拒绝。重跑 verify_home_polish_078.py。
手工清单（STATUS.md 中标"未验证"，除非实机跑过）：附件选择、阅读导入、知识库根目录设置、成果目录设置。
风险：注册表只在内存中，应用重启后清空。需要逐个核对已保存的根目录在再次读取时能否通过"成果根、库根子路径"规则。

**任务 E：小项加固**
1. writing_parse.rs:296 的 `wait_with_output()` 改为 spawn 后用 `try_wait` 轮询，超过 10 秒则 kill，沿用 task_shell.rs 中已有的模式。
2. reading_attachment_extract.py：核对 docx、epub 两支在解析前都先检查原始字节，含 `<!ENTITY` 即拒绝；不拒绝 DOCTYPE。补测试：合成的实体膨胀恶意 docx 与 epub 样本应被拒绝；合成的正常 epub（XHTML 带 DOCTYPE）回归通过。测试样本全部为合成数据。
3. 非锁类生产 unwrap/expect 逐条处理（计数见 D6）：engine_ipc.rs 2 处 spawn expect 改为返回 Err；rss_store.rs 10 处改为 Result；writing_outline.rs 3 处改为 Result。无法返回 Result 的，写不变量注释。锁类 unwrap 与 lib.rs 中的锁 unwrap 不动。

验收：相关单测与 `cargo test --lib -- --test-threads=1` 全绿；STATUS.md 列出每处 unwrap 的处理结果。

**任务 D：拆分 lib.rs（纯重构，最后做）**
1. 写只读的命令清单导出脚本，导出当前 281 个 `#[tauri::command]` 名称与 `generate_handler!` 清单，作为基线。基线输出存于 `tests/verification/desktop/evidence/od-harden-2043/`。
2. 每次只迁一个领域到 `src/commands/<领域>.rs`，领域顺序为任务、工作记录、阅读、知识库、写作、设置、引擎。每完成一个领域：命令名集合与基线逐项比对，必须仍是 281 个且完全一致；`cargo test --lib -- --test-threads=1` 结果与迁移前一致；前端 invoke 名称零改动。
3. 只做机械移动，不改签名、命令名与行为。提交时机由用户决定，本计划不提交。
4. SettingsPanel.tsx、HomeChat.tsx 的拆分不在本计划内。

**决策点（需用户确认，括号内为默认值）**
- D1 私网订阅源（默认：拒绝私网，设置开关"允许内网订阅源"默认关）。任务书已定。
- D2 代理残余风险（默认：接受并写入 docs/decisions.md）。备选：检测到系统代理时直接拒绝订阅拉取。
- D3 Python 测试运行方式（默认：逐个 `python3` 运行选定脚本，不新增依赖）。备选：CI 安装 pytest 后运行 `python3 -m pytest`，这属于新增依赖。
- D4 Python 子集（默认：只跑与本任务相关的脚本，即 E-2 的 reading_attachment_extract 回归与 verify_home_polish_078 静态断言）。其余约 150 个 desktop 验证脚本多数依赖真实窗口或 Xvfb，CI 中没有 xvfb-run，不进 CI。
- D5 宿主授权过期策略（默认：30 分钟，每次命中刷新）。备选：固定 30 分钟，到期即失效。
- D6 E-3 计数。任务书写 engine_ipc 16、rss_store 10、dsh_settings 4、writing_outline 3。实测生产代码的非锁计数为 engine_ipc 2、rss_store 10、dsh_settings 0、writing_outline 3；engine_ipc 另有 14 处、dsh_settings 另有 4 处为锁类。默认不改锁类，与任务书"非锁类"的字面一致。请确认。
- D7 CI 缓存（默认：用 GitHub 官方 actions/cache 缓存 cargo 与 npm）。备选：Swatinem/rust-cache 是第三方 action，属于新增依赖，需批准后才用。

**不在本计划范围**
- CSP 的 unsafe-inline、lib.rs 中的锁 unwrap、SettingsPanel.tsx 与 HomeChat.tsx 的拆分。
- runtime/ 的任何改动；提交、推送、打标签；派发子代理。

**验收与记录**
- 每项实测命令与输出写入 STATUS.md，分为"静态检查、脚本测试、未验证"。
- 每项完成后在 REVIEW.md 写独立审查结论，未审查不标通过。
- 未运行的项（actionlint、Linux CI、Windows 实机、真实窗口手工清单）标"未验证"。


### OD-KB-202：暂存知识卡进入「知识审核」再入库（已实施并通过复核，2026-11-19）

> **触发（用户当轮反馈）：** 知识库里没看到模型联动过来的待审卡片；期待先显示在「知识审核」、用户确认后才进入知识库。现状是「同步到知识库」直接落盘，没有审核动作。
>
> **现状（源码事实）：**
> 1. `knowledge_sync_to_vault` / `knowledge_vault::sync_from_workspace` 扫描 `<workspace>/staging/kb`，逐张校验后直接写入知识库并重建索引，没有审核记录。
> 2. 「知识审核」页的待审项只来自 `knowledge_review_stage`（整理材料→模型产卡→审核→apply），与 workspace 暂存卡是两条链路；所以暂存卡不会出现在审核列表。
> 3. 「同步到知识库」按钮位于「整理材料」弹窗内，等同于直接入库。
>
> **建议方案（推荐）：把暂存卡接入知识审核页**
> 1. 后端新增 `knowledge_staging_list`（只读列出 workspace/staging/kb 下卡片：相对路径、标题、类型、字数、预览；带路径边界校验）。
> 2. 后端新增 `knowledge_staging_sync(paths)`：只对勾选的卡执行现有同步校验（frontmatter/字数/冲突/索引更新），成功后删除对应暂存文件；未勾选的保持不动。复用 `sync_from_workspace` 内部逻辑，不新造落盘规则。
> 3. 后端新增 `knowledge_staging_discard(paths)`：删除勾选暂存卡（拒绝路径穿越；仅限 staging/kb 内 .md）。
> 4. 「知识审核」页新增「暂存卡待审核」区：多选 + 「入库选中」「丢弃选中」；「全部入库」保留但改为带确认对话框；无卡片时该区不占位。入库/丢弃后刷新审核队列与资产列表。
> 5. 审核通过时向 governance 追加 `approve_card` 记录（来源标 staging），与现有审核决策留痕一致。
>
> **验收：** Rust 单测覆盖列表/选中同步/丢弃/路径穿越拒绝；`npx tsc --noEmit`、`npm run build`；新增 `verify_knowledge_staging_202.py` 端到端（合成卡→列表→选中入库→未选保留→丢弃）。
>
> **明确不做：** 不做自动入库；不改 raw 材料审核链路与审核 JSON schema；不触碰用户真实卡片（验证只用 /tmp 合成根）。
>
> **备选（若用户选择另一条路）：** 去掉审核步骤，任务结束或打开知识库时自动同步暂存卡（仅作为显式选项，需另行批准）。

### OD-READ-004：首页统计下移 + MOBI/PDF 原生阅读导入（已实施并通过复核，2026-11-18）

> **触发（用户当轮指示）：** ①首页阅读统计从问候区下方移到对话框中「项目跟踪」快捷按钮的位置，取消该快捷入口，对话框下移并与统计条留出间距；②阅读导入除 EPUB 外原生支持 MOBI、PDF 等常见电子书格式。
>
> **范围与做法：**
> 1. 首页布局（`HomeCanvas.tsx`）：统计条移出上部问候区，改渲染在 `HomeChat` 下方（`home-reading-stats-slot`），与输入区留 `pt-4` 间距；删除「最近项目/项目跟踪」快捷按钮及其数据加载（`projectList` effect、`ProjectInfo` 状态）；对话容器加 `pt-2 md:pt-3` 下移间距。
> 2. MOBI（`scripts/reading_attachment_extract.py`）：纯标准库实现 Palm Database 解析 + PalmDOC LZ77 解压（压缩 0/1/2），MOBI 头文本编码（65001/1252）、HTML 标签转纯文本、EXTH 封面（201/202 + 首图记录）提取；HUFF/CDIC 压缩与 DRM 加密如实拒绝并说明。Rust `READING_EXTRACT_EXTS` 增加 `mobi`，界面文件过滤器与错误提示同步。
> 3. PDF：随包 `runtime/python-vendor` 增加 `pypdf`（写入 `scripts/python-requirements.txt`），保证发布包无需用户自行 pip 安装即可逐页提取文字；扫描件仍如实提示无文本。
>
> **验收：** 新增 `tests/verification/desktop/verify_reading_004_formats.py`（19 项：布局/白名单/依赖/MOBI 合成样例三态/PDF 文字提取）；`cargo test --offline --lib`、`npx tsc --noEmit`、`npm run build` 全过。
>
> **明确不做：** MOBI 的 HUFF/CDIC 解压、DRM 去保护、PDF 扫描件 OCR、AZW3/KF8 专门解析、PDF 页面渲图。

### OD-READ-003：健脑室——首页统计 + 左栏分页 + 富媒体阅读 + 选区浮条修复（已实施并通过复核，2026-11-18）

> **触发（用户当轮指示）：** EPUB 已可导入，继续完善四件事：①首页展示阅读统计；②左栏分「本地书籍 / 订阅文章」两页，书籍展示封面；③阅读区支持封面/图片/音频/视频与 Markdown 排版；④修复阅读界面划线标记与笔记浮动菜单条不显示。
>
> **开工基线：** main @ a93212260a；工作区有两处既有未提交改动（export_engine.rs / integrity.rs），本任务不触碰、不还原。
>
> **根因与现状（源码事实）：**
> 1. **选区浮条不显示是确定性 bug**：`ReadingPage.tsx` `readSelection()` 用 `closest("[data-paragraph]")` 定位段落，但正文段落元素只有 `data-chapter-index`/`data-paragraph-index`，没有 `data-paragraph` → 选区定位必然失败，浮条永远不出现。
> 2. **统计无聚合**：`ReadingBookMeta` 已有 char_count/annotation_count/note_count/progress_percent，`rss_store` 有文章元数据，缺一个聚合命令与首页展示。「闪记」全仓库无对应数据结构（GOALS/STATUS/代码均无），统计不含闪记、不做假数字。
> 3. **富媒体被丢弃**：`scripts/reading_attachment_extract.py` `_html_blocks` 只收文字，`<img>/<audio>/<video>` 全丢；`rss_store.rs` `sanitize_html` 把 img 压成 "[图片：alt]" 占位文字。书架无封面概念。
> 4. **左栏单列表**：书籍与订阅文章同列混排，无分页、无封面、无按来源分组折叠。

**本切片范围：**
> 1. **统计聚合与首页展示**：
>    - `reading_store.rs` 新增 `reading_stats_at(root)`：书籍本数（source_format != rss）、已读完本数（progress_percent ≥ 0.95，口径写进代码注释）、阅读字数（Σchar_count）、批注条数（Σannotation_count，含划线/标记/想法）、笔记篇数（Σnote_count）；`rss_store.rs` 提供文章篇数；Tauri 命令 `reading_stats` 合并返回。
>    - 首页 `HomeCanvas` 问候区下加一行统计条（图标 + 数字 + 标签，暖白令牌新拟物卡片，一行紧凑排布；OD-ARCH-035 原则：不逐项加说明文字）；点击跳转健脑室。数据读取失败如实显示错误，不编造 0。
> 2. **左栏两分页**：
>    - 左栏顶部「本地书籍 / 订阅文章」两页签；书籍页：封面卡（无封面用暖底 + 书名首字占位）+ 书名 + 进度条，顶部「导入」；订阅页：按订阅来源分组折叠（组头=来源名+篇数，展开为文章列表：标题 + 未读点），顶部「订阅管理」「拉取全部」。折叠状态存组件 state 不落盘。
> 3. **富媒体与排版**：
>    - 数据链路：段落仍是纯文本（保住划线锚定），媒体以行内标记嵌在段落文本中：提取器输出 `![img](odmedia:<zip内路径>)`；RSS `sanitize_html` 输出 `![img](<原src>)`（data:/http(s): 保留，其余丢弃），`<audio>/<video>` 输出对应占位标记。
>    - Rust 新增 `reading_media_extract(book_id, media_ref)`：仅限本书源 EPUB 内按 zip 路径读字节（单文件 ≤ 10MB、MIME 白名单 image/audio/video），返回 base64+mime；封面在导入时提取（OPF cover-image，原图字节存 `meta/<book_id>.cover.*`，≤ 2MB），书架与阅读头部显示。
>    - 前端：媒体段落渲染为 `<figure>`（图片圆角、居中、点击放大 lightbox；音频/视频原生控件 + 新拟物容器）；`odmedia:` 经命令取 base64 渲染（组件内缓存）；RSS 的 http(s) 图直接由 WebView 加载（不代理不落盘）。段落内行内 Markdown（粗体/链接/行内代码）用已有 react-markdown 管线渲染；`reading.css` 增加 .reading-media-* 样式（圆角 14px、描边、柔和阴影，与全局新拟物一致）。
>    - 参考（OD-ARCH-035，动工前用 screenshots-design-inspiration 检索并把 2 句借鉴要点写入任务记录）：qiaomu-rss-dsh 只读借鉴其「正文白名单渲染 + 选中即划线」思路，不搬代码。
> 4. **选区浮条修复**：
>    - 修复1：段落元素补 `data-paragraph` 属性（定位选择器的真正锚点）。
>    - 修复2：document mouseup 后仅 setTimeout(0) 读选区在部分拖选场景早于选区落定——增加 selectionchange（去抖 ~150ms）作为第二通道，两条路都收敛到同一 `readSelectionRef`；失败静默清空浮条状态。
>    - 保留：键盘选区 onKeyUp、工具栏 mousedown preventDefault、跨段拒绝提示。

**验收：**
> 1. Rust：`reading_stats`/`reading_media_extract`/封面提取单测；`cargo test --offline --lib -- --test-threads=1` 全过。
> 2. 前端：`npx tsc --noEmit`、`npm run build` 通过；新增 `tests/verification/desktop/verify_reading_003_selection.py`（断言 data-paragraph 锚点、selectionchange 通道、浮条渲染条件、首页统计条与左栏分页存在）。
> 3. 合成含图 EPUB：导入后封面出现在书架、图片在正文显示、划线浮条可出现（脚本级验证 + 真实窗口留给用户）。
> 4. 首页统计条为真实聚合数字；左栏两分页可切换、订阅分组可折叠。

**明确不做：** 不做阅读主题切换；不做 PDF 图片化；不代理/转存 RSS 音视频；不动 runtime/ 与模型路由。（「闪记」已按用户当轮语义以 thought 批注计数实现。）


### OD-ARCH-183：Windows 引擎进程树回收（已实施，2026-11-18；Windows 实机待复验）

> **触发（Windows 实测）：** 关闭应用后 `node.exe` 残留，开关 2 次累计 7 个。实测命令行：`"node" D:\OmniDesk\runtime\dsh-runtime\node_modules\@deepseek-ai\dsh\lib\bin.js --profile discipline --patch C:\Users\Yvan\OmniDesk\app\config\omnidesk-model.yml --port 0 --no-open`，父进程为已退出的 python 引擎。残留是 DSH 主进程本身（每个引擎实例一个），不是 MCP server——工具面未声明 mcpServers，free-search/cost-meter 插件也无 mcp 声明。
>
> **根因（源码事实）：**
> 1. `engine/omnidesk_engine/process_owner.py` 是 POSIX-only（读 `/proc`）：Windows 上 `_proc_stat` 返回 None，`_kill_child_group`/`terminate_group` 永远空转，优雅关停从不回收 node；`record_child_group` 也不登记 node pid。
> 2. `desktop/src-tauri/src/platform.rs`：Windows 上 `boot_id()` 返回 None、`process_stat()` 返回 None、`process_cmd()` 返回空 → `process_owner::reclaim_before_start` 每次启动都判 `stale_boot`，直接删登记、不回收任何遗留进程；`cleanup_engine_group` 见 pgid=0 直接跳过。
> 3. Job Object（0.2.15/0.2.16）理论兜底 kill-on-close，但实测未回收；不再把它当唯一保证。
> 4. 进程别名未生效：残留镜像名是 `node` 而非 `omnidesk-node`（`OMNIDESK_NODE` 被环境变量覆盖或副本回退），按名清理不可靠。

**修复方案（双保险，名字只作识别辅助）：**
> **阶段 1 引擎侧（engine/omnidesk_engine）：**
> 1. `process_owner.py` 增加 Windows 分支：用 `taskkill /PID <pid> /T /F` 树杀替代 `killpg`；`record_child_group` 在无 `/proc` 时直接记录 `child_pid=proc.pid`、`child_pgid=0`。
> 2. `web_api_adapter.py` 的 `shutdown()` 在 finally 中执行 Windows 树杀（RPC 超时/异常也必须执行）；node 启动成功后把 pid 写入登记（`owner.json` 的 `child_pid`）。

> **阶段 2 宿主侧（desktop/src-tauri）：**
> 1. `platform.rs`：Windows 实现进程存活判断（`tasklist /FI "PID eq N"` 或 Toolhelp FFI）与命令行读取（仅启动/收尾时用，不在热路径）。
> 2. `process_owner.rs`：Windows 不依赖 `boot_id` 判 stale；按 `host_pid` 判活（另一实例活跃则拒绝）；遗留引擎与 `child_pid` 经「镜像名 node/omnidesk-node + 命令行含 `dsh-runtime` 与 `bin.js`」核验后 `taskkill /T /F`；Job 保留为最后保险。
> 3. 退出路径 `finish_stop`：`terminate` 后仍按登记 `child_pid` 做一次树杀兜底。

> **阶段 3（可选，识别辅助）：** 查明 `OMNIDESK_NODE` 别名为何未生效（环境变量覆盖 / 副本失败 / 用户全局设置），使残留进程名归一到 `omnidesk-node`；不作为杀进程判据。

**验证：** `cargo test --offline --lib` 全过（新增 Windows 判定纯逻辑测试可跨平台跑）；引擎 Python 单测（合成 pid/命令行，不真杀）；Windows 实测：正常使用后关 app，`Get-CimInstance Win32_Process` 过滤 `dsh-runtime` 残留为 0；连续开关 2 次无累积；正常任务与模型调用不受影响。

**明确不做：** 不改 `runtime/` 内 DSH；不按裸 `node.exe` 全名杀（防误伤用户其它 node 程序）；不动模型路由与凭证。

**执行结果（2026-11-18，ARCHITECT 亲自实施）：**
> 1. 引擎侧：`process_owner.py` 增加 Windows 分支——`tasklist` 判定存活、`Get-CimInstance` 取命令行、`taskkill /PID <pid> /T /F` 树杀；`_inspect_owner_windows` 用镜像名核验（python/node），不依赖 `/proc`；`record_child_group` 在无 `/proc` 时靠 `_proc_stat` 的 Windows 实现记录 `child_pid`（pgid=0）；`claim_ownership`/`reclaim_owner` 对 pgid=0 改走 child_pid 树杀。`web_api_adapter.shutdown()` 在 Windows 上先树杀再 terminate（graceful 与 force 两条路径都覆盖）。`serve._finalize()` 关停失败时保留 owner.json，供宿主下次回收。
> 2. 宿主侧：`platform.rs` 实现 Windows `process_stat`/`process_cmd`/`process_table`/`process_image`/`kill_process_tree`/`sweep_orphan_dsh_processes`；`process_owner.rs` 增加 Windows 启动回收（镜像名核验 + child_pid 树杀 + 死父进程 CLI 指纹清扫）、收尾兜底 `cleanup_engine_group` Windows 分支、退出兜底 `cleanup_child_on_exit`；`lib.rs` 在 `RunEvent::Exit` 调用退出兜底。
> 3. 测试：`cargo test --offline --lib -- --test-threads=1` → 1232 passed / 0 failed / 1 ignored；引擎适配层 249 个单测全过（含新增 5 个 Windows 判定单测）；`verify_process_platform_133` 28 项全过（断言已更新）；`verify_sys_process_701` 40 项全过；`verify_proc_alias_149` 34/34。
> 4. 偏差与未验证：`verify_process_lifecycle_074` 有 4 项（N/O/P/R）既有失败，已在 HEAD 纯净副本上复现同样失败，判定为环境性问题、与本改动无关。Windows 专属代码无法在本机编译验证（`cargo check --target x86_64-pc-windows-msvc` 因缺 `llvm-rc` 无法构建），需在 Windows 实机复验：开关两次 app 后 `Get-CimInstance` 过滤 `dsh-runtime` 残留为 0。


### 附：知识卡 staging 路径歧义小修（2026-11-18，已实施，ARCHITECT 亲自执行）

> **触发：** Windows 实测建卡失败。定性（源码 + 实测证据）：不是宿主 ACL，也不是 skill 限制工具——`knowledge-search` SKILL 第四节「workspace 暂存区」的 `workspace` 一词被模型按字面映射到 `…\OmniDesk\workspace\`，而引擎沙箱根是其下的 `artifacts`（`prepare_workspace_dir` 返回值），越根被 DSH 拒（`Access to the path 'staging' is denied`，enforcement: partial 不弹窗）。模型已在 `workspace\artifacts\staging\kb\知识库` 成功建目录——那正是宿主同步入口扫描的位置。
>
> **修改（2 处）：** ①`skills/knowledge-search/SKILL.md` 第四节第 3 条改为「以引擎当前工作目录（cwd，默认 `~/OmniDesk/workspace/artifacts`）为基准写相对路径 `staging/kb/…`，不要走进名字叫 workspace 的目录，不确定先 pwd」；②`tests/verification/desktop/verify_knowledge_sync_152.py` 断言同步更新（断言 `staging/kb/知识库/概念/` + 「当前工作目录」表述，并断言不再出现反引号包裹的 workspace/staging 前缀写法）。
>
> **验证：** `python3 tests/verification/desktop/verify_knowledge_sync_152.py` → 48 项全过（exit 0）。宿主同步入口（`knowledge_sync_to_vault` 扫 `<数据根>/staging/kb`）与引擎 cwd 同源，无需改代码。


### OD-READ-002：健脑室——读书页融合 RSS 新闻聚合（待批准，2026-11-18）

> **触发（用户当轮指示）：** 把新闻聚合 RSS 融合进读书界面；读书入口更名为「健脑室」；保留既有划线工具栏能力。参考 qiaomu-rss-dsh 插件源码（GPL-3.0，只读借鉴思路，不搬代码）与其信息源。用户已指定派工模型与档位；ARCHITECT 只做计划与复核。
>
> **参考项目只读勘察结论（2026-11-18，ARCHITECT 完成）：**
> 1. **乔木 RSS（qiaomu-rss-dsh）**：feed 解析走 RSS 2.x / RDF / Atom 统一归一化（entry: id/guid/link/title/published/content/author/summary）；正文上限 100k 字符、每 feed 最多 50 条、总量 1MB；订阅与文章 JSON 落本地 storages，元数据与正文分文件；HTML 白名单净化后渲染，选段伴读只带选中引文（text quote anchor + prefix/suffix）。划线/笔记明确「插件不提供」（README 原文），划线能力复用 OmniDesk 读书页既有实现。
> 2. **信息源**：精选作者 9 条 RSS（潮流周刊/阮一峰/云风/和菜头/张鑫旭/小众软件/月光博客/Reorx/pseudoyu）；独立博客目录来自 timqian/chinese-independent-blogs（MIT），共 1342 条，可按 tag 筛；播客目录另列。本切片只内嵌精选作者 + 精选独立博客（不超过 40 条，总大小控制在 20KB 内，保留 MIT 归属注释），不做公众号源（需第三方服务）、不做播客。
> 3. **设计参考（screenshots-design-inspiration MCP 已检索）**：My Reader AI（左栏文章列表 + 右侧正文单列）、Perplexity（信息密度留白）、Apple Books（书架卡片 + 批注颜色系统）。借鉴要点：左栏双分组（书籍在上、订阅文章在下）共用一条列表样式，用来源徽标区分；阅读区保持单列排版，划线工具栏沿用既有 4 按钮浮条；书架顶部加「订阅」入口，不新增 Dock 项。

**本切片范围（平台内置，不动 Profile 声明）：**
> 1. **入口更名**：Dock 读书入口 label 改「健脑室」，页面内标题同步；ReadingPage 模块路径不改（保持既有 import 稳定），改显示文案与注释。
> 2. **RSS 源管理（阶段 1 最小版）**：阅读目录 rss/subscriptions.json 存订阅（schema omnidesk.rss.v1）；预置精选作者 + 精选独立博客静态清单（Rust 常量数组，含 MIT 归属注释）；用户可在界面添加任意 RSS/Atom URL、删除订阅；不做 OPML 导入导出（留后续）。
> 3. **RSS 拉取与解析**：Rust 侧 HTTP GET（单请求 20s 超时、响应体上限 2MB、最多 200 条 entry、每 feed 保留最新 50 条）；RSS 2.x/RDF/Atom 三格式归一化；XML 解析手写小型解析器（不引新 crate，参考乔木插件 xml.js 的思路）；失败时返回错误信息不静默。网络客户端必须新建（local_http 只允许 127.0.0.1，不可复用）；允许引入 reqwest（已在 Cargo.lock 作传递依赖）或 ureq，选型由 BUILDER 决定并说明理由。
> 4. **文章入库与书架融合**：每篇文章在书架是一条条目（source_format=rss，book_id=rss-<sha8>-<articleKey>）；标题=文章标题，author=作者，source_name=订阅名；正文转 Markdown 后作为单一章节导入（复用 ReadingBook 结构，paragraphs=正文分段）；同一文章（feed url+entry guid/link 哈希相同）不重复入库、不覆盖已有批注。阅读目录 rss/articles/ 只存元数据 JSON（含已拉取的正文），落仓库外。
> 5. **UI 融合**：书架列表分两组：「书籍」「订阅文章」，同一列表样式 + 来源徽标；订阅文章条目附「未读/已读」小圆点（元数据字段 read）；RSS 文章用与书籍一致的阅读器渲染（划线/想法/标段/问 AI 四按钮浮条不变）；书籍条目不加未读点。
> 6. **AI 伴读沿用既有链路**：选中文字后问 AI 走 reading_concept；RSS 文章整理走 reading_organize（把文章批注传入）。不加新 intent。
> 7. **隐私边界**：RSS 拉取只发 HTTP GET 到用户添加/预置清单里的订阅地址；不发文章正文给任何外部服务；AI 只发选中文本或批注（走既有 submitTask 红线扫描链路）；日志不记正文。

**验收（阶段 1）：**
> 1. Rust：rss_store 新模块单测覆盖 RSS2.x/Atom 解析（含中文/CDATA/HTML 实体）、去重、条目上限、正文截断；cargo test --offline --lib -- --test-threads=1 全过。
> 2. 前端：npx tsc --noEmit、npm run build 通过；阅读页能添加/删除订阅、拉取文章、打开 RSS 条目、划线、问 AI（真实窗口验收留用户）。
> 3. 数据：订阅与文章只落 ~/OmniDesk/reading/rss/；仓库内不出现文章正文；重复导入不覆盖；网络失败报错可读。
> 4. 预置清单含 MIT 归属注释；不含公众号源与播客；总大小 <20KB。

**明确不做（阶段 1）：** 不做播客/公众号源、不做 OPML 导入导出、不做阅读版本切换/翻译/改写、不做搜索/收藏/未读过滤（书架只有分组展示）、不做乔木服务对接、不改 runtime/、不引入数据库、不做全文搜索索引。

**风险与技术决策：**
> 1. HTTP 客户端选型：reqwest 已在传递依赖树，加为直接依赖不改变锁文件；ureq 更轻但需加新 crate。默认建议 reqwest（blocking 特性即可），BUILDER 可按实际依赖树调整，理由写入 STATUS.md。
> 2. XML 解析：手写小型解析器（约 200 行）足够 RSS/Atom；避免引入 quick-xml 等新依赖。
> 3. HTML 正文转 Markdown：手写白名单 + 段落提取（复用 reading_store 已有的正文分段逻辑），不追求完美转换，代码块/图片/列表保留基本结构。
> 4. 阶段 2 候选（另行批准）：OPML 导入导出、未读/收藏过滤、搜索、文章列表分页加载、播客转写。

**派工（用户当轮已指定）：**
> 1. 单个 BUILDER 子代理执行阶段 1；模型 OpenDesign/deepseek-v4.1-flash-eco、思考等级 high。若 eco 档不可用，按 OD-READ-001 先例：先试 OpenDesign/deepseek-v4.1-flash + high，再不行用 OpenDesign/glm-5.3-flash + high，并记录实际原因。
> 2. 工作树：/Users/yvan/.codex/worktrees/2221/MyWorkStation（分支 od-read-002-brain-room，基于 main HEAD cb0a0b31f0，已创建并干净）。
> 3. 任务书落盘 docs/od-read-002-stage1-dispatch.md（worktree 内）；ARCHITECT 不亲自实现。
> 4. ARCHITECT 复核 diff 与测试输出，写 REVIEW.md；通过后交由用户决定是否提交推送（本轮不自动推送）。

**批准状态：2026-11-18 用户当轮原话已授权按指定模型派工**（「怎么融合以及原有的读书界面怎么设计，你参考原设计和 /opendesign 的案例以及omnidesk的设计规范来确定plan，然后指派子agent执行，记得用deepseek-v4.1-flash-eco模型high思考等级」）。如该模型实际不可用按上述顺序回退并记录。

**执行与复核结果（2026-11-18）：** BUILDER 子代理 Godel（deepseek-v4.1-flash-eco / high，派工通道实际接受 eco 档）完成阶段 1：rss_store.rs（订阅存储 + 49 条预置 + 手写 RSS/Atom 解析 + 本地回环 HTTP 回放测试）、lib.rs 6 命令、ReadingPage 健脑室改名与书架双分组/订阅模态/未读圆点、reqwest(blocking+native-tls) 直接依赖。ARCHITECT 实跑复核：cargo 1243/0/1（OMNIDESK_REPO_ROOT 绕过 worktree 路径误伤）、tsc 0、build 0、verify 42 项全过、git diff --check 0；REVIEW.md 已记，**通过**（真实窗口交互与真实模型输出未验收）。4 个 runtime/ 符号链接为构建工件，提交时不得加入。提交推送待用户裁决。

**阶段 1 实施偏差（2026-11-18，BUILDER）：**
> 1. 为支撑书架「未读」圆点与打开文章后回写已读状态，任务书的三条订阅命令之外新增 `rss_articles_list()` 与 `rss_article_mark_read(book_id)`；`rss_fetch` 仍按任务书的 `subscription_id?` 参数工作。
> 2. 正文将 HTML 白名单净化后转成 `ReadingChapter.paragraphs` 的纯文本分段，不保留 Markdown 标记：现有阅读器按段落渲染，保留 `#`/`**` 会把标记当正文显示。
> 3. 开工后 `desktop/src-tauri/src/lib.rs` 出现一套未登记的 `rss_library/rss_catalog/rss_refresh/...` 命令桩（与任务书命令名不一致）；已统一为任务书命令集，避免两套 RSS 契约并存。
> 4. 网络客户端选 `reqwest 0.13.5` 的 `blocking + native-tls + system-proxy`；macOS 走系统 Security Framework，离线构建可用。启用这些特性会新增 `hyper-tls/native-tls/security-framework` 等传递依赖，`Cargo.lock` 因此增加约 215 行，原“锁文件不变”假设不成立。
### OD-READ-001：读书阅读笔记分页（初步实施，2026-11-18）

> **触发（用户当轮指示）：** 为 OmniDesk 增加读书阅读笔记分页：读书时能划线、标记、添加思考碎片，前端记录；AI 能整理笔记，也能随时查询相关概念。先初步实现这些功能。用户给出三个参考项目：`lysforge/dsh-lamplit`、`zhaohongxuan/dsh-weread`、`PensiveFei/deep-read-summarize`。

**参考项目核实（只读上游源码，不引入其插件依赖）：**
> 1. **lamplit**：统一书架 → 沉浸阅读 → 划词 AI → 按书笔记，笔记字段为「书 / 章节 / 引文 / 想法 / 来源模式」，导出 Markdown；本地优先。
> 2. **dsh-weread**：把「个人划线」与「想法」分成两个类型，按章节/日期回顾，能跳回原文；提示词只带当前摘录，不把整本书无差别灌进模型。
> 3. **deep-read-summarize**：长文先分块再整理成结构化 Markdown，强调原文核验、来源状态与质量未通过时不得冒充完成。

**本切片范围（平台内置，不依赖 Profile 声明）：**
> 1. Dock 新增「读书」入口；页面为「书架 → 阅读器 + 笔记侧栏」。书架列出书与阅读进度；阅读器按章节/段落呈现正文。
> 2. 导入用户通过系统文件框**显式选择**的 `.md/.txt/.pdf/.docx/.epub`；抽取正文与章节，书与批注落仓库外 `~/OmniDesk/reading/`（测试用一次性根）。重复导入同一内容按内容哈希返回已有书，不覆盖。
> 3. 划线与思考碎片按「章节 + 段落 + 字符偏移 + 引文」锚定；标记为段落级书签。侧栏可回到原文、编辑或删除；进度按滚动比例保存。
> 4. AI：选中文字后可「解释 / 查概念」，也可把本书批注整理成 Markdown 草稿。模型结果只显示为草稿，只有用户点击保存才写入阅读目录；不自动写知识库、不自动确认。
> 5. 模型调用继续走 `host.submitTask` 的既有准入、红线扫描与任务状态链路；只发送用户当前选中的引文、问题和批注，不在后台自动上传整本书。

**验收：**
> 1. Rust：`reading_store` 单测覆盖导入/章节切分/重复导入/字符偏移（含中文与 emoji）/批注增删改/进度/导出；`cargo test --lib` 通过。
> 2. 前端：`npx tsc --noEmit`、`npm run build` 通过；界面实际渲染与选择/批注/侧栏回跳在真实窗口验收（若本轮窗口不可用则如实记未验证）。
> 3. 数据：仓库内不出现书正文、批注或测试夹具；失败不产生半成品书；重复导入不覆盖。

**明确不做（本切片）：** 微信读书扫码/同步、GitHub 书库、云端阅读进度、扫描版 PDF OCR、PDF 原版分页渲染、多端同步、自动把 AI 稿写入知识库；不改 `runtime/`，不改 DSH 私有数据格式，不新增数据库。

**风险与后续：** EPUB/PDF 抽取依赖本机 Python 与 `pypdf`；抽取失败时只报告原因，不伪造正文。跨段选择本切片先限制在同一段，跨段划线与重叠批注的合并展示留到后续验收。

**执行结果（2026-11-18，ARCHITECT 亲自执行，待独立复核）：**
> 已落地：Dock 固定「读书」入口与懒加载页面；书架/阅读器/笔记与 AI 侧栏；`.md/.txt/.pdf/.docx/.epub` 导入（UTF-8 直读、非 UTF-8/PDF/DOCX/EPUB 走既有 Python helper，EPUB 新增 stdlib 章节抽取）；批注按章节+段落+code point 偏移+引文锚定，支持划线、段落标记、思考碎片、编辑/删除/回跳；进度单独落 `progress/`，不随滚动重写整本书；AI 解释/查概念与批注整理只产草稿，用户点保存才写 `notes/`，确定性导出写 `exports/`。
> 实跑：`cargo test --offline --lib -- --test-threads=1` 1225 passed / 0 failed / 1 ignored；`npx tsc --noEmit`、`npm run build` 通过；`node --experimental-strip-types --test src/readingAi.test.mjs` 4/4；EPUB/GB18030 helper 手工样例通过。
> 未验证：真实桌面窗口的选区浮条、回跳定位与视觉排版；真实模型的概念解释与整理稿质量；32 MiB 级 PDF 抽取耗时；跨段选择（本切片明确拒绝并提示）。

**用户裁决（2026-11-18 深夜，待批准派工）：** 用户复核后裁决：①用户睡觉后由 ARCHITECT 复核推进；②再次学习三个参考项目后，对现行方案（含已实现部分）进行复核；③认为可行或需要完善的，直接修改本计划，不必再等批准；④授权安排 BUILDER 子代理执行（模型 `OpenDesign/deepseek-v4.1-flash-eco`，思考等级 high；如该模型/档位不可用，记录实际原因后选用可用档位或改派 glm-5.3-flash，并在 STATUS.md 说明）；⑤ARCHITECT 只负责复核，不亲自实现；⑥复核通过后，由 BUILDER 子代理把改动提交至 GitHub 并推送，形成最新版本。

**ARCHITECT 复核结论（2026-11-18，依据三个参考项目的再次只读学习）：**
> 1. **方案总体可行。** 已实现部分与三个参考项目的共同骨架一致（本地优先、划线/想法分型、按书整理、原文回查、草稿确认），且遵守 OmniDesk 的仓库外数据边界与「AI 只产草稿」契约；不需要推翻重做。
> 2. **借鉴 deep-read-summarize 的「原文核验」补确定性引文核验（新增阶段 4）。** 现行 `reading_organize` 只有提示词约束「不得补造」，没有程序级核验。补一条宿主纯函数：解析整理稿里的 `>` 引用行，逐条与本书批注的 quote/paragraph 做规范化比对（去空白差异）；任何一条找不到来源，就在整理稿顶部插入「⚠ 本稿存在 N 条未核验引文」提示块，拒绝静默通过。这是宿主确定性检查，不调模型、不新增依赖。
> 3. **借鉴 lamplit 的「整理前后文」补边界改进（并入阶段 4）。** AI 整理稿保存时，若正文超过 200 字符，在保存的 Markdown 顶部补 frontmatter（title/created/book/source/annotation_count），方便外部工具（Obsidian）识别；不做多文件拆分。整理稿的模型输出含围栏包裹时先剥一层围栏（参考其 extractNote 的做法，保持简单实现）。
> 4. **不做的事维持不变：** 不接微信读书/GitHub 书架、不做扫描件 OCR、不改 runtime/、不引入数据库；跨段划线、重叠批注重建、AI 稿直接写知识库仍列为后续复审触发，不在本轮。

**阶段 4（确定性引文核验 + 整理稿边界，BUILDER 执行）**
> 1. `desktop/src-tauri/src/reading_store.rs`：新增 `verify_organized_markdown(book, markdown) -> Vec<String>` 纯函数：提取整理稿中的 Markdown 引用行（`>` 开头），去首尾空白与多余空格后，与本书批注 quote 及批注所在段落原文比对；返回未核验引文列表（最多列 20 条）。`save_note_at` 调用它，未核验数 > 0 时在保存稿顶部插入提示块（不阻断保存，但不得静默）。
> 2. `save_note_at` 保存的 Markdown 无 frontmatter 时补 5 字段 frontmatter（title/created/book/source/annotation_count）；已有 frontmatter 不改写。
> 3. `desktop/src/shell/ReadingPage.tsx`：AI 整理结果进入编辑区前，剥一层 Markdown 围栏（仅当整段被单个 ```` ``` ````/`~~~` 围栏包裹时）；整理稿保存回执在界面显示「已核验引文 x 条 / 未核验 y 条」。
> 4. Rust 单测：核验函数（全部命中 / 部分 / 全部未命中 / 中文引文空白差异）；frontmatter 补写与不改写；围栏剥离纯函数。现有测试不回退。
> 5. 验证：`cargo test --lib -- --test-threads=1` 全过；`npx tsc --noEmit`、`npm run build`、`node --experimental-strip-types --test src/readingAi.test.mjs` 通过。

**派工方式（本轮用户已授权）：**
> 1. 阶段 4 派给一个 BUILDER 子代理（模型 `OpenDesign/deepseek-v4.1-flash-eco`；思考等级 high，如该档位被拒绝则用该模型可用档位或 `OpenDesign/glm-5.3-flash` high，按模板 `docs/BUILDER-Task-Instruction-Template.md` 派工，任务书原文写入 `docs/od-read-001-stage4-dispatch.md`）。
> 2. ARCHITECT 不亲自实现阶段 4；完成后独立复核 diff 与测试输出，写 REVIEW.md。
> 3. 复核通过后，由**另一个** BUILDER 子代理执行提交与推送（提交信息遵循仓库惯例，注明 OD-READ-001；不做 force push、不打标签、不改版本号；推送前跑一次 `git diff --check` 与 `cargo test --lib reading_store -- --test-threads=1`）。推送回执写入 STATUS.md。
> 4. 版本号与 CHANGELOG 不在本轮自动改动；发版留给用户醒着时决定。

**阶段 4 执行与复核结果（2026-11-18）：** BUILDER（deepseek-v4.1-flash-eco / high，昵称 Boyle）完成五条实现；ARCHITECT 实跑复核：cargo 1232/0/1、tsc 0、build 0、readingAi 5/5、git diff --check 0。任务书允许清单漏列 `desktop/src/readingAi.ts`，BUILDER 按测试可导入性补在该文件，复核认可（REVIEW.md 已记）。阶段 4 通过，进入提交推送子任务。

### OD-ARCH-176：工作记录场景自然语言意图与落盘决策重构（待批准，2026-11-17）

> **触发（用户裁决）：** 诊断确认周期复盘在工作记录场景失败（REVIEW.md 2026-11-17 诊断）。用户明确产品方向：①对话分类只有未分类/工作记录/文案写作/项目跟踪，没有复盘专门场景，复盘必须能在工作记录场景直接完成；②意图由模型理解自然语言决定，不能机械靠关键词表触发；③对助手的指令（如「你自己去搜索」）绝不落盘为工作记录。
>
> **根因：** `workIntent.ts` `isPlainStatementNote` 关键词黑名单把祈使形式的指令误判为陈述句先落盘（OD-WR-105）；`HomeChat.tsx:2724` 把 intent 分流门控在周报场景，工作记录场景的复盘请求既不汇集材料、模型又受 period-review skill「不自取」契约约束，形成死等。

**设计原则：** 工作记录场景每轮由模型先理解再路由；关键词表只保留在周报场景（用户在该场景意图明确）作快速通道；拿不准的轮次问用户，不猜。

**阶段 1（意图分类前移，核心）**
> 1. 工作记录场景废除「陈述句不调模型先落盘」：每轮先做一次轻量分类调用（复用现有分析链路），输出统一契约：`{ intent: note / instruction / chat / period-review / report / daily-summary, record?, review?, reply_hint? }`。
> 2. `intent=note`：落盘 + 确认展示 + 质量分析（同一调用带回 review，不二次调用）。
> 3. `intent=instruction/chat`：**不落盘**，作为普通对话轮回复；注入本周记录索引（只读）作上下文，模型可直接回答「查一下记录」类请求。
> 4. `intent=period-review`：工作记录场景内联触发 `worklogPeriodReviewCollect` 材料汇集 + period-review skill 指针出稿（OD-WR-505 复用）；`report/daily-summary` 复用周报场景既有路径。
> 5. 分类不确定（note 与 instruction 边界模糊）→ 用现有 question 卡片问「这条要记吗」，宁可多问不误记。
> 6. **思考档位策略（2026-11-17 用户裁决并入）：** 场景内分类/分析/指令回复类调用默认 `reasoningEffort: "off"`（模型直出，走 `web_api_adapter.py:1273` 既有按次请求覆盖通道，不改覆盖文件、不重启引擎）；用户在模型选择器手动选了档位则按所选档位传。复盘/周报起草轮默认跟随模型默认（不强制 off），用户选档位即按档位——保证重交付物质量；用户想直出可自行选 off。未声明档位模型显式 off 必被接受（OD-ARCH-175 阶段 1 结论），该策略跨渠道安全。

**阶段 2（契约与技能微调）**
> 1. period-review 技能/场景契约补充：宿主汇集材料随请求送达时直接起草，不再输出「等材料」话术（工作记录场景路径）。
> 2. 随手记分析契约更新：明确区分指令/请求与事实陈述，指令不得作为记录或分析对象。
> 3. 周报场景（`preset.key === "weekly"`）行为不变。

**阶段 3（验证）**
> 1. verify 脚本：各意图分流断言、指令不落盘、复盘内联汇集、不确定时走提问。
> 2. `npx tsc --noEmit`、`npm run build`、既有 verify 回归。
> 3. 真实模型验证（用户执行）：工作记录场景分别输入工作事实、复盘请求、指令三类消息，核对落盘/出稿/对话行为。

**明确不做：** 不改周报场景关键词快速通道；不动 OD-ARCH-175 模型路由改造；不加新对话分类；不改 runtime/。

**风险：** 每轮多一次模型调用（原陈述句即时落盘变为一两秒延迟，用户已明确理解优先，可接受）；模型把事实误判为指令 → 阶段 1 的「拿不准就问」兜底；契约改动外溢其他场景 → 限定工作记录场景分支。

> **派工（2026-11-17 用户批准全部执行）：** 子代理执行（BUILDER 模板），模型 deepseek-v4.1-flash-eco / high。阶段 1 为全栈重构（TS + Rust + engine/ Python 管线）单代理执行；阶段 2（技能/契约文本）依赖阶段 1 落地后派发；阶段 3 由 ARCHITECT 亲自复跑验证。技术前提已核实：`web_api_adapter.py` `select_model`（session/selectModel）支持按会话传 reasoningEffort，每次任务独立会话即等效按任务生效；`select_model` 目前无调用方，需补提交链管线。
>
> **阶段 1 执行结果（2026-11-17，子代理 Kierkegaard，ARCHITECT 复核中）：** 分类调用/意图路由/落盘决策/提问卡/off 档位管线已落地（workIntentDecision.ts 新增统一契约；提交链贯通 lib.rs → task_core → select_model）。cargo 1209 全过、tsc 0、build 通过、verify_arch_176 55/55、引擎单测 101 全过。复核发现一处待补：`pickWorkIntentEffort` 未做 off 能力预判——声明集合不含 off 的模型会被运行期拒绝（OD-ARCH-175 已证实的同类问题），并入阶段 2 修复；selectModel 持久化影响面列为阶段 4 真实验证项。
>
> **阶段 2–3 执行与复核结果（2026-11-17）：** 阶段 2（Aristotle）落地 off 两层能力预判（声明集合明确不含 off → 跟随默认）与 period-review 技能文本更新，verify_arch_176 增至 70/70。阶段 3 ARCHITECT 实跑：cargo 1209/1209（flaky 不复现）、tsc/build/diff-check 通过、verify_arch_176 70/70、conversationPrompt 33/33、bindings 69/69、089 86/86、103 52/52、104 73/73、503/505 通过、202 39/40（既有失败）。复核结论见 REVIEW.md。**阶段 4 真实模型验证待用户执行**（三类输入端到端、selectModel 持久化影响面、声明不含 off 模型、安装副本技能文本迁移）。

### OD-ARCH-177：配套插件推荐指引 + anysearch 技能环境适配（待批准，2026-11-17）

> **触发：** Windows 实测发现引擎环境没有 anysearch CLI（技能自述为薄包装，实体在 Codex 技能目录），模型浪费数轮探索后回退 web_fetch。**用户裁决：插件会升级或过期，不把插件/CLI 打包进仓库；改为在功能管理/设置界面提供配套插件推荐——插件名、地址、建议版本。**

**阶段 1（技能文本环境适配，先行）**
> 1. 改写 `skills/anysearch/SKILL.md`：明确「OmniDesk 引擎环境默认没有本机 Codex 技能目录与 ANYSEARCH_SKILL_DIR；检测不到 CLI 时不要按报错指引反复重试，直接改用 web_fetch/内置检索，并如实标注来源与限制」。保留凭据与红线约束不变。

**阶段 2（设置界面配套插件推荐）**
> 1. 设置页新增「配套插件」区块（只读元数据）：插件名、地址（仓库/安装页）、建议版本、用途一句话；首条为 anysearch CLI（指向其上游仓库与建议版本）。数据为仓库内置静态清单文件（非机密、无自动安装、无遥测）。
> 2. UI 遵循「不堆次要文字」原则：列表本身传达信息，不配长说明。
> 3. 明确不做：不自动下载/安装/升级任何插件；不内置 CLI 脚本；不做版本检查联网请求。
> 4. **常备插件清单（2026-11-17 用户裁决）：** anysearch 与 dsh-cost-meter 两项均列为常备推荐插件，各含插件名、地址、建议版本与用途一句话。

**阶段 3（验证）**
> 1. verify 脚本：清单结构与渲染断言；`tsc`/`npm run build`/`cargo test --lib` 回归。
> 2. 真实模型验证（用户执行）：引擎环境中确认技能按新文本直达 web_fetch 回退。

### OD-ARCH-178：思考档位声明进发行链 + 目录驱动档位选择（待批准，2026-11-17）

> **触发（用户 0.2.32 实测 + 407s 回合日志）：** ①Mac 安装的 0.2.32 中，普通对话路径把用户所选档位（low/off）无证据下发给未声明模型，被 selectModel 拒绝，连续 4 次失败（0.2.33 的证据门只覆盖工作记录场景）；②发行链的 opendesign 模型**没有声明 reasoningEfforts**（声明只存在于 dev profile patch），未声明 = DSH selectModel 拒绝一切显式档位 = 用户永远无法设置思考等级，模型跑在默认重思考档；③一个本可 8 秒回答的问题花了 404 秒：62 次模型调用、上下文 5509→76106 token（13.8 倍，单条 54KB bash grep 是断崖）、约 10 个死胡同步、提示词收敛规则无硬性执行。
>
> **DSH 原版设计（对照基准）：** Profile 按 model 声明 `reasoningEfforts` → modelCatalog 向 UI 暴露 → UI 只提供声明过的档位 → session/selectModel 校验并装档 → agentDefaultModel 后台持久化（最近一次选择会成为引擎默认，OmniDesk 需知情使用）。OmniDesk 发行链缺的正是声明这一环。

**阶段 A（声明进发行链，核心）**
> 1. 宿主侧能力清单：在仓库维护 opendesign 模型档位声明（deepseek-v4.1-flash 用 dev patch 已实测的 7 档映射 off→none 等；其余模型实测后逐个补充），由 `model_routes`/`render_overlay` 在生成覆盖文件时并入 model_entries 的 `reasoningEfforts`，使 modelCatalog 与 selectModel 两端一致生效。
> 2. 已安装用户随 0.2.34 升级 + 「修复缺失/恢复出厂内容」获得声明。

**阶段 B（全路径证据门收口）**
> 1. 核对 4 个 submitTask 站点（analyze / routed / classify / weekly）之外是否仍有未走证据门的路径；普通对话路径同样接入两层证据，未声明一律不传档位、静默降级。

**阶段 C（ModelPicker 目录驱动，DSH 原版行为）**
> 1. 档位按钮按当前模型的声明档位渲染；未声明模型只显示「跟随模型默认」，不再提供会被拒绝的选项。

**阶段 D（407s 类问题的硬化，独立后续）**
> 1. bash 通道输出字节上限（tool-fs 上限不覆盖 bash，54KB 价目表灌入即断崖来源）。
> 2. 宿主侧步数硬拦截与「连续 N 步无新事实」强制收敛。
> 3. 排查「上一条报错文本被当作 prompt 回灌」的缺陷（日志 turn 4）。

**明确不做：** 不改 runtime/；不为未实测的模型声明档位；不在本计划内做 bash 沙箱改造。

**验证：** 真实 DSH 场景回归（声明模型选档 → selectModel 成功 → 生效；未声明模型选档 → 静默降级不报错）；UI 断言；既有 verify 全过；0.2.34 发版后用户 Windows/Mac 双端实测。

> **执行结果（2026-11-17）：** 阶段 A/B/C 由子代理 Heisenberg / Zeno（eco + high）并行完成，ARCHITECT 复核通过并亲自完成真实 DSH 双向验证（正向：声明后 selectModel 接受 off 任务成功；负向：未声明优雅降级 4/4）；用户批准随 0.2.34 发版实测。

### OD-ARCH-179：阶段 D 立项结题——pi 极简口径的收敛硬化（已实施，2026-11-17）

> **触发：** 用户批准 D 项立项，要求参考 pi-agent 极简设计思路升级、不再出错。勘察结论先行：D 项原列的三件事在 runtime 配置面上大多没有杠杆——①bash 工具已有「截断到尾部 + 溢落盘」，但阈值是 runtime 内部常量，tool-bash 配置面只有 runInBackground/promoteOnTimeout；②agent-loop 无任何步数预算配置面；③「报错回灌」经会话事件流核实为 DSH 原版 `agent/inbox/spliced` 机制（失败原因拼进收件箱供模型自适应），非缺陷。按极简原则：不加宿主拦截层、不改 runtime、缺口如实记录（见 docs/decisions.md D-109），把杠杆集中在唯一可配置面——preset persona 规则与已落地的档位声明。

**实施（ARCHITECT 亲自执行，改动 1 处）：**
> 1. `runtime/dsh-home-seed/profiles/discipline/cordis.patch.yml` discipline-core persona 追加「自查收敛规则」（两条）：OmniDesk 自查类问题先读 app/config 两个配置文件各一次再作答（404 秒回合在第 44 步才读到答案文件）；bash 禁止对 storages/、node_modules/ 全目录 grep/find，大范围检索走 tool-fs-search（自带条数上限）。work-review-core 无 shell 不适用，不加。

**验证与边界：**
> - 既有 verify 回归不受影响（该文件无内容断言依赖）；发版后已安装用户经「修复缺失 / 恢复出厂内容」获得规则。
> - 明确不做：不改 runtime/；不做宿主侧步数拦截；bash 截断阈值与步数预算记为 runtime 上游缺口（D-109 已登记复审触发条件）。

### OD-ARCH-180：Skill 规范与生成流程升级（待批准，2026-11-17）

> **触发：** 用户评审 ChatGPT（gpt-luna）的 OmniDesk Skill 体系设计方案（对话 6af97cf4）。ARCHITECT 判断：方向正确（与 agent-skills 渐进披露、OD-ARCH-174 的按需加载一致），设计方式约七成可采纳，但范围需按 OmniDesk 纪律压缩——不建 Skill 管理平台/生命周期状态机/评分面板，业务域内容不入公开仓库。

**已核实的现状（判断依据）：** ①OmniDesk 的 Skill 机制是真渐进披露：目录仅暴露名称/描述/指针（约 2410 字符），模型按需调 skill 工具读正文——提案里「是真正按需加载还是光放文件」的疑虑已不成立；②现有 10 个 skill 均为单 SKILL.md（35–97 行），无 references/assets 目录，正文短、无必要拆分；③仓库没有 skill-creator；④各 skill 结构不统一（无统一触发/排除/质量清单章节）。

**阶段 1（规范，文档 + 断言）**
> 1. 新增 `docs/skills-spec.md`：最小统一结构（必选六段：用途 / 触发与排除 / 输入与依据规则 / 执行步骤 / 输出契约与质量清单 / 失败处理）、frontmatter 必填字段、命名与目录规范（references/ 仅当正文超 120 行或含大素材时启用）、安全边界（无真实业务材料与凭证）。
> 2. 新增 `verify_skills_spec_180.py`：对 `skills/*/SKILL.md` 断言必填字段与六段章节存在；现有 skill 不合规项列出缺口（不强制一次性重写）。
>
> **进展（2026-11-18）：** 规范草稿已写（docs/skills-spec.md，含示例与“快速判断标准”），待用户评审修改后冻结；逐步实施验证脚本。

**阶段 2（skill-creator 轻量版）**
> 1. 新增 `skills/skill-creator/SKILL.md`：按规范生成/审核 skill 正文与边界检查（与现有 skill 重叠检测、越权工具检测），产出模板与待验证清单；不做测试编排与版本发布。

**阶段 3（存量审计与试点）**
> 1. 对 10 个现存 skill 跑规范断言，缺口逐条记录；选 1 个低风险 skill（daily-summary 或 knowledge-search）按六段补齐作为试点，其余随迭代补齐。
> 2. 真实模型抽测：试点 skill 的触发准确率与输出契约（用户当轮允许后执行）。

**明确不做：** Skill 生命周期状态机、质量评分面板、管理界面、references 目录强制、纪检/业务域 skill 入库（用户自有材料放仓库外私有目录，按 AGENTS.md 数据边界）；不改 runtime/。

**风险：** 存量 skill 结构差异大，统一六段可能需要渐进改造（本计划只断言+试点，不批量重写）；skill-creator 与 Codex 内置同名技能语义不同，文案需明确「本仓产出物是 OmniDesk skill」。

### OD-ARCH-181：逐模型手动声明思考档位（已实施，2026-11-18）

> **触发（用户裁决）：** 用户要求把设置页的思考等级/思考模式控制移到每个模型后面，由用户自己手动声明档位并传递给 DSH——「选定的模型我自己知道有什么档，出错说明模型不支持，我再改回来」。即：用直接控制替代宿主白名单逐个实测。

**实施（ARCHITECT 亲自执行）：**
> 1. `ModelEntry` 新增 `reasoningEfforts`（用户手动声明，持久化进 model-settings.v2.json）；写入覆盖文件时优先于 Profile 继承与宿主已验证清单（merge 只补缺失键，用户声明完整覆盖）。
> 2. 设置页「模型与端点」每个模型行下新增 7 档切换 chips（关闭/最低/低/中/高/很高/最高），off 取值按网关惯用映射 none、其余同名；再点取消；全部取消即回退「跟随模型默认」。保存校验档位键与取值合法性。
> 3. 宿主侧保留 provider 级思考等级/思考模式字段（兼容旧配置）；逐模型声明未设置时沿用「Profile > 已验证清单」的既有合并顺序。

**验证：** cargo test --lib 单线程 1218/0（新增「用户声明覆盖 Profile 与清单」单测）；tsc / npm run build 通过；verify_arch_178、arch_176 71/71、202 39/40（既有失败）全过。本改动编译进 Rust，需新构建（0.2.35 或 dev）后才能在设置页看到 chips。
>
> **微调与发版（2026-11-18，用户实机确认档位生效后）：** ①设置页 7 个档位按钮缩小约一半（字号 9px、紧凑内边距）；②对话模型选择器去掉「跟随模型默认」，档位存在时直接列声明档位、为空时单按钮兜底；随 0.2.35 发版。

### OD-ARCH-182：外发脱敏边界收口（待批准，2026-11-18 紧急）

> **触发（用户 0.2.38 实机自检）：** 含实名、企业、金额与内部职务的材料经工作区知识库文件被模型以工具读取后，明文发送到公网端点（opendesign / amr-link.open-design.ai）。核查结论：**提示词通道的脱敏器默认开启且在工作；漏洞通道是工具读取的文件内容——工具结果由 DSH 引擎直接进模型上下文，OmniDesk 脱敏器不接入该链路**（masker.py 自述「不接入 LLM 调用链」）。叠加因素：敏感字典默认为空（示例仅合成词条）、金额无默认规则、人名/机构为启发式（宁可少遮）、红线扫描不覆盖工具读取内容。属用户自述的「未做实质脱敏」，按项目规则「启发式扫描不等于代码级安全控制」——这层从来不是安全边界。

**范围与原则：** 在 OmniDesk 自有层补可见性和可控边界；不改 runtime/（工具结果钩子是 DSH 上游能力，记缺口）。

**阶段 A（可见性，立即）**
> 1. 外发端点判定：非 localhost 端点时，界面提示「脱敏仅覆盖提示词通道，工具读取的工作区内容不受脱敏保护」；敏感字典为空时同日提示「未配置人员/机构词典」。
> 2. 每轮对话展示脱敏状态：已脱敏处数与类型；脱敏被 OMNIDESK_MASKER=off 关闭时显式告警。

**阶段 B（默认规则补强）**
> 1. masker 增加金额模式（数字+万/亿/元、区间写法）→ [AMOUNT]；统一社会信用代码（18 位）→ [USCC]；保持既有身份证/银行卡/手机/邮箱规则。
> 2. 规则命中率单测（合成样例），避免误遮普通文本。

**阶段 C（工具读取边界的工程替代，重点）**
> 1. 「脱敏镜像工作区」：工作区文件入库/整理时生成脱敏副本，对话工具只读镜像目录；原文件不进入工具可读路径（或场景规则明确禁止读原目录）。需评估对现有工作记录/知识库路径的影响。
> 2. 或者：知识库类文件的对话读取统一走 knowledge_ingest 的脱敏管线（已实现），模型不直接读原文件。
> 3. 二选一须先做路径影响评估，再定实现。

**明确不做：** 不改 runtime/；不承诺「脱敏=授权」（案件、线索、人物画像等禁入材料仍禁止进入工作台，规则不变）；不把真实词典写入仓库。

**验证：** 合成样例端到端（提示词与工具读取两条通道各自的可观测结果）；既有 masker 单测全过；真实窗口提示文案核对。

> **用户裁决（2026-11-18）：** 不做阶段 A 的外发可见性建设；只要求在宿主端对话框输入向外发模型时先做脱敏，规则保持简单（名字、银行卡、电话等部分替换）。**实施结果：** masker 补强——金额（万/亿）、名单式人名、无后缀企业名（两家/三家/等+企业）、扩展姓名触发词、常见词防误遮（金额/金融等）；对话两条路径统一显示「已自动脱敏（类别 处数）」；顺带修正一处自 0.2.33 起失效的适配器单测（selectModel 拒绝档位应为降级而非抛错）。验证：masker 46/46、markdown 38/38、adapter 101/101、tsc/build、masker UI/audit 脚本全过。**未覆盖（如实告知）：** 模型用工具读取的工作区文件内容仍不经过脱敏——该通道不在输入侧；阶段 C 的镜像方案仍待另行评估。

**未验证（用户实测）：** 声明不实档位时真实调用报错的呈现与回改流程；设置保存后选择器档位刷新的时机（保存会同步覆盖文件并重启引擎）。

### OD-ARCH-175：未声明思考档位渠道的顺畅兼容（待批准，2026-11-17）

> **触发：** 用户测试 volcengine / deepseek-v4.1-flash：未声明档位导致显式档位切换被预检阻断（错误一），且该渠道模型发现端点 404（错误二，不阻断）。用户方向：未声明模型默认不添加思考等级；用户设置了档位就尝试使用，失败自动回退默认并告知用户，确保切换与使用顺畅。
>
> **研判结论：** 现有架构已有一半机制——已声明能力走本地预检（api.ts + check_effort_capability），未声明能力由 DSH 在首次调用拒绝（model_settings.rs 注释明确运行期 UNSUPPORTED_REASONING_EFFORT 报错）。本计划把「未声明/必败」情形从阻断改为「尝试或本地回退 + 告知」，全部实现在 desktop/ 宿主层，**不改 runtime/**。
>
> **派工（2026-11-17 用户批准执行）：** 子代理执行（BUILDER 模板），指定模型 deepseek-v4.1-flash-eco + 思考等级 high。阶段 1（只读核查）与阶段 3（TS 实现）写集不相交，并行派发；阶段 2（Rust）依赖阶段 1 结论后派发；阶段 4 真实模型验证由 ARCHITECT 复核后组织。若 eco 在派工通道不可用，回退 OpenDesign/deepseek-v4.1-flash + high 并在 STATUS.md 记录偏差。

**行为矩阵（核心设计）**

1. **未声明/目录不可用**（如 volcengine 未声明 efforts，或目录探测失败）：预检放行，用户档位照写覆盖文件（尝试使用）。首次调用若被 DSH 拒绝，宿主识别运行期档位错误后自动回退：重写覆盖文件去掉 reasoningEffort、重启引擎、在会话中告知用户「该模型不支持所选档位，已回退默认档，请重发任务」。失败任务不自动重发（避免重复副作用），复用现有 retryable 重发入口。
2. **已声明且档位匹配**（如 opendesign 7 档）：现状不变，正常下发。
3. **已声明但不匹配/声明为空**：本地 100% 确定显式档位必败，不发请求，切换直接按默认档完成并告知用户「已按默认思考等级切换」。这把「不成功就继续使用默认，告知用户」的判定提前到本地，省一次必败调用。

**实施阶段**

> **阶段 1（验证错误形态，先行）：** 用隔离 Profile 或真实切换实测 DSH 对未声明模型显式档位的运行期拒绝形态（错误码/error= 段原文），确定宿主识别条件。形态与预期不符则回滚设计再研判。
>
> **阶段 2（宿主回退链路，Rust）：** 任务状态解析处识别档位拒绝错误 → 自动重写覆盖文件（select_route 去掉 reasoningEffort）→ stop_engine 重启 → 写入一条非机密回退记录供 UI 读取。并发保护复用 lifecycle.begin_change；回退动作只做一次（防循环）。
>
> **阶段 3（切换预检改造，api.ts + UI）：** setModelRoute 移除阻断式 throw；情形 3 本地直接以 default 提交并返回 advisory；情形 1 照提交并返回「可能被运行期拒绝」提示。HomeChat 用现有 notice/retryable 机制展示回退通知，文案一句（UI 原则：不堆次要文字）。
>
> **阶段 4（测试与真实验证）：** Rust 单测覆盖三种情形判定与回退重写；verify 脚本更新；真实模型验证 volcengine 切档（切换→发消息→观察回退与提示），结果落 STATUS/REVIEW。真实模型调用须用户当轮允许。

> **阶段执行结果（2026-11-17，ARCHITECT 复核通过）：** 阶段 1（子代理 Plato）确认运行期拒绝文本与触发条件，且发现未声明模型显式 off 可用；阶段 3（Aristotle）落地两层证据预检（目录+Profile 原始声明），tsc/构建/verify 12+11 全过；阶段 2（Fermat）落地 Rust 自动回退（精确解析、单次 guard、后台停引擎），cargo test --lib 1209/1209（另见 3 个环境性 flaky 不复现）。复核结论见 REVIEW.md。**阶段 4 真实模型端到端验证未执行**（需重编译桌面应用并实际切换 volcengine，待用户当轮执行）。

**明确不做：** 不改 runtime/；不自动重发失败任务；不建模型能力缓存/注册表；不动模型发现 404 的探测逻辑（错误二另行小改或不改）；不新增设置项。

**风险：** 运行期错误形态未实测（阶段 1 先行解决）；回退重启时机与用户并发切换竞态（已有 lifecycle 锁缓解）；声明为空的本地回退偏离「尝试」字面、属优化取舍（用户可裁决改为也照发一次）。

### OD-ARCH-174：Skill 优化——指针补全、目录瘦身、正文重写（待批准，2026-11-17）

> **触发：** 0.2.30 完成后用户要求优化 Skill。勘察证据：①真实模型 smoke（f1e06a3）确认知识整理任务不读 Skill，根因是 knowledge 场景在 rules.json 无 skill_bindings，模型没有"该用哪个技能"的指引；②每次任务注入 skill-catalog 2518 字符（10 技能），其中 4 个（anysearch/document-review/material-writing/knowledge-search）从不被模型自主调用，目录对它们是纯开销；③写材料/项目场景与周报共用默认分支的指针来源（presetSkillsFor(preset,"report")），边界情况可能拿错技能指针；④10 个 Skill 正文约 36KB，是全文注入时代写的冗长文风，按需读取时代可收紧。
>
> **用户裁决（2026-11-17）：** ①知识库场景只需要检索知识卡（绑定 knowledge-search），知识卡沉淀发生在任意场景（工作/写作/无分类），不是独立场景的职责；②阶段 C 必做。
>
> **派工：** 子代理 OpenDesign/deepseek-v4.1-flash-eco + high，按 BUILDER 模板逐阶段派发；每阶段 ARCHITECT 复核后进下一阶段。真实模型验证用现行可用 key。

**阶段 A：指针补全与绑定修正（低风险）**
> 1. 修 writing/project 场景指针来源：conversationPrompt.ts 默认分支（mode=report）不应无条件走 presetSkillsFor(preset, "report")；按当前场景取各自绑定（writing→material-writing、project→project-agent），周报场景维持 memory-report。改动后用 Node 直接调组装器验证四场景指针各归其位。
> 2. knowledge 场景补绑定：knowledgeContext 命中时（宿主已检出的检索上下文），在 knowledgeBlock 追加 knowledge-search 的技能指针（≤260 字符，含"两跳检索/先读索引.md"要点）；无命中不注入。不动 rules.json 的场景结构（knowledge 不是独立场景，是横切能力）。
> 3. 验证：verify_conversation_prompt / verify_skill_bindings_082 断言更新并全过；tsc 0。
> 4. 执行结果（2026-11-17，ARCHITECT 复核：代码与测试通过，发现注入点策略缺口，见第 5 条）：defaultSceneSkills 修正四场景指针串号；knowledgeSkillPointer 落地（151 字符）；tsc 0、verify_conversation_prompt 33/33、verify_skill_bindings_082 62/62、worklog_089 85/85、ai_draft 27/27。
> 5. **A2 追加（ARCHITECT 裁决）：** docs/decisions.md D-104 口径"knowledge-search 不进任何绑定，纯按需"的前提已变化——阶段 2 造出了指针注入点（260 字符不再是 4901 字正文）。裁决：新增专用绑定键 `knowledge`（知识检索流程），HomeCanvas 读取后作为独立参数传给 composeConversationPrompt（不混入场景级 skills）；出厂 discipline.rules.json 增加 `knowledge → knowledge-search` 绑定；同步更新 D-104 决策记录（注明复审触发：OD-ARCH-174 指针机制）。这样 knowledge 指针在默认 Profile 即生效，且不污染四场景主指针。
> 6. **A2 执行结果（2026-11-17，ARCHITECT 复核通过）：** knowledge 专用绑定键落地（rules.json + HomeCanvas 独立读取 + HomeChat 透传 + ConversationPreset.knowledgeSkills）；普通对话 preset=null 也注入知识指针；D-108 决策记录完整（含复审触发）。tsc 0；verify_conversation_prompt 33/33、skill_bindings_082 69/69、worklog_089 86/86、cargo 1200 全过。已安装 Profile 的 rules.json 迁移待用户「恢复默认规则包」。

**阶段 B：目录瘦身（低风险）**
> 1. 对从不被模型自主调用的 4 个技能（anysearch/document-review/material-writing/knowledge-search）在 SKILL.md frontmatter 加 disable-model-invocation: true；保留 user-invocable。效果：模型自主目录不再列它们（估省约 400 字符），流程绑定与用户手点不受影响（绑定注入走 skill_contents，不走目录）。
> 2. 同步仓库 skills/ 与 sync-skills.sh 校验；mock 会话复测目录字符（预期 2518 → 约 2000）。
> 3. 验证：mock 前后对比落 evidence/od-arch-174/；相关 verify 脚本全过。
> 4. **B 中途裁决（ARCHITECT，基于阻断性发现）：** disable-model-invocation 同时关掉目录条目与 skill 工具执行（dsh-tool-skill execute 二次校验），与指针机制冲突。裁决按"能力优先、目录开销其次"收缩：material-writing、knowledge-search 撤回标记（它们的绑定指针要求模型调 skill 读正文）；anysearch 撤回标记（阶段 1 计划明确公开检索由模型按需走 anysearch，自主可见是能力的一部分）；只保留 document-review 的标记（纯用户手点场景，无绑定指针指向它，目录省约 110 字符）。目录 2518 → 约 2410，收益缩小但零能力损失；后续若 DSH 拆分"目录可见"与"可执行"两个开关，再回来收这 4 个。
> 5. **B 执行结果（2026-11-17）：** 4 标记实验证明 disable-model-invocation 使 skill 工具返回 "not available for model invocation"（阶段 A/A2 的指针会失效）；按裁决撤回 3 个、保留 document-review；mock 目录 2518 → 2410 字符；verify 33/33、69/69、86/86 全过。
> 6. **B 最终（复核通过）：** 实测目录 2518 → 2401 字符（−117）；仅 document-review 从模型目录消失；sync --check 一致；mock 三任务 succeeded。既有缺口记录：sync-skills.sh 在 macOS 需绕过 BSD realpath（未修）。
> 7. **C 执行结果（2026-11-17，ARCHITECT 复核通过，含真实模型端到端验证）：** 三技能正文压至原 63–70%（memory-agent 1754→1228、memory-report 1099→749、period-review 1820→1268），契约与硬约束逐字保留（082 69/69、089 86/86、503 58/58、505 71/71）。**真实模型端到端**（新驱动 run_custom_prompt.py 送桌面组装器产物）：模型收到指针后实际调用 skill(name=memory-report) 读到重写正文，随后按五段模板产出（本周概述/事项进展/问题与风险/下步计划/待确认事项全在）——"指针→按需读取→按契约输出"全链路真实模型确认。knowledge 任务的模型未触发 skill 读取属模型自主决策（宿主检索上下文已直接给出），非回归。
>
> **OD-ARCH-174 完成总结：** 四场景指针各归其位；knowledge 专用绑定键落地（D-108）；目录 2518→2401；三核心技能正文 −30~37%；真实模型全链路验证通过。已知缺口：sync-skills.sh macOS realpath 兼容（未修）；已安装 Profile 的 rules.json 需「恢复默认规则包」获得 knowledge 绑定；阶段 C 其余 7 个技能未重写（anysearch/document-review 等按需技能留待后续按使用频率决定）。

**阶段 C：核心技能正文重写（中风险，用户必做项）**
> 1. 重写 memory-agent / memory-report / period-review 三个核心技能正文：删除全文注入时代的冗余（重复平台契约的写盘边界说明、解释"平台会替你做什么"的段落、与 conversationPrompt 指针重复的读取指引），保留输出契约（五段模板/判定结构/标题深度）与硬约束（引用编号/不虚构）。
> 2. 重写原则：模型通过 skill 工具读全文时，读到的是"纯任务指令"；平台契约（写盘边界、宿主计算）只在指针与平台提示词出现一次，不在技能正文重复。
> 3. 每个技能重写后：先 diff 说明删了什么留了什么，再跑对应 verify 脚本（082/503/505）确认输出契约断言不变；三个全过才继续下一个。
> 4. 真实模型回归（现行 key）：随手记/周报/复盘各 1 次，对照 0.2.30 smoke 的产出结构；结果落 evidence/od-arch-174/。
>
> **明确不做：** 不动 knowledge-agent（沉淀型技能留给用户手点场景，本轮不改正文）；不改 DSH tool-skill 的目录机制（disable-model-invocation 是既有配置项）；不改 runtime/；不动凭证；知识卡"任意场景沉淀"的流程不变（卡片落盘仍由 knowledge-search 的建卡路径 + 宿主执行）。
>
> **验收：** A/B/C 每阶段 mock+脚本证据齐全；C 结束后真实模型三类任务产出契约与 0.2.30 smoke 一致；目录与注入总量显著下降；全部验证脚本通过或既有失效如实记录。

### OD-ARCH-173（v3，最终版）：0.2.30 成熟、高效、安全基线（已完成并通过验收，2026-11-17；release 提交 1d07d91bba，验收记录见 REVIEW.md）

> **本版定位：** 本节是 0.2.30 的唯一执行计划，取代下方 OD-ARCH-172（v2）。v2 由 DeepSeek 起草（提交 1c3514a），ARCHITECT 复核后确认：三主线（效率/安全/成熟）与"明确不做"边界成立，阶段顺序合理；但存在四处问题——①阶段 1 遗漏 11-16 已合入的 PTC 修复（OD-ARCH-154）与 11-15 已修的思考等级通道（OD-ARCH-153），直接执行会重做或冲突；②阶段 2"Skill 按需加载"依赖 DSH tool-skill 的实际行为，未先验证就承诺验收指标；③阶段 0 的两个基线脚本可与现有 diagnose-omnidesk.py 合并观测口径，避免重复造轮子；④阶段 4 模型预检与 model_settings.rs 现状（provider/api/apiKeyEnv/reasoningEffort 已落）存在字段重叠，需先盘点再补差。本版按此收窄与排序，执行主体为子代理 OpenDesign/deepseek-v4.1-flash-eco + high（用户当轮明确授权派工，符合 AGENTS.md 派工例外条款）。

**与 v2 的差异（先读这里）：**
1. 阶段 0 基线脚本改为"先复用 diagnose-omnidesk.py 的进程/端口/配置元数据，再补提示词分段与时间线拆分"，不新建两套入口。
2. 阶段 1 前置确认两项已合入修复不回退：PTC（tool-presentation/present 已在 discipline-core）与 thinking 等级（agent-default-model 块已写 reasoningEffort）。
3. 阶段 2 拆成两步：先只读验证 DSH tool-skill 能否按需读 Skill 正文（合成会话 + 日志证据），验证通过才改 conversationPrompt 注入策略；不通过则保留全文注入并如实记录缺口，不硬砍。
4. 阶段 4 改为"盘点 model_settings.rs 已有字段与 DSH 侧缺失项，只补差"，不重写已有 provider/api/apiKeyEnv/thinking 字段。
5. 全程不动 runtime/；不改 OD-UI-717 已批准的纸面风格方向（阶段 1 的 persona 精简只改提示词文本，不改样式）。

**阶段 0：基线（只读，半天）——已完成，证据 tests/verification/desktop/evidence/od-arch-173/baseline.md**

**阶段 1 补充裁决（ARCHITECT 复核后追加，2026-11-17）：**
> 1. PTC 与提示词预算冲突：保留 PTC 时 system 15252 字符（PTC 类型注入 12098），去 PTC 时 2983。裁决：保留 PTC（OD-ARCH-154 是已验收的批处理能力，价值大于提示词体积；且 PTC 减少工具往返本身服务同一效率目标）。阶段 1 验收改为"web-search 段归零 + 去 PTC 口径 ≤3000 字符"，≤1000 tokens 目标仅对非 PTC preset 有效。
> 2. 阶段 3 增补任务：检查 discipline.profile.json 的 dependencies 与 profile_assets.rs 的 DISCIPLINE_PLUGINS 中残留的 dsh-free-search 声明，随工具面收敛一并清理（这属于"按动作收敛工具白名单"的同一范围）。
> 3. 阶段 6 增补任务：现有安装的 Profile 需要重播种/迁移才能移除已安装 bundle 中的 dsh-free-search，纳入发布步骤。
> 4. 真实模型复测统一推迟到阶段 6 smoke test（402 余额问题需用户处理；mock 复测已足证提示词结构变化）。
> 1. 记录 HEAD/git status；确认 OD-ARCH-153/154 两项已合入修复在工作树中未回退（grep 即可）。
> 2. 基于 scripts/diagnose-omnidesk.py 扩展只读模式：输出最近一次真实会话的提示词分段字符数、工具调用次数、首轮响应耗时、总时长；不输出正文与凭证。
> 3. 在隔离数据根跑三类合成任务各一次（Excel、周报、知识整理），基线表落 tests/verification/desktop/evidence/od-arch-173/。
> 4. 验收：同一张表能回答"397 秒里模型/工具/审批/压缩/缓存各占多少"，并作为后续阶段对照。

**阶段 1：提示词装配（效率第一刀）**
> 1. discipline-core 与 work-review-core 的 persona 前缀从 "You are a coding agent powered by the {{model}} model." 换成一句 OmniDesk 底座：个人工作台账助手，只读写用户明确指定的文件，日期与统计由宿主计算，不确定时如实说明。
> 2. 从 discipline preset 移除常驻 web-search-free 插件声明（samples/profiles/discipline.profile.patch.yml 第 14 行区域），检索收敛/预算/工具选择规则同步精简（advanced_search 等细分规则随插件一起移除）；保留 tool-web 的 fetch。公开检索由 anysearch Skill 按需承担。处理搜索设置入口，避免死开关。
> 3. 日期不进系统提示词段：核实 time-context 插件实际注入位置，若进 system 段则改为运行上下文注入；cwd 保持放最后。
> 4. 目标：系统提示词 ≤1000 tokens（目标 200–300）。验收用阶段 0 脚本复测。

**阶段 2：Skill 按需加载（先验证，再改）**
> 1. 只读验证：合成会话中挂载一个测试 Skill，检查 DSH tool-skill 是否能让模型主动读取正文（证据：工具调用日志 + 会话产物）。
> 2. 验证通过：conversationPrompt.ts 不再注入 Skill 全文，改为 id/name/description/path 或 ≤300 字符摘要；每个动作最多一个主 Skill。
> 3. 验证不通过：保留全文注入，在 docs/decisions.md 记录缺口与复审触发条件（等 DSH 升级），不改坏现有行为。
> 4. 验收（若通过）：Skill 段 ≤300 字符；周报/复盘/知识整理产出契约不回归；Skill 来源（全局/Profile/动作）与同名冲突可见。
> 5. 执行结果（2026-11-17，ARCHITECT 复核通过）：验证通过并已实施。DSH tool-skill 返回全文（原生 skill_content 包装；PTC 下经 run_code 内 tools.skill），会话自带 2518 字符技能目录（19 个 Skill 的 name+description）。conversationPrompt.ts 全文注入改为 ≤260 字符指针，四动作 user 段降 61–72%。遗留既有验证脚本断言全文注入需随阶段 6 更新（verify_skill_bindings_082、verify_worklog_089、verify_skill_flow_injection_108）；skill-catalog 2518 字符是 DSH 目录注入非桌面注入，压目录需改 tool-skill 配置，超出本阶段范围，记缺口。真实模型行为归入阶段 6 smoke test。

**阶段 3：工具面与安全约束**
> 1. 按动作收敛工具白名单：搜索、Office、知识整理按需；读写、审批、副作用工具显式。
> 2. 核对 read/搜索/bash 的行数与字节上限（pi 基准：2000 行/50KB、grep 单行 500 字符）；可配置的落地，不可配置的记缺口；全量输出落临时文件。
> 3. 路径边界：cwd 显式绑定、允许根校验、写操作同文件串行、原子写、精确替换优先。
> 4. bash 安全：cwd、超时、进程树 kill、环境变量过滤、输出上限、审批保留；bash 只做后备。
> 5. 错误与脱敏：工具错误作为可恢复结果；工具结果进模型前脱敏；审批失败不升级为整轮失败。
> 6. 只读审计 DSH 是否提供 before/after 钩子、按工具串行、审批前置、turn 间热切换、abort 下发；有则配置，无则记缺口，不改 runtime/。
> 7. 交付：工具约束矩阵（每个工具的参数/输出/路径/并发/安全现状与缺口）。
> 8. 执行结果（2026-11-17，ARCHITECT 复核通过）：dsh-free-search 依赖声明已从 discipline.profile.json 与 profile_assets.rs 清除（cost-meter 保留）；工具面盘点结论为无可安全删除项，discipline-core 11 工具 / work-review-core 7 工具保持最小集合；tool-fs/tool-fs-search 显式钉住输出上限（read 2000 行/50KB、grep 单行 500 字节、glob 100 条、grep 250 条）；钩子/审批/串行/abort 审计完成，缺口（读不受 workspace 根约束、无统一脱敏、ask-user 无宿主卡片、无 turn 间热切换）记入 tool-matrix.md；cargo test 1187 全过、mock 四会话 succeeded。D-105 决策已落 docs/decisions.md。

**阶段 4：模型接入成熟度（只补差）**
> 1. 盘点 model_settings.rs 已有字段（provider/api/apiKeyEnv/reasoningEffort/contextWindow 已落）与 DSH 侧仍缺的能力项（成本/compat/模型列表预检），只补缺失项，不重写已有字段。
> 2. 保存/切换前可用性预检：模型列表 404、UnsupportedModel、认证失败分别给出可操作提示；预检不读明文凭证。
> 3. 模型目录缓存只做只读数据（目录/etag/checkedAt），刷新不阻塞启动。
> 4. 验收：模型不可用时在运行前给出明确原因；密钥不出现在 UI、日志、提示词或成果文件。
> 5. 执行结果（2026-11-17，ARCHITECT 复核通过）：Rust save() 前置档位校验（declared_efforts/check_effort_capability，6 单测）；设置页保存前端点预检（401/402/403 阻止，404/网络提示放行）+ 档位收敛（声明 false 自动回落默认）；切换路径 api.ts 用模型目录校验显式档位。cargo test 1193 全过、tsc 0。缺口如实记录：聊天期才暴露的 402/403 保存期发现不了；目录无 ETag/checkedAt 持久缓存（DSH 未暴露版本字段）；真实端点响应文本未验证（归阶段 6）。D-106 决策已落 docs/decisions.md。

**阶段 5：可观测性与配置解释**
> 1. 设置页或只读诊断入口显示：系统提示词分段与 token、工具与 Skill 可见性、配置来源、模型 compat 与 thinking 能力、压缩状态；只显示元数据。
> 2. 任务时间线显示模型往返、工具 prepare/execute、审批等待、压缩/重试、缓存命中；不显示用户正文。
> 3. 验收：用户能回答"这条行为从哪来、时间花在哪"，不需要读源码。
> 4. 执行结果（2026-11-17，ARCHITECT 复核通过）：新增只读诊断命令 diagnostics_metadata（Rust，复用会话解析思路，只出元数据不出正文）；设置页新增「配置解释」页：生效路由/api/thinking 能力/上下文窗口、提示词分段与来源、工具面清单、Skill 概况。tsc 0、npm run build 0、cargo test 1200 全过（diagnostics 7 新单测）。缺口：任务时间线逐轮耗时/审批等待/缓存命中未展示（明确不做范围）；真实窗口截图未完成（macOS 锁屏，黑图证据存 evidence），由用户验收时目视。D-107 已落 docs/decisions.md。

**阶段 6：验证、发布与回退**
> 1. npx tsc --noEmit；npm run build；cargo test --lib；现有 UI/Profile 验证脚本全过。
> 2. 隔离 Profile 的 --dump-config 与启动探针；确认 web-search-free 移除后平台启动、anysearch 可用、Skill 按需加载（若阶段 2 通过）可用。
> 3. 真实模型 smoke test（需用户当轮允许）：Excel、周报、知识整理各一次；记录提示词字符/token、工具往返、首字、总时长、缓存与压缩消耗。
> 4. 版本 0.2.30；提交推送；macOS/Windows 构建；STATUS/REVIEW 写清改前改后、证据与未验证项。
> 5. 回退：恢复 preset-standard、重新挂回 web-search-free、恢复原 persona 与（若改动）Skill 全文注入；保留旧配置备份。
>
> 6. 执行结果（2026-11-17，待 ARCHITECT 复核）：tsc 0、npm run build 0、cargo test --lib 单线程 1200/1200、cargo check 0、Tauri release `--no-bundle` 编译通过；54 个 UI/Profile 脚本中 41 通过、13 项既有失效/环境失败（HEAD 快照复现，逐项见 stage6.md）；三个 Skill 注入脚本已改为指针断言，082 58/58、089 85/85 通过，108 因 macOS 无 Xvfb 仅语法检查。版本与 CHANGELOG 已收口 0.2.30。播种迁移结论：现有安装不自动清理，需「修复缺失 / 恢复出厂内容」（写前备份）或重装；发布工作流重建 seed 后新装包生效。真实模型 smoke 因 402 未执行，Windows 未构建。提交推送与回退路径见 stage6.md。

**验收矩阵（0.2.30 必须可验证）**
> - 效率：系统提示词 ≤1000 tokens（目标 200–300）；Skill 段 ≤300 字符（若阶段 2 通过）；工具输出有上限或明确缺口；基线表能拆分 397 秒；缓存命中被记录。
> - 安全：凭证零读取/零打印/零前端存储；路径根与 cwd 显式；审批与 sandbox 保留；工具结果脱敏；无第三方代码/Extension 市场/npm 资源包。
> - 成熟：提示词/工具/Skill/配置来源可解释；模型不可用有预检；回退路径可用；现有任务与验证脚本无回归；未验证项如实列出。

**明确不做：** 不改 runtime/；不迁移线程/会话为 pi 的 JSONL session tree；不实现自有 compaction；不引入三级配置与 Project Trust 闸门；不引入 Extension 市场、第三方代码、npm 资源包；不重写模型注册表；不改 Tauri/React 通信协议；不新增 RPC/SDK/Web/TUI 宿主；不做 UI/交互重构（OD-UI-717 既有批准范围除外）；不内建子代理编排、MCP 市场、计划模式；不使用真实用户材料做测试。

**风险：** 移除 web-search-free 改变搜索入口；Skill 按需加载可能增加一次工具往返（阶段 2 先验证再决定）；DSH 钩子暴露程度未知（只读审计，有则用无则记）；提示词前缀变化可能影响缓存（阶段 0 基线对照）；模型预检可能因端点不提供列表而只能给出有限结论。每项在阶段 0 基线与阶段 6 回退中记录。

**派工方式：** 每个阶段作为一个独立任务，按 docs/BUILDER-Task-Instruction-Template.md 派给子代理（模型 OpenDesign/deepseek-v4.1-flash-eco，思考等级 high）；阶段 0 与阶段 2 的验证步骤可由子代理执行，但 ARCHITECT 在每阶段完成后独立复核源码 diff 与测试输出后才进入下一阶段；阶段 6 的真实模型 smoke test 与提交推送须用户当轮确认。

### OD-ARCH-185：稳定别名与脱敏链路一致性（阶段 1 已实施，2026-11-19）

> **触发（用户裁决）：** 用户只粘贴文字、不给文件，但要求外发链路保持脱敏一致性，并认可“稳定别名 + 镜像工作区”方向。授权执行。

**阶段 1（已实施）：稳定别名表**
> 1. 新增 `engine/omnidesk_engine/security/alias_store.py`：本地 `<数据根>/config/pseudonym-map.v1.json`，存 (类型, 原文, 别名) 三段；0600 权限、原子写、损坏容错；金额不持久化（避免映射表失控）。
> 2. masker 接入：启用时同一实体跨会话/跨文档使用同一掩码串（优先词典/规则命中的稳定别名，形式仍为 张** / 芭某公司 这类自然掩码）；unmask 用持久别名还原旧文档里的 ** 形式；report 增加 alias_restored。
> 3. 宿主注入 `OMNIDESK_ALIAS_MAP` 到引擎子进程（与敏感词典同目录），默认启用、无弹窗、无授权步骤。

**阶段 2（未实施，触发条件）：镜像工作区**
> 当出现“模型需要读取本地原文文件”的真实需求时再立项：文件加入材料时宿主生成脱敏副本、工具只读镜像；受限于 DSH 读不设工作区根的 runtime 缺口，镜像只能靠约定抑制，不能代码级强制（复审触发：runtime 提供读边界或沙箱时改回强隔离）。

**已知边界：** 别名表只解决“同一实体掩码稳定”，不提高检测覆盖率——裸人名/无后缀企业仍靠敏感词典精确兜底；别名文件本身是敏感文件（本机 0600，不进 Git、不外发）。工具读取原文通道仍在（阶段 2 前不消除）。

> 已完成的旧计划已归档到仓库外 `MyWorkStation-archive/2026-11-17/process-docs/`。


### OD-READ-002 阶段 3：健脑室侧栏自动折叠 + 首页统计条底部居中（2026-11-19，用户指示）

> **触发（用户 4 条改进）：** ①右侧 AI/笔记面板常态化折叠，收起时只显示图标栏，点击图标或划线动作条动作自动展开，90 秒无右侧栏操作/输入自动收起；②首页读书展示条底部居中；③正文多媒体/Markdown 显示；④RSS 文章计入阅读字数。经核对：③④已在 483e2b4177 落地（rss_store 已转 img/audio/video 标记，reading_stats.chars 单测覆盖 RSS），本轮只做①②并验证③④。

**实施（BUILDER 执行）：**
> 1. `desktop/src/shell/ReadingPage.tsx`：`panelOpen` 默认改为 `false`（宽窄屏一致）；面板收起时不再整体 `hidden`，改为右侧固定 40px 垂直图标栏（笔记图标 + 批注数角标、AI 图标、展开箭头），样式与 Dock/顶栏图标按钮一致；点击图标栏条目 → 展开面板并切到对应 tab；划线动作条的 划线/想法/标段 → 展开并切 notes，问 AI → 展开并切 ai（既有 `setPanelOpen(true)` 调用点保留）。
> 2. 90 秒自动收起：面板展开期间，监听面板内的 click/keydown/scroll/pointerdown 重置计时器；任意一次交互后 90 秒无新事件即 `setPanelOpen(false)`；面板收起或切走书目时清除计时器；主阅读区滚动不重置计时器（以右侧栏操作为准）。
> 3. `desktop/src/shell/HomeCanvas.tsx` + 样式：`ReadingStatsBar` 槽位改为贴底居中——槽位 `shrink-0 flex justify-center`，统计条自身宽度按内容收缩（去多余拉伸），保持可点击跳转健脑室。
> 4. 验证③：打开一本含图片/链接/Markdown 的书与一篇含图 RSS 文章，确认图片渲染、超链接可点、行内 Markdown 生效；划线偏移锚点不因富媒体偏移（现有 `verify_reading_003_selection.py` 回归）。
> 5. 验证④：`reading_stats` 单测已断言 `chars == 书籍 + RSS`；补充前端统计条展示「字数」含订阅文章的静态断言。
> 6. 回归：`cargo test --lib -- --test-threads=1`（设 `OMNIDESK_REPO_ROOT=/Users/yvan/Code/MyWorkStation`）、`npx tsc --noEmit`、`npm run build`、verify_reading_003/004 全过；证据追加到 `tests/verification/desktop/evidence/od-read-002/stage3.md`。

**明确不做：** 不改划线锚定数据结构；不动 rss 抓取链路；不做面板宽度记忆持久化；不提交不推送（由 ARCHITECT 复核后另派提交任务）。


### OD-READ-002 阶段 4：健脑室阅读区新拟物视觉改版（2026-11-19，用户批准执行）

> **触发（用户裁决）：** 认可设计样稿（reading.css + ReadingPage.tsx 样式层），要求按新拟物方案改版书/文阅读区，并「AI 划线浮动条同步设计」，授权执行。样稿路径：/Users/yvan/.codex/visualizations/2026/11/18/01a1ed5a-db65-7780-a2e2-c620b62cb67d/reading-neumorphic-mockup.html（默认态/展开态两张 PNG 同目录）。
> **借鉴要点（≤2 句）：** 阅读主区用一张凸起「纸」承载正文（Apple Books 沉浸纸面 + 宽松行距），书架与侧栏用凸起小卡、选中转内凹井 + 主色环（OpenDesign omnidesk-neumorphism 案例配方）；正文媒体与引文由内凹窗承载。

**实施（BUILDER 执行，仅样式与布局）：**
> 1. reading.css 新增健脑室设计变量（暖纸面新拟物配方）：--rd-raise / --rd-raise-sm / --rd-inset / --rd-inset-sm / --rd-ring；作用域限定阅读页根节点，不污染其它页面。
> 2. 阅读纸面：正文容器改凸起大圆角卡（padding 约 34/44），正文段落宋体栈 + 2.0 行距；划线用琥珀软底、想法用主色下划线（沿用现有标注逻辑，只换视觉）；正文图片/媒体改内凹窗 + 圆角；RSS 引文块内凹井 + 主色左缘；章节头进度条改内凹轨 + 主色填充。
> 3. 书架此消彼长：panelOpen=false 时左侧书架 288px（搜索内凹胶囊、条目凸起卡、选中内凹井+主色环、进度内凹轨）；panelOpen=true 时书架收为 56px 图标栏（书/订阅/搜索三钮，可点回列表），过渡约 220ms；窄屏（<1280）沿用现有行为（书架折叠 + 面板 overlay）。
> 4. 右侧面板：展开卡改凸起卡（宽约 320-340 可拖拽不变），tabs 内凹胶囊 + 选中凸起，批注卡与 AI 草稿区改内凹井；收起 40px 图标栏样式统一（圆钮 raise-sm / active inset）。
> 5. 划线浮动条：同一新拟物语言——raised 胶囊容器（白高光 + 暖褐投影、圆角），四个按钮 hover 凸起 / active 内凹，问 AI 按钮主色文字；位置、行为、锚点逻辑不变。
> 6. 验证：cargo test --lib（OMNIDESK_REPO_ROOT 指向主仓库）+ tsc + build + 既有 verify_reading_003/004/stage3 全过；新增 verify_reading_005_design.py 断言新样式类与浮动条样式存在；证据 tests/verification/desktop/evidence/od-read-002/stage4.md。真实窗口视觉由用户终验。
> 7. 明确不做：不动划线锚定与批注数据结构；不动 RSS 抓取；不改首页其它区域；不提交不推送。

### OD-READ-002 阶段 5：健脑室仪表盘落地（2026-11-19，用户批准执行）

> **触发（用户裁决）：** 仪表盘 v2 样稿定稿（/Users/yvan/.codex/visualizations/2026/11/18/01a1ed5a-db65-7780-a2e2-c620b62cb67d/reading-dashboard-v2.html），要求落地：顶部当前书籍封面+进度、本月目标 1000 分钟（无黑色刻度条）、五格数据（阅读字数 K 格式化在前、今年已读在后、批注、订阅文章、整理笔记）、已读书籍列表（封面/作者/日期/完成环）、花瓣图两层结构+外侧标签。

**实施（BUILDER 执行）：**
> 1. 新增 `desktop/src/shell/ReadingDashboard.tsx`：无书打开时的主区仪表盘；数据来自 ReadingPage 已有 library/readingStats/rssSubscriptions/rssArticles；空态（无书无订阅）用两张插图引导卡 + 精选作者 chips。
> 2. 顶部：最近更新且有进度的书籍（封面/书名/格式章数/上次阅读日期）+ 42% 大数字；本月目标固定 1000 分钟，估算分钟 = 本月更新书籍 Σ(chars × progress)/300，标注「估算」，完成后进度条无黑色刻度条。
> 3. 五格数据：字数 K 格式化；今年已读 = 本年读完书籍数（progress_percent≥1 且 updated_at 在当年）；批注/订阅文章（含 X 读完）/整理笔记；无数据来源的涨跌小字不显示。
> 4. 下部：左列已读书籍（最新 4 本，封面=meta.cover 数据 URI，缺省首字占位；作者 meta.author；日期；100% 环）；右列花瓣图（固定底色层 + 数据层，百分比=该源已读篇数占本月已读总数；标签在花瓣外侧）；管理订阅入口。
> 5. 入口：导入书籍复用现有文件框导入；查看全部切到书架；自定义 RSS 地址复用新增订阅弹窗；精选作者 chips 点击即订阅（新增宿主命令 `rss_preset_sources` 返回内置源清单）。
> 6. 样式落在 reading.css（.reading-dash-* 作用域，复用 --rd-* 变量）。
> 7. 验证：cargo lib 全量（OMNIDESK_REPO_ROOT 指向主仓库）、tsc、build、verify_reading_003/004/005 与新增 verify_reading_006_dashboard.py；证据 stage5.md。真实窗口视觉由用户终验。
> 8. 明确不做：不改抓取链路与批注锚定；不虚构历史数据；不提交不推送。

### OD-READ-002 阶段 6：健脑室仪表盘独占、书籍删除、封面与正文图片修复（2026-11-19，用户实测反馈）

> **触发（用户真机截图反馈）：** ①打开健脑室不应出现左侧书籍列表，只有进入书籍/RSS 阅读才出现；②书籍只能添加不能删除，需要删除；③封面与正文图片仍无法显示。
> **实测定位（ARCHITECT）：** 封面：DashboardCover 在 `meta.cover` 为空时直接走首字占位、不调用 `reading_cover_image`；且既有书导入时未存封面（meta.cover 全空），需按需补提取。正文图片：RSS 文章 `<img src=https://…>` 被 `isDirectMediaSrc` 直接交给 `<img>`，Tauri CSP 仅允许 self/data/asset，远程 https 被拦。仪表盘另有三处小问题：0.9991 进度被算作未读完（今年已读 0）、目标条文字在 5% 宽度时被裁、页面不可滚动导致花瓣图截断。

**实施（BUILDER 执行）：**
> 1. 书架显隐：ReadingPage 仅在 `book` 非空时渲染左侧书架（RSS 文章同样是 book）；仪表盘独占整宽。仪表盘「查看全部」改为打开「书架管理」弹窗（列全部本地书籍，可打开/删除）。
> 2. 书籍删除：`reading_store::delete_book_at(root, book_id)` 只删除本地书籍（books/meta/progress/notes 与封面文件；exports 保留），拒绝 rss 条目；新增命令 `reading_delete_book` 并注册；api.ts/types.ts 增加接口；书架行与书架管理弹窗提供删除（ConfirmDialog 二次确认），删除后刷新书架、若正打开该书则关闭阅读器。
> 3. 封面修复：`read_cover_at` 在 meta.cover 缺失时按需从源文件重新提取（复用 task_shell 阅读抽取的 cover 字节，store_cover 写 meta 并更新 meta.cover 持久化）；DashboardCover 与书架封面组件不再因 meta.cover 为空跳过读取，统一走 `reading_cover_image` 并缓存。
> 4. 正文图片修复：新增命令 `reading_remote_media(url)`（reqwest blocking，仅 http/https、≤2 次重定向且每跳校验 scheme、10s 超时、≤8 MiB、响应 content-type 必须 image/*），返回 ReadingMediaPayload；前端 MediaBlock 对 http(s) 源改走该命令并缓存，EPUB odmedia 逻辑不变；图片失败给出可读错误。
> 5. 仪表盘修正：完成判定用 `progress_percent >= 0.995`（与界面 100% 一致）；目标条内部文字在宽度 <12% 时隐藏、剩余分钟移到条下方；仪表盘容器 `overflow-y:auto`，花瓣图随高度收缩不再被截断。
> 6. 验证：cargo lib 全量（OMNIDESK_REPO_ROOT 指向主仓库）、tsc、build、verify_reading_003/004/005/006 全过；更新/新增 verify_reading_007_fixes.py（书架显隐、删除命令、封面回退、远程媒体命令、0.995 判定、目标条条件渲染）。证据 stage6.md。
> 7. 明确不做：不动 RSS 抓取链路；不删除导出文件；不改划线与批注锚定；不提交不推送。

### OD-READ-002 阶段 7：仪表盘细节、真实阅读时长、旧内容媒体回填（2026-11-19，用户实测反馈）

> **触发（用户 7 条反馈）：** ①缺少连续天数/本月分钟数两个按钮条；②下部两栏在 1280 宽时错误堆叠；③已读书籍缺作者列；④本月阅读时间不是真实记录；⑤花瓣中心图标改 RSS 大写 R；⑥目标条左下加「继续阅读」；⑦正文图片仍不显示。
> **实测定位：** 现有 EPUB 书籍与本机 RSS 文章的 books/<id>.json 正文中都没有 `![img]`/`odmedia:` 标记（导入/抓取发生在媒体提取上线之前）；RSS 仅新条目会带标记，旧条目 refresh 时直接 skip。阅读时长当前是「字数×进度/300」估算，未记录真实时间。

**实施（BUILDER 执行）：**
> 1. 真实阅读时长：progress/<id>.json 增加 `daily_seconds`（按 UTC 日期累计）；新增 `reading_time_add(book_id, seconds)` 命令（单次 ≤1 小时，累计）；`reading_stats_at` 增加 `month_minutes`（当月 daily_seconds 合计取整）与 `streak_days`（从今天向前连续有阅读秒数的天数，今天为 0 时从昨天起算）。ReadingStats 前后端类型同步。
> 2. 前端计时：ReadingPage 在书籍打开且窗口可见/聚焦时每 15 秒累计，60 秒 flush 一次并在切书/关闭/隐藏时 flush；不把空闲后台时间计入。
> 3. 仪表盘：标题右侧恢复「连续天数」「本月分钟」两枚胶囊；目标区改用真实 month_minutes（去掉估算标注）；目标条左下加「继续阅读」按钮（打开最近在读书籍，无书则禁用）；花瓣中心改为大写「R」。
> 4. 响应式：下部两栏在 ≥1000px 容器宽时并排、低于则堆叠（修掉 1280 宽时错误堆叠）；作者列始终显示，meta.author 为空显示「未知作者」。
> 5. EPUB 作者：helper 解析 OPF `dc:creator` 写入 author（新导入与回填都生效）。
> 6. 旧内容媒体回填：新增 `reading_refresh_content(book_id)`：仅 epub/mobi，重新调用 read_reading_source 取带媒体标记的章节与作者/封面；按 quote 在章节内重定位批注（找不到则保留原位置并在返回值里报告 unresolved 数）；更新 books/meta（新增 `media_version: u32` 默认 0，新导入置 1，回填后置 1）。ReadingPage openBook 时若 epub/mobi 且 media_version<1 自动调用一次并重载，失败/源文件缺失则置 1 防重复并提示。
> 7. RSS 旧文章回填：ingest_feed_at 对已存在文章，若其批注为空且新解析正文含媒体标记（或条目有 enclosure/media:content 图片而正文无图和标记），更新该文章 chapters/meta（保留 read/progress）；有批注的跳过。parse_entry 增加 enclosure / media:content 图片 URL 提取（仅 http/https、image 类型），正文无 img 时前置一条 `![img](url)`。
> 8. 验证：cargo 全量、tsc、build；verify_reading_003/004/005/006/007 全过；新增 verify_reading_008_dashboard2.py（连续天数/本月分钟胶囊、真实计时命令与 daily_seconds、1000px 断点、作者回退、R 字中心、继续阅读按钮、refresh_content、media_version、RSS enclosure）。证据 stage7.md。
> 9. 明确不做：不改批注锚定结构（回填只做 quote 重定位）；不删用户导出；不提交不推送。

### OD-READ-002 阶段 8：顶部并排自适应 + RSS 旧占位图回填（2026-11-19，用户再次实测）

> **触发：** ①顶部「当前在读书籍」与「本月目标」在 ≤1280px 时被 CSS 堆叠，要求始终并排自适应；②正文仍显示 `[图片：…]` 文本占位（实测为小众软件 2026-11-18 抓取的旧文章：当时图片被转成 alt 占位，正文无媒体标记）。现抓取的同一条目 content:encoded 中有真实 `<img src=https://…>`。
> **实施：**
> 1. reading.css：删除 `≤1280` 对 `.reading-dash-top` 的单列覆盖；所有宽度保持两列（窄屏用 0.9fr/1.1fr 与缩小间距），必要时 hero 数字缩放。
> 2. 新增命令 `rss_refresh_article(book_id)`：读取 RSS 文章 meta 的订阅源，重新抓取该 feed，按 guid/link 匹配条目，重算正文；仅当文章无批注时替换 chapters/author/source_sha256（保留进度与已读）；返回 updated/unresolved 原因。
> 3. ReadingPage openBook：书籍为 rss 且正文含 `[图片：` 旧占位（或含图片指令但无可解析地址）且批注为空时，自动调用 rss_refresh_article 后重载；失败提示原因。
> 4. rss_store：`html_to_paragraphs` 接收 base_url（条目 link），相对 `<img src="/…">`/无协议地址解析为绝对 http(s) 后再转 `![img]`；`sanitize_media_url` 增加 base 解析；保留现有 `[图片：` 文本仅在无法解析时出现（正常应为图片标记）。
> 5. 验证：cargo、tsc、build；003/004/005/006/007/008 全过；新增 verify_reading_009_top_and_placeholder.py（顶部无单列覆盖、refresh_article、base_url 解析、旧占位触发）。证据 stage8.md。
> 6. 不提交不推送；不改批注锚定。

### OD-READ-002 阶段 9：仪表盘内凹分割线 + RSS 文章打开卡顿修复（2026-11-19）

> **触发：** ①仪表盘中部数据区需要上下内凹分割线，区分上中下三段；②点击订阅文章长时间停在「正在打开书籍…」，无法进入阅读界面（根因：openBook 在 setBook 之前同步等待 rss_refresh_article 重新抓取 feed，最长 20s+；且该命令是阻塞式同步命令）。
> **实施：**
> 1. reading.css：`.reading-dash-divider` 改内凹凹槽线（surface 底色 + inset 上下阴影 + 3px 高），上下两条都生效。
> 2. ReadingPage.openBook：先 setBook(loaded) 显示文章，再在后台执行旧占位回填；回填成功更新书籍内容（保持滚动位置），失败 toast；busy 状态只在初次读取期间。
> 3. lib.rs：rss_refresh_article 改为 spawn_blocking 执行，避免阻塞 async 运行时。
> 4. 验证：cargo、tsc、build；003–009 全过；新增 verify_reading_010_open_and_groove.py（先显示后回填、spawn_blocking、凹槽样式）。证据 stage9.md。
> 5. 不提交不推送。

### OD-READ-002 阶段 10：健脑室内页排版、RSS 收藏与 30 分钟自动刷新（2026-11-20，用户验收后新一轮）

> **触发：** ①左侧「健脑室」标题加粗放大、侧栏字体整体 +2；②移除订阅管理与拉取全部按钮，进入页面时若距上次刷新 ≥30 分钟自动抓取；超过 30 天且未收藏的文章不再显示；③书籍增加改名按钮；④订阅源默认全部折叠，每次进入随机展开一个源展示其文章；⑤阅读区顶部标题栏各按钮加粗，「+」改为收藏图标，收藏文章不受 30 天限制。
> **实施：** 1) 侧栏标题 16px/700/letter-spacing，条目/注释/标签字号 +2；2) 移除两按钮；ReadingPage mount/focus 检查最大 last_fetched_at，≥30 分钟静默 rss_fetch 并刷新列表；3) RSS 列表过滤 30 天（published_ts 或 fetched_at，favorite 例外）；4) RssArticleMeta 增加 favorite（serde default），新增 rss_article_set_favorite 命令 + api；阅读头 + 改为星标 toggle（仅 rss；本地书保留导入 +）；星标文章一直显示、行内显示星标；5) 书籍改名：reading_rename_book 命令（book+meta title，非 rss），侧栏/管理弹窗铅笔按钮 + 输入弹窗；6) 侧栏源默认折叠，进入时随机展开一个源（一次）；7) 头栏 笔记/导出/写想法/整理笔记 font-semibold。验证：cargo、tsc、build、003–010 与新增 011 全过；证据 stage10.md。不提交不推送。

### OD-READ-002 阶段 11：正文阅读设置 + RSS 文末模块（2026-11-20，用户本轮明确指令）

> **触发（用户本轮指令）：** ①顶部右上角加「阅读设置」：字体、文字大小、排列方式仅作用于正文，标题不变；不设宽度与颜色；字体增加免费的思源黑体、思源宋体、霞鹜文楷。②RSS 阅读底部增加附件模块、结束标记、预计阅读时间和下一篇文章标题。③文章阅读上下滑动，不需要底部模块或隐藏。
>
> **借鉴要点（≤2 句）：** Pocket/Medium 的正文是安静窄栏、控制集中在顶部或侧边（screenshots-design-inspiration，2026-11-20）；Apple News 用编辑化标题层级和紧凑的字体/对齐选择器。阅读设置弹层沿用本应用暖纸新拟物，不照搬参考产品的配色与宽度控件。

**实施范围（本轮亲自执行）：**
> 1. **字体资源**：新增 `@fontsource-variable/noto-sans-sc`、`@fontsource-variable/noto-serif-sc`、`lxgw-wenkai-webfont` 三个自托管依赖（SIL OFL 1.1），`main.tsx` 引入；OFL 文本复制到 `desktop/public/fonts/`，`THIRD-PARTY-NOTICES.md` 增列。运行时零外链。
> 2. **阅读设置**：ReadingPage 顶栏右侧新增 `Aa` 按钮与弹层；选项为「系统默认 / 思源黑体 / 思源宋体 / 霞鹜文楷」、字号 14–26、排列「左对齐 / 两端对齐 / 右对齐」；设置写入 `localStorage`（`omnidesk.reading.settings.v1`）并提供恢复默认；点击外部或 Esc 关闭。
> 3. **仅正文**：设置以 CSS 变量落在 `.reading-page` 根节点，只被 `.reading-page .reading-paragraph` 消费；章节标题、顶栏、侧栏、附件与文末模块保持原字号与字体。宽度、背景色、颜色不提供设置。
> 4. **RSS 附件数据**：`RssArticleMeta` 新增 `attachments: Vec<RssAttachment>`（serde default）；解析 `enclosure` / `media:content` 中非图片的音频、视频、PDF 等，只收 http(s)、最多 20 条、记录标题、MIME 与字节数；旧文章在下次抓取时补写元数据，正文与批注不动。
> 5. **RSS 文末模块（仅 `source_format === "rss"`）**：附件井（音频/视频复用既有 MediaBlock 与远程媒体链路；文件提供「打开原文件」）、「打开原文」、居中「—— 完 ——」、按正文字数估算的「约 N 分钟读完」（400 字/分钟，至少 1 分钟）、同一订阅源上一篇更早文章的「下一篇」卡片（点击直接打开）。本地书籍不渲染该模块。
> 6. **验证**：`cargo test --offline --lib -- --test-threads=1`、`npx tsc --noEmit`、`npm run build`、既有 verify_reading_003–011 与新增 `verify_reading_012_settings_and_footer.py`；新增 RSS 附件解析与旧元数据补写单测。证据写入 `tests/verification/desktop/evidence/od-read-002/stage11.md`。
> 7. **明确不做**：不提供宽度/颜色/主题设置；不改标题字号；不改批注锚定；不改 RSS 抓取网络策略；不把本地书籍套用文末模块；不提交不推送。
