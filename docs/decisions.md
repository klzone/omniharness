# OmniDesk 架构决策记录

> 这里只登记已确定的决策及仍待裁决的冲突；候选方案、实施状态与审查结论分别记录在 PLANS.md、STATUS.md、REVIEW.md。


## 已确定

### D-060（2026-10-29，OD-ARCH-131 已实施）：Profile 自带 Skill 有自己的源目录，取源顺序 Profile → 仓库 → 内嵌

**状态：** 已实施（`profile_rules.rs` 的 `is_valid_skill_name` / `skill_source_root` / `local_skills` / `allowed_with_local`；`profile_assets.rs` 的 `SkillSource` 与 `expected_skill_files` 取源升级；`lib.rs` 的 `profile_skill_source_open` / `profile_skill_scaffold`；`SettingsPanel.tsx` 绑定区「Profile 自带 Skill」动作条；`profile_lifecycle.rs` 迁移包新成员 `root: "skill"`；窗口验收 24/24 通过；真实模型端到端未测）。

**决策：** 自建 Profile 可以带自己的 Skill，源码放在 `<应用数据根>/profiles/<Profile 名>/skills/<Skill 名>/`。装配、巡检、界面绑定清单与迁移包统一按「Profile 自带 → 仓库 `skills/` → 编译期内嵌」取源，第一处命中即用。

**四条子决策：**

1. **绑定校验从白名单降为形状校验。** `ALLOWED_SKILLS` 只保留「内置清单」的含义，`validate()` 改用 `is_valid_skill_name`（小写字母／数字／`-`／`_`，1–48 字符，直接复用 `settings::is_valid_profile_name`）。用户自己的 Skill 能存进去；`..`、大写、空格这类拼不出目录的名字仍整份拒绝。
2. **一个函数决定取源，三处调用共用。** `expected_skill_files(scope, name, repo_skills)` 返回「文件清单 + 来源标记」，保存绑定收敛、自建 Profile 装配、巡检都走它；`materialize_custom` 里原先重复的挑源分支删掉。名字在拼路径前先过形状校验，三处来源都不接受非法名。
3. **界面只多一行动作条，不新增弹层。** `allowed_skills` 由后端拼成「内置 ∪ 该 Profile 自带」，勾选片因此能选到自己的 Skill。「Profile 自带 Skill」区块提供「打开目录」（首次点才建）与「按名字生成模板」（已存在不覆盖）。
4. **迁移包带上自带 Skill。** 新增成员 `root: "skill"`、路径 `<Skill 名>/<相对路径>`，整包不超过 2MB、最多 256 个成员、每个 Skill 最多 64 个文件、单文件不超过 1MB、正文必须 UTF-8；逐段拒绝空段、`..`、隐藏文件与反斜杠；导入落点是 `profiles/<名字>/skills/`（与导出源同一位置）。旧包没有 skill 成员照常导入。

**为什么不做「Profile 间共享一个 Skill 目录再按绑定筛选」：** 共享目录里同名 Skill 只能有一份，两个 Profile 要同一 Skill 的不同写法时就无解；自带源目录让「这个工作台的定制」跟着 Profile 走，迁移与删除都是整套的。
**为什么不做「联网下载 Skill」：** Skill 决定模型的判断口径，来源必须可控。现在只有两条入口——随程序发布（内嵌）与用户自己放进目录（自带）；不引入市场与签名校验之前不加第三条。
**为什么默认值仍是「仓库优先」：** 内置 8 个 Skill 在开发机上改 `skills/` 要立即生效（D-039）。Profile 自带源只在自建 Profile 里存在，优先级高于仓库只是因为它更具体，不改变出厂行为。

**证据：** `profile_rules.rs:302-360`（四个新函数）、`:244` 起 `validate()` 的校验替换；`profile_assets.rs:690-750`（`SkillSource` + `expected_skill_files`）；`lib.rs:1674` / `:1724`（两个命令）；`profile_lifecycle.rs:1189`（`collect_skill_files`）、`:1440`（`check_skill_bundle_path`）、`:1677` 起导入落点；`cargo test --offline --lib` 495 passed（新增 4 个）、`--tests` 12 组通过、`pytest` 89 passed、前端构建与 debug 构建通过、`preview_profile_skill_source_131.py` 24/24、EXIT=0，证据在 `tests/verification/desktop/evidence/od-arch-131/`。

**复审触发：** ①出现「同一 Skill 名在多个 Profile 里要不同正文」之外的分层需求（如全局覆盖层）；②自带 Skill 需要随包携带二进制附件（当前只收 UTF-8 文本）；③用户要求 Skill 市场或联网安装；④迁移包因 Skill 体积逼近 2MB 整包上限。

### D-059（2026-10-29，OD-ARCH-130 已实施）：绑定保存后立即收敛运行时 `skills/`，规则写盘成功即不回滚，基础标准恒空操作

**状态：** 已实施（`profile_assets.rs` 的 `BoundSkillSync` 与 `sync_bound_skills` 及 2 个单测；`lib.rs` 的 `profile_skill_bindings_save` 接上收敛；`types.ts` 的 `skills` 回执字段；`SettingsPanel.tsx` 的回执文案；窗口验收 25/25 通过；真实模型端到端未测）。

**决策：** 三条一起立，因为它们判的是同一件事——「用户改完绑定多久能用上」。

1. **保存绑定成功即收敛运行时目录。** 设置页保存 `skill_bindings` 之后，当场按新清单把 `<dsh_home>/skills/` 补齐：需要的从停放区归位、缺的按「仓库优先、内嵌兜底」写入、解绑的移入停放区。不要求用户再切一次 Profile 或重启引擎。
2. **规则写盘成功就不回滚。** Skill 拷贝失败（缺源、权限、磁盘）只记进回执，不把 `rules.json` 退回旧值。绑定是用户的决定，补写失败是可解释的次生问题。
3. **基础标准恒为空操作。** 这条路径在 `standard` 判定处直接返回，不建 `skills/`、不建 `profiles/`、不写登记表（沿用 D-057 第一条的干净底座口径）。

**理由：**

1. **运行时的真源是目录，不是规则文件。** 引擎扫的是 `<dsh_home>/skills/`，`rules.json` 只决定「该有哪几个」。写规则不写目录，界面与运行时就有一段不可见的空窗；130 之前这段空窗的代价是「用户以为配好了，下一轮提交却没带上下文」，且很难自查。
2. **保存是一个窄动作，不该顺带重装整套 Profile。** 收敛函数只调用既有原语（`reconcile_managed_skills`、`sync_skill_files`、`record_managed_skills`），不跑 `materialize`、不探测或安装插件、不重启引擎、不写场景与声明文件。整套装配仍归切换与引擎启动，两个入口职责不重叠。
3. **路径必须取运行中引擎的配置。** 切换、启动与保存三条链要落同一个 `dsh_home`；按 Profile 名另行推算根目录会在多根或自定义根的场景下补到别处，且回执显示成功。这条是「同一份配置只解析一次」的具体化。
4. **解绑留痕用停放区，不用删除。** 用户可能手滑取消勾选，停放区让「再勾回来」是无损的；这也避免「解绑即删」把用户手改过的正文一起抹掉。
5. **失败要能被用户看懂。** 回执分成 `filled` / `kept` / `returned` / `parked` / `missing_source` / `failed` 几类，前端拼成一句人话。比起笼统的「操作失败」，用户能据此决定「是换个 Profile 还是补源文件」。

**为什么不做「保存后自动重启引擎」：** 绑定只影响下一轮提交的预注入集合，当前这轮对话不需要中断；重启会打断正在生成的回答，代价远大于收益。真正需要重启的是模型与端点切换，那是另一条链（已在 096 收口）。
**为什么不做「保存时顺带装缺的插件」：** 插件补装已有三条既有通道（切换后、冷启动、设置页手动）并受 `profile_auto_install_plugins` 开关约束；把它塞进保存绑定会让一次点选触发联网安装，用户无法预期。
**为什么不做「在保存时清理所有未绑定 Skill」：** `reconcile_managed_skills` 只处理 OmniDesk 登记过归属的目录（`.omnidesk-managed`）。用户自己放进 `skills/` 的第三方 Skill 不在登记表里，不会被这条路径碰。

**证据：** `desktop/src-tauri/src/profile_assets.rs:418`（`BoundSkillSync`）、`:447`（`sync_bound_skills`）、`:2602` / `:2672`（两个单测）；`desktop/src-tauri/src/lib.rs:2120`（命令）、`:2165`（`save_skill_bindings_into`）；`desktop/src/types.ts:1358`、`:1362`；`desktop/src/SettingsPanel.tsx:1091`；`tests/verification/desktop/evidence/od-arch-130/facts.json`（25/25）；`cargo test --offline --lib` 490 passed。

**复审触发：** ①实测出现「保存后当前这轮就该用上新绑定」（需评估是否在轮次边界重建会话）；②出现「用户自己放进 `skills/` 的 Skill 被这条路径动过」（说明 `.omnidesk-managed` 归属判定被绕过）；③需要在保存时顺带安装或更新 Skill 正文（当前不联网、不覆盖用户改动）；④多窗口或两台机器同时改同一 Profile 时需要合并而非覆盖（当前最后写入者胜）。

### D-058（2026-10-29，OD-ARCH-129 已实施）：场景 × Skill 绑定只由 Profile 规则包表达，界面整表保存、只换绑定不动其它规则

**状态：** 已实施（`profile_rules.rs` 的 `allowed()` / `BindingChoice` / `set_skill_bindings` 与 4 个单测；`lib.rs` 的 `profile_skill_bindings_save` 命令与 `save_skill_bindings_into` 边界、4 个行为测试；`SettingsPanel.tsx` 的勾选区块与 `settings-panel.css` 的行式样式；窗口验收 24/24 通过；真实模型端到端未测）。

**决策：** 三条一起立，因为它们判的是同一件事——「哪个场景预注入哪几个 Skill」在系统里只有一个存放处和一条写入路径。

1. **绑定只存在 Profile 的 `rules.json` 里。** 不新开配置表、不放进 UI 状态或 localStorage。规则包随 Profile 挂载（D-054），所以「切换 Profile = 切换预注入集合」这条等式自动成立，不需要额外的同步机制。
2. **界面整表提交，后端整表替换。** 前端维护草稿，保存时提交「界面上出现过的场景」整张表；后端 `set_skill_bindings` 只替换 `skill_bindings` 并复跑 `validate()`，`global` / `scenes` / `profile` 逐项保留。不做增量命令，也不允许界面绕过校验直写文本。
3. **配不出来的绑定不该靠手写 JSON。** 规则资产页给出一行式勾选表；`allowed_skills` 由后端下发，避免前端另存一份技能名单。基础标准下整块不渲染，后端同样拒绝且不建目录（沿用 D-057 的第一条）。

**理由：**

1. **一个真源比两条同步链可靠。** 若把绑定另存一份（例如 UI 侧），切换 Profile、导入 Profile、手改规则文件这三个动作都会让两份数据分叉，而分叉后没有任何界面能说清「现在到底注入什么」。绑定本来就属于「这个 Profile 的规则」，放在规则包里最自然。
2. **整表替换让「保存」变成一个可回滚的动作。** 每次保存都走 `write_profile_rules` 的备份 + 原子写，旧文件留成 `.bak-<epoch>`；增量更新则会出现「某一次点击写坏一项，但要靠比对才知道」的情况。
3. **校验只在一处。** `validate()` 已限制每场景 `max_injected` 在 1..=3、绑定数不超过上限、技能必须在 `ALLOWED_SKILLS` 内且不重复。界面只负责「点满 3 个就禁用其余片」这种即时反馈，真正的拒绝在宿主——前端门禁失效也不会写出越界文件。
4. **新 Profile 不该被迫先装行业规则。** 规则文件不存在时用 `profile_starter::rules_text` 的起步骨架打底，用户选三个 Skill 就能存下一份「只有绑定、没有行业内容」的规则包。若强行要求先「恢复默认规则包」，等于让每个想自定义的人先接受一套别人的业务规则。
5. **空选择要能和「没配过」区分。** 空数组写进映射而不是删键：装配侧对两者处理一致（都不预注入），但文件里能看出用户动过这一行，将来做「按场景恢复默认」时才有依据。

**为什么不做「绑定放进 Profile 声明 `decl.yml`」：** 声明描述的是场景与展示，规则包描述的是行为约束；绑定决定的是「往上下文里注入什么」，属于行为侧。放进声明会让「只读巡检」和「准入策略」两条链都要解析展示文件。
**为什么不做「界面直接编辑 JSON 就够了」：** 那一页的 JSON 编辑保留（高级用户仍可整包改），但把日常操作留在手写 JSON 上，等于要求用户记住 `weekly-note` / `weekly-report` 这类键名和 `max_injected` 的含义，这与「切过去就是专属工作台」的产品边界相悖。
**为什么不上「每个场景任意数量」：** 预注入是底线，不是清单。注入越多越吃上下文，也越容易和模型自己的判断打架；上限 3 与 108 定的「场景只绑必要的、其余按需加载」一致。真要放宽，应先有实测证据说明 3 不够。

**证据：** `desktop/src-tauri/src/profile_rules.rs:300`（`allowed`）、`:363`（`BindingChoice`）、`:374`（`set_skill_bindings`）；`desktop/src-tauri/src/lib.rs:2120`（命令）、`:2132`（`save_skill_bindings_into`）、`:5000`（注册）；`desktop/src/SettingsPanel.tsx:3174`（区块）、`:1091`（`saveBindings`）；`desktop/src/settings-panel.css:810`（`.binding-block`）；`tests/verification/desktop/evidence/od-arch-129/facts.json`（24/24）；`cargo test --offline --lib` 488 passed。

**复审触发：** ①实测出现「某些场景确实需要绑定 4 个以上 Skill」（需重新论证上限，并考虑按需加载与预注入的分界）；②需要「某场景显式排除某 Skill」或「按周 / 按日程切换绑定」（绑定表需升级结构，`max_injected` 不足以表达排除）；③绑定需要在不用重启的前提下对进行中的会话生效（当前是下一轮提交生效）；④出现「界面保存后规则文件里其它字段被改动」的实测（说明整表替换的边界被破坏）。


### D-057（2026-10-29，OD-ARCH-128 已实施）：干净底座零写入收口，插件登记以安装成功为前提，巡检与装配同源

**状态：** 已实施（`profile_assets.rs` 的 `parse_plugin_spec` / `normalize_plugin_input` / `AddPluginReceipt` / `add_plugin` 与 `inspect_custom` 插件段、4 个新单测；`lib.rs` 的 `save_rules_into` 边界与 `profile_plugins_add` 命令、2 个新单测；`SettingsPanel.tsx` 的声明窄条与基础标准禁用保存；窗口验收 27/27 通过；真实模型端到端未测）。

**决策：** 三条一起立，因为它们判的是同一件事——「这个 Profile 需要什么」在系统里只能有一个可信来源。

1. **基础标准零写入。** `standard` 为真时，规则保存、恢复默认规则包、声明插件三类写入一律在函数第一句拒绝，**不解析、不建目录、不落盘**。基础标准只承载 UI 与 DSH 底座，任何业务规则的存在本身就是「切到定制 Profile」的标志。
2. **插件登记以安装成功为前提。** 声明插件走「先探测 → 缺才装 → 装成功后写登记表」。本地已就绪只补登记、不联网；装失败时登记表逐字节不动。登记表代表「这个工作台需要它」，不代表「它已经装过」。
3. **巡检与装配同源。** 只读巡检的插件清单是「登记表 ∪ 磁盘探测」，与装配时用的判定一致；巡检路径不回写登记表、不安装任何东西。

**理由：**

1. **干净底座是分水岭，不是偏好。** 用户可以接受「基础标准下什么都没有」，不能接受「什么都没装，但已经有规则在生效」。规则是准入策略的输入（`current_rule_policy` 从同一路径读），所以判断必须放在任何写盘机会之前——`write_profile_rules` 的第一句就是 `create_dir_all`，晚一步拒就已经留下一个空 Profile 目录。
2. **界面门禁不能代替后端判定。** 前端 `isCustomAssetProfile` 只是不渲染那一行，命令本身仍可被调用。三处入口（基础标准、出厂 Profile、未登记目录）都在后端拦，前端门禁只是减少无效操作。
3. **失败的安装不该变成依赖。** 若先登记再安装，一旦安装失败（网络、包名不存在、pnpm 构建脚本被拒），界面会长期显示一条永远补不上的依赖，「缺件自动补齐」也会反复重试同一个坏包。反过来先装后登记，最坏情况是「装好了但没登记」，下一次声明即可补上，且探测会说真话。
4. **两个来源是历史包袱，不是设计。** 登记表是装配与补装的真源，磁盘是事实。用户会在命令行装插件，此时两者不一致；只读其中一个，就会出现「设置页说 0 项、切换时却加载了」这种自相矛盾。并集只是把已知事实合起来，不引入第三个真源。
5. **包根归一必须在入口做一次。** 登记表与补丁里会出现子路径与版本写法（OD-ARCH-119），而 `install_plugin` 只认包根。放到入口归一，后续所有比较（去重、界面 key、安装参数）自然一致；放到各处零散处理，迟早有一处漏掉。

**为什么不做「基础标准也能试装插件」：** 那需要在「干净」之外再定义一个「临时污染」状态，用户无法从界面分辨自己现在处于哪一种。真要试，应另立一个不落登记表的会话级通道，并显式标注它是临时的。
**为什么不做「声明即登记、安装交给后台」：** 那会让「已声明但从未装成」的条目无限累积，且用户点了一次按钮却得不到任何失败回执，违反「如实说明」这条口径。

**证据：** `desktop/src-tauri/src/profile_assets.rs:1703`（`parse_plugin_spec`）、`:1726`（`normalize_plugin_input`）、`:1794`（`add_plugin`）、`:1231`（`inspect_custom` 插件并集段）；`desktop/src-tauri/src/lib.rs:2112`（`profile_rules_save`）、`:2120`（`save_rules_into`）、`:1950`（`profile_plugins_add`）、`:4943`（命令注册）；`desktop/src/SettingsPanel.tsx:871` / `:2337`；`tests/verification/desktop/evidence/od-arch-128/facts.json`（27/27）；`cargo test --offline --lib` 480 passed。

**复审触发：** ①用户提出「基础标准下临时试装一个插件」的真实诉求（需另立会话级通道）；②需要在声明插件的同一步里同时改 `package.json` 版本约束（届时登记表要带版本策略，而不只是记录）；③出现「登记表与磁盘探测结果长期不一致且无法解释」的实测（说明有第三处写入点漏管）；④插件数量增长到需要分组 / 排序 / 启用停用时（登记表需要升级 schema）。


### D-056（2026-10-29，OD-ARCH-127 已实施）：Profile 迁移以「包」为单位，校验先于落盘、同名不覆盖、插件不进包

**状态：** 已实施（`profile_lifecycle.rs` 的 `export_bundle` / `verify_bundle` / `import_bundle` / `list_exports` 与 7 个单测；`lib.rs` 的 4 条迁移命令；`SettingsPanel.tsx` 的导出 / 导入 / 迁移包区块；窗口验收 31/31 通过；真实模型端到端未测）。

**决策：** 一套自定义 Profile 能整包带走、能在别处装回来。迁移单位是一个自包含的单文件 JSON 包（`omnidesk.profile-bundle.v1`，后缀 `.omniprofile.json`），包内只含白名单文本配置：DSH 侧 `package.json` / `cordis.yml` / `cordis.patch.yml` / 可选两个 pnpm 文件，应用侧 `decl.yml` / `rules.json` / `categories.json`，以及 `name` / `label` / `created_at` / `origin` / `plugins`。

**理由：**

1. **「包」而不是「目录拷贝」。** `node_modules` 是可执行二进制，跨架构 / 跨平台不可移植；打进去得到的是一份「看起来装好了、实际加载不了」的 Profile，比没有更糟。插件清单随包走，安装仍由 DSH 官方通道按平台执行。
2. **JSON 而不是 tar。** 内容全是 KB 级文本，单文件 JSON 明文可审阅、可手工抢救；离线构建不必新增归档依赖（仓库锁文件里没有可用 crate）。
3. **校验全部前置且只读。** 包大小、schema、成员数、名字合法性、单段文件名白名单、字节数、SHA-256、四类内容解析，任何一项失败磁盘零变化。校验和落盘分成两个函数，避免「边写边发现坏了」。
4. **同名一律拒绝。** 合并两套工作台的规则、场景、Skill 绑定不可逆；同名时给出「换名重试」通道（`rename_to`），不提供覆盖选项。
5. **落盘可回滚。** 临时目录 → `rename` 到位 → 登记表最后写；任一步失败清掉临时与已落盘目录。登记表先写会让软件里出现指向不存在目录的条目，是更糟的失败形态。
6. **`categories.json` 的 `profile` 字段随目标名重写。** 它是分类表的归属键；不改写则导入后切过去被判成别人的表。
7. **插件不进包，导入路径不联网。** 导入只把清单写进登记表，缺件由既有 `required_plugins` / `ensure_plugins` 通道补装。
8. **导出固定落 `<应用数据根>/exports/`，不弹系统保存框。** 结果可复现、少一次对话框，capabilities 因此保持最小集（无 `dialog:allow-save`）。

**为什么不做「导出整个数据根」：** 那会把会话历史、线程、缓存、凭证引用一起带走，扩大数据边界；而且这些不是「工作台配置」，在别的机器上未必可读。
**为什么不做「导入即覆盖同名」：** 覆盖会丢掉当前同名 Profile 的全部资产，且没有撤销路径。125 的「恢复不覆盖」已经确立同一口径。
**为什么不做云端同步或 Profile 市场：** 迁移是用户自己拿文件走，不需要账户体系；云端同步会引入凭证、版本与冲突合并三类新问题。

**证据：** `profile_lifecycle.rs:1008` 起（`BUNDLE_SCHEMA` 至 `import_bundle`，约 500 行）、`list_exports` 在 `:1107`；`lib.rs:1804` / `:1827` / `:1859` / `:1871` 四条命令；`SettingsPanel.tsx` 的 `exportProfile` / `importProfile` / `pickImportFile` 与迁移包区块；`tests/verification/desktop/evidence/od-arch-127/facts.json`（31/31）；`cargo test --offline --lib` 471 passed。

**复审触发：** ①出现跨架构或跨 DSH 版本迁移失败的实测（届时包内需要版本约束字段与兼容矩阵）；②用户提出「把会话历史 / 成果文档一起搬」的诉求（那属于数据备份，不是 Profile 迁移，需另立决策）；③需要「导入时合并同名」的真实场景；④导出目录累积到需要清理入口。


### D-055（2026-10-29，OD-ARCH-126 已实施）：显示名与 Profile 标识分离，改名只动两处显示名

**状态：** 已实施（`profile_lifecycle.rs` 的 `rename_label` 与 `RenameReceipt`、5 个单测；`lib.rs` 的 `profile_rename` 命令；`SettingsPanel.tsx` 的行内改名；窗口验收 26/26 通过；真实模型端到端未测）。

**决策：** 「改名」改的是**显示名**，不是 Profile 的身份。显示名的真身只有两处——`index.v1.json` 里的 `ManagedProfile.label`，与 `profiles/<name>/decl.yml` 里的 `display_name`。改名只动这两处；目录名、`--profile` 标识、`rules.json` 的 `profile` 字段、Skill 归属、插件清单、分类表、会话目录一律不动。

**理由：**

1. **目录名是四类引用的键。** `--profile` 传目录名，`index.v1.json` 的 `name`、`rules.json` 的 `profile`、DSH 侧 `profiles/<name>/`、Skill 归属判断全部以它为键。改名若连带搬目录，四处引用要同步改写，任何一处漏改的表现都是「名字对了、资产全没了」——一个用户无从排查的失败。
2. **两份真身必须同时改，否则界面自相矛盾。** 登记表管顶栏弹层与设置页列名，声明管场景页与资产页标题。只改一处，用户会看到同一个工作台在两个地方叫两个名字。
3. **宁可整体拒绝，不留半套名字。** 声明解析或校验失败、登记表写不回去，都整体失败并还原：声明先写（留 `.bak-<epoch>` 备份），登记表失败时用备份还原声明。最坏情况停在「声明已改、登记未改」，界面显示旧名，磁盘上有一份可还原的备份，可重试、可手工修。
4. **逐行替换而非整份重写。** 只替换顶层 `display_name:` 那一行，保留用户手写的注释、缩进与字段顺序；写出的新文本再走一次解析 + `validate` + 回读比对。因为这是文本替换而不是 YAML 序列化，第二遍校验是正确性的唯一保证。
5. **出厂 Profile 与未登记目录不许改名。** 出厂 label 来自编译期映射（`profile_choice::friendly_label`），写登记表不生效；未登记目录是历史遗留，改了等于给一个软件里看不见的 Profile 起名字。

**为什么不做「改名时同步搬目录」：** 那会让改名从两次小文件写入膨胀成一次牵动四类引用、九类资产目录的迁移，需要事务、回滚与中断恢复。收益只是一个更好看的目录名，而用户看的一直是显示名。
**为什么不做「改 Profile 标识（name）」：** 那是「换身份」，语义上是先建新 Profile、复制资产、再把旧的移入回收站。把它伪装成改名会让用户以为只是换了称呼，实际却换了一套绑定。
**为什么不允许显示名含英文双引号与 `#`：** 声明是受限 YAML 子集，`#` 是行尾注释起点、英文双引号无转义写法，写进去会得到一份解析不回来的声明。含单引号时加双引号。其余常见输入（中文、空格、括号、破折号）都支持，多行粘贴压成单行空格而不是报错。

**证据：** `profile_lifecycle.rs` 的 `rename_label`（约 874 行）、`with_display_name`、`display_name_line`、`clean_display_label`；`lib.rs:1765` 的 `profile_rename`；`SettingsPanel.tsx` 的 `startRename` / `saveRename` / 行内渲染；`tests/verification/desktop/evidence/od-arch-126/facts.json`（26/26）；`cargo test --offline --lib` 464 passed。

**复审触发：** ①用户提出「想把 Profile 目录名也一起改」的真实诉求（届时需要为四类引用设计迁移与回滚）；②出现「同一 Profile 在不同窗口需要不同显示名」的场景（那说明显示名该进会话层而非 Profile 层）；③Profile 导入 / 导出落地时，显示名要能随包搬运且不覆盖本机同名条目；④用户需要在软件里清理 `.bak` 备份文件。


### D-054（2026-10-29，OD-ARCH-125 已实施）：回收站是软件内的恢复通道，不是文件系统潜规则

**状态：** 已实施（`profile_lifecycle.rs` 的 `TRASH_FILE` / `write_trash_meta` / `list_trash` / `restore` 与 `remove` 改造；`lib.rs` 的 `profile_trash_list` / `profile_trash_restore` 与 `omni://profile-registry` 事件；`SettingsPanel.tsx` 的回收站区块；窗口验收 24/24 通过；真实模型端到端未测）。

**决策：** 「移除 Profile」是**可逆**操作，恢复必须是软件内的、一键的、纯本地的，不依赖用户理解文件系统。具体四条：

1. **元数据随目录一起走。** 移除时在被改名的目录里写 `trash.v1.json`（`omnidesk.profile-trash.v1`），记录 `name` / `label` / `created_at` / `origin` / `plugins` / `trashed_at`。目录名只能解析出 name，显示名与来源信息一旦丢了就永久丢了——「恢复」若还原不出原来的样子，就不是恢复。
2. **恢复纯本地、不联网、不覆盖。** 两次 `rename` 加一次登记写回；出厂名、已登记同名、目标目录已存在三种情况一律拒绝。不自动合并两个目录：合并是不可逆的资产混淆，宁可让用户看见失败原因。
3. **元数据不进正式 Profile。** 恢复成功后删除目的目录里的 `trash.v1.json`。曾经被移除过不该成为 Profile 的永久属性。
4. **清单变化必须广播。** `omni://profile-registry` 由 `profile_create` / `profile_remove` / `profile_trash_restore` 发出，顶栏订阅重读。这条是实测发现的真实缺陷：登记与磁盘都对，唯独列表是旧的，用户切不到刚恢复的 Profile。

**为什么不做「永久删除 / 清空回收站」：** 那是不可逆动作，需要与「移除」明确区分的确认流程，且用户本轮没有提出。回收站的存在本就是为「先别删干净」留的余地；在同一轮里给它配一个一键清空入口会把这份余地抵消掉。

**为什么不做「恢复时若同名就覆盖」：** 覆盖会丢掉用户当前同名 Profile 的全部配置、场景、规则与 Skill，且没有撤销路径。同类拒绝（`profile_create` 也拒同名）已经确立了这个口径。

**为什么不自动恢复、要靠用户点：** 移除是用户的明确动作，恢复也应是的。自动恢复会让「移除」变成一个过几秒自己撤销的假动作，用户无法依靠它清理列表。

**证据：** `profile_lifecycle.rs:481` 的 `write_trash_meta`、`:492` 的 `name_from_trash_id`、`:527` 的 `list_trash`、`:605` 的 `restore`、`:713` 的 `remove`；`lib.rs:81` 的 `PROFILE_REGISTRY_EVENT`；`tests/verification/desktop/evidence/od-arch-125/facts.json`（24/24）；`cargo test --offline --lib` 459 passed。

**复审触发：** ①用户提出「清空回收站」或「永久删除」的真实诉求；②出现跨设备/跨机器迁移 Profile 的需求（届时元数据要能独立于目录搬运）；③回收站条目数量增长到需要分页或按时间清理；④出现「恢复时想合并同名 Profile」的真实场景，需重新评估拒绝口径。


### D-053（2026-10-29，OD-ARCH-124 已实施）：插件自愈有三个触发点，装配永不联网

**状态：** 已实施（`profile_assets.rs` 的 `missing_plugins`；`lib.rs` 的 `maybe_autofill_plugins_on_startup`、`append_profile_assets_log`、`.setup()` 接入点；`TopNav.tsx` 的空状态兜底；`SettingsPanel.tsx` 的开关文案；窗口验收 14/14 通过；真实模型端到端未测）。

**决策：** 「补齐 Profile 需要的插件」这件事有三个触发点——用户手动点「安装」、切换 Profile 之后、应用冷启动之后。三者共用同一张清单（`required_plugins`）、同一个判据（`should_auto_install_plugins`）、同一个安装通道（`dsh plugin --profile <p> add`）、同一个总开关（`profile_auto_install_plugins`，默认开）。而**装配**（写配置清单、cordis 补丁、场景声明、规则、Skill）永远不联网，联网只发生在明确的那一次插件安装里。

**四条子决策：**

1. **启动路径只探测，不安装。** `missing_plugins` 是纯读函数（多候选目录查 `package.json`，不联网、不写盘、不建目录）；确认缺件后才起后台线程。理由是启动必须快且可离线：探测是毫秒级，安装是几十秒到几分钟。窗口实测首启到插件就位期间界面可交互。
2. **基础标准恒不装。** `required_plugins` 与 `ensure_plugins` 对 `profile == standard` 都硬编码返回空，不依赖界面文案；`maybe_autofill_plugins_on_startup` 在探测之前就短路。这样「OmniDesk 是干净底座」这条边界只靠代码即成立，不必靠用户记得关开关。
3. **一个总开关统管切换与启动。** 不给冷启动另造开关。用户对「要不要自动联网装东西」只应做一次决定；拆成两个开关会出现「切换时装、启动时不装」这种没人能解释的状态。
4. **装失败必须让用户看见。** 后台自愈的默认表现是无感（装齐只给一句回执），但缺件卡必须在没有「切换回执」的情况下也能从空状态建出来——冷启动时用户根本没打开过 Profile 弹层。这是 124 唯一的前端改动，也是最容易漏掉的一环：后端做对了、用户仍不知道发生了什么。

**为什么不做「启动时同步装完再开窗」：** 那会把「双击图标到能用」的时间里塞进一次网络安装（实测 121 的真装耗时 47–115 秒）。用户对启动慢的容忍度远低于对后台补齐的容忍度，而且网络不可用时会变成「软件打不开」。

**为什么不做「缺件就自动降级到基础标准」：** 缺一个插件不等于整个 Profile 不可用——场景声明、规则、Skill 仍然是齐的，多数能力照常工作。静默降级会让用户以为自己的 Profile 不见了，比缺一个插件更糟。正确做法是照常进入该 Profile，把缺件摆到台面上并后台补。

**证据：** `profile_assets.rs` 的 `missing_plugins` / `required_plugins` / `ensure_plugins`；`lib.rs:1441` 的 `maybe_autofill_plugins_on_startup`、`lib.rs:1494` 的 `append_profile_assets_log`、`lib.rs:4795` 的 `.setup()` 接入；`settings.rs:217` 的 `profile_auto_install_plugins`（默认 true，settings.rs:204）；`tests/verification/desktop/evidence/od-arch-124/facts.json`（14/14，含首启、删包重启、开关关闭、基础标准四条路径）；回归 121（20/20，两次真装 47.2 / 69.2 秒）与 123（31/31）。

**复审触发：** ①出现「用户希望启动就阻塞装完」的真实诉求；②DSH 官方插件通道新增批量 / 离线安装能力，可以显著缩短耗时；③出现需要按场景而非按 Profile 判断插件是否必需的场景；④开关被拆成两个的诉求出现时，先回头读本条的「一个总开关」理由。

### D-052（2026-10-29，OD-ARCH-123 收尾自查结论）：数据目录与会话历史是「机器级」，不随 Profile 走

**状态：** 现状确认（未改代码）。核查点：`settings.rs` 的 `artifact_root` / `library_root` / `knowledge_vault_root` 三个字段各只有一个全局值；`thread_store` 的根是 `<应用数据根>/threads`，同样不带 Profile 名；引擎侧 DSH 会话目录是 `<dsh_home>/sessions/<按 cwd 分目录>/`，按工作目录而不是按 `--profile` 分堆。也就是说：切 Profile 会换掉场景声明、规则、Skill、插件、分类表，但**不会换掉成果根 / 库根 / 知识库根 / 历史会话。

**决策：** 这几项**有意不跟随 Profile**，保持机器级单值。

**理由：**

1. **数据是用户的，不是 Profile 的。** 用户换 Profile 是想换「用什么规则干活」，不是想把上周的周报和知识库一起搬走。按 Profile 分目录会让同一批真实资料在切换后「消失」，而磁盘上并没有任何东西被删除——这是最难排查的一类假故障。
2. **切换必须是可逆且无损的。** 109/121/122 定的是「切过去就装配好」，装配的是可再生的资产（声明、规则、Skill、插件）。数据和会话不可再生，把它们卷进切换路径，就得为「切回基础标准后成果去哪了」设计迁移、回滚与提示，代价远大于收益。
3. **知识库在 P1 就是只读外部目录**（D-031）。让它随 Profile 变，等于让「同一个 Profile 在两次切换后指向不同的库」，而界面上的搜索与引用会静默换源。
4. **会话历史按 cwd 归档是 DSH 的口径**，OmniDesk 不改 runtime。要让历史按 Profile 隔离，只能在宿主侧过滤展示——那会造出「引擎里还在、列表里看不见」的分裂。

**代价与已知限制：** 同一台机器上的多个 Profile 共享成果根与历史会话列表；一个「工作」Profile 和一个「生活」Profile 会看到同一批会话。想彻底隔开，正确做法是给不同用途配置不同的成果根与库根（设置页已可改，且切换 Profile 不会覆盖这两项设置），而不是让平台按 Profile 自动改写路径。

**为什么不做「切换时自动换根」：** 那需要在每个 Profile 里再存一份路径配置，并为「目标路径不存在 / 不可写 / 落在禁区」设计整套回退。切换本身的语义会从「换规则」膨胀成「换数据视图」，与用户「Profile 是定制、OmniDesk 是干净底座」的分工不符——干净底座恰恰意味着数据位置由用户在设置页显式决定，平台不替他猜。

**证据：** `settings.rs:45-58`（三个 `Option<String>` 单值，`set_artifact_root` / `set_library_root` / `set_knowledge_vault_root` 都是整文件改写同一份 `settings.v1.json`）；`app_paths.rs:102`（`threads: root.join("threads")`）；`engine_supervisor.rs:305`、`:324`（引擎只把 `profile` 传给 DSH，不传数据根）；实测 `~/OmniDesk/runtime/dsh-home/sessions/` 下按 cwd 分 7 个目录，无 Profile 维度。

**复审触发：** ①用户明确要求「不同 Profile 用不同成果根」；②出现「同一台机器多使用者、需要数据级隔离」的真实场景；③DSH 官方把 sessions 改成按 Profile 隔离，届时宿主侧要与之一致；④知识库升到 P2 支持多根时，重新评估根与 Profile 的关系。


### D-051（2026-10-29，OD-ARCH-123 已实施）：声明热重载只开在「应用内保存」，基础标准不落声明

**状态：** 已实施（`lib.rs` 的 `PROFILE_DECL_EVENT` / `profile_decl_save` / `save_decl_into`、`App.tsx` 的事件订阅、`SettingsPanel.tsx` 的 `declStandard` 提示与禁用与「切换 Profile 即重读声明」）；真实窗口验收 31/31 通过；真实模型端到端未测。

**决策：** 场景声明决定 Dock 上挂什么入口。它有两种变更来源，处理方式必须不同：**用户在设置页点「校验并保存」**属于应用内的、经过解析校验与备份的变更，应当在保存成功后立即生效；**用户在应用外部手改或删除文件**属于应用看不见的变更，只能提示、不能热重载。同时，基础标准（`omnidesk-base`）作为干净底座，不允许存在声明文件，保存请求在写盘之前就被拒绝。

**三条子决策：**

1. **热重载只走应用内保存这一条路。** 应用内保存成功后 `emit` `omni://profile-decl`，界面收到即清掉漂移提示并重读声明。外部手改仍由 5 秒指纹轮询报 `modified` / `missing`，不重载。理由：谁改的决定界面该不该跟着变——用户刚点完保存，界面还按旧入口显示，等于告诉用户刚保存的东西没生效；而外部手改若也热重载，「磁盘上是什么」与「界面是什么」会悄悄脱钩，用户在排查问题时失去唯一的判断依据（052 定的可复核边界）。
2. **基础标准的拒绝必须发生在建立目录之前。** `save_decl_into` 的 `standard` 短路是第一句，早于 `create_dir_all`。校验失败同口径：一字节都不写。这样「基础标准是不是干净底座」可以只靠「`profiles/omnidesk-base/` 在不在」复核，而不必读文件内容。
3. **前后端用同一个标准判据。** 后端走 `profile_asset_scope`（运行中引擎实例的 Profile），前端 `declStandard` 用 `choice.standard === choice.effective` 并在 `profileEpoch` 变化时重置。禁用按钮与拒绝保存必须是同一件事，否则迟早分叉成「界面上不能点、接口上能写」或反过来。

**为什么不在基础标准下写一份空声明：** 空文件也会让 `profiles/omnidesk-base/` 这个目录存在，之后「底座是否被污染」就不再是个可以一眼看清的问题。干净底座的价值在于它可被验证，写一份「无内容」的占位文件等于亲手毁掉这个验证点。

**为什么不做「外部改动自动热重载」：** 编辑器保存、脚本改写、同步盘回滚都可能触发，界面会在用户没操作的情况下换掉入口；一旦解析失败还会掉回内置入口，用户很难分辨「我改坏了」与「应用读错了」。提示条 + 显式「重新读取」能把决定权留在用户手里。

**证据：** `cargo test --offline --lib` 451 passed（新增 3）；`--tests` 12 个 test binary 全 ok；`pytest tests -q` 89 passed / 30 subtests；`npx tsc --noEmit`、`npm run build`、release 构建通过；真实窗口 `preview_profile_decl_hot_123.py` 31/31（`evidence/od-arch-123/`），回归 `preview_profile_starter_122.py` 38/38、`preview_profile_custom_118.py` 26/26、`verify_decl_restart_hint.py` 6/6。

**复审触发：** ①要做多窗口同步（届时 `emit` 的窗口范围与去重需要重新设计）；②用户明确要求「外部改文件也自动生效」，需同时给出解析失败时的回退与提示方案；③声明开始承载入口以外的内容（例如 Skill 绑定），热重载的影响面不再是纯界面；④真实模型端到端暴露「保存后入口与引擎状态不一致」。

**附：切换 Profile 后设置页必须当场重读声明（同轮审查补的第四条）。** 切换 Profile 时设置页清空各分页缓存，分类 / 能力 / 资产 / 规则都靠自身分页「打开时才读」的 effect 兜底；场景声明没有分页级兜底，它随「对话分类」页一起渲染。只清缓存会让用户停在该页切换 Profile 后一直看到空编辑框，而磁盘上 `decl.yml` 刚由装配写好。因此「清缓存」与「重读声明」绑定成同一个动作：哪一天有人想把这处 `refreshDecl()` 拿掉，等于让界面重新开始说假话。


### D-050（2026-10-29，OD-ARCH-122 已实施）：自建 Profile 也要「体检看得见 Skill、起手自带骨架」

**状态：** 已实施（`profile_assets.rs` 的 `inspect_custom` Skill 巡检、新模块 `profile_starter.rs`、`profile_lifecycle.rs` 的起手分支、`lib.rs` 的 `profile_decl_starter`、`SettingsPanel.tsx` 的「填入起步模板」）；真实窗口验收 38/38 通过；真实模型端到端未测。

**决策：** 产品口径是「OmniDesk 本体干净，Profile 承载定制」。出厂 Profile 由清单装配，自建 Profile 由 `decl.yml` + `rules.json` 驱动。这两份文件既然决定了「切过去是什么工作台」，就必须满足两条：**用户能看见它声明的东西到底在不在了**，**新起一个 Profile 不该是空白**。

**四条子决策：**

1. **巡检与装配共用同一张 Skill 清单。** `inspect_custom` 走 `wanted_skills`，与 `materialize_custom` 完全同源，逐项复用 `expected_skill_files` + `inspect_skill`。此前巡检只看插件，`rules.json` 声明了 Skill 也看不见——用户删掉目录后，只有下一次切换才会被悄悄补回，中间这段时间界面在说谎。
2. **巡检永远只读。** 不建目录、不写 `.assets.v1.json`、不补任何文件。「打开设置页」不能改变磁盘状态，这是 109 定下的硬边界，本轮新增的 Skill 段同样遵守，并用单测 + 窗口实测双向验证。
3. **起手骨架是最小可改的一份，不是一套默认产品。** `decl.yml` 只给五个标准入口（工作台首页 / 工作记录 / 文案写作 / 项目跟踪 / 历史会话），`rules.json` 三块全空——没有行业规则、没有红线词、没有 Skill 绑定。零预注入是刻意的：预置行业内容会把 OmniDesk 变成某一行的专用工具，也违背用户「省 token、快回复」的要求。用户要什么自己在「规则资产」页加，或从别的 Profile 复制。
4. **只在两条入口产出，永不覆盖。** `create(copy_from = None)` 与用户手动点「填入起步模板」。已有文件一律跳过，已运行的自建 Profile 不会被自动塞一份声明——覆盖用户「故意不要场景」的选择比空白更糟。写前先用真实解析器自校验，写不出来就报错，不写半成品。

**为什么不在切换时补写声明：** 切换是「按清单装配」，不是「猜用户想要什么工作台」。自建 Profile 的声明属于用户资产，只有创建那一刻与用户主动点击时才该产生。这条与 109 的「切换只装配、不改内容」一致。

**证据：** `cargo test --offline --lib` 448 passed（新增 5）；`--tests` 12 个 test binary 全 ok；`pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 41/41；`npx tsc --noEmit`、`npm run build`、release 构建通过；真实窗口 `preview_profile_starter_122.py` 38/38（`evidence/od-arch-122/`）。

**复审触发：** ①需要「旧自建 Profile 一键补骨架」的能力；②需要行业模板库或 Profile 市场，届时评估模板来源与校验；③多个 Profile 共用同一份 Skill 目录导致版本冲突；④真实模型端到端暴露「零预注入的默认体验不够用」。


### D-049（2026-10-29，OD-ARCH-121 已实施）：切 Profile 时后台自动补齐插件，安装命令必须有上界且能整组回收

**状态：** 已实施（`profile_assets.rs` 的超时执行器与 `install_plugin_with_timeout`、`settings.rs` 的 `profile_auto_install_plugins`、`lib.rs` 的 `should_auto_install_plugins` / `spawn_profile_plugins_autofill` / `profile_assembly_prefs(_set)`、`TopNav.tsx` / `SettingsPanel.tsx` 的事件订阅与开关）；真实窗口两路验收通过；真实模型端到端未测。

**决策：** OmniDesk 本体保持干净初始环境，Profile 携带自己的配置、场景、规则、Skill 与插件清单。切换 Profile 时按清单自动装配；插件若缺失，**切换先本地生效并立即返回**，缺件交给后台任务自动联网补装，结果用事件推回界面。默认开启，可关；基础标准恒不安装任何插件。

**四条子决策：**

1. **安装不能占据切换的同步路径。** 联网安装的耗时不可控（本次实测一次安装约 1–2 分钟，含 pnpm 解析与下载）。切换的语义是「本地配置已经换好」，把等待网络塞进去会让用户以为软件卡死。因此 `spawn_blocking` + 事件回传：命令返回时只保证本地落盘与旧引擎停止，插件结果稍后单独到达。
2. **安装命令必须有上界，且要收整组进程。** `INSTALL_TIMEOUT` 180 秒，`try_wait` 轮询 + deadline。**只 `kill` 子进程不够**：单测实测 `sh` 被杀后孙进程仍握着 stdout/stderr，收尾被拖到 30 秒。Unix 下用 `process_group(0)` 让命令自成进程组，超时 `killpg(SIGKILL)`；管道排空给 2 秒宽限，避免脱组进程把读线程钉死。非 Unix 退回 `child.kill()` 并如实标注。
3. **超时与失败必须如实回传，不冒充成功。** `InstallRun.timed_out` 独立于退出码；超时不走「pnpm 构建脚本放行后重试」分支（那条只对真实 `Ignored build scripts` 生效）。前端收到仍缺的结果就把缺件卡刷成最新清单并保留手动安装入口，绝不在失败时收起提示。
4. **默认自动，但要能关，且基础标准恒不装。** `settings.v1.json` 的 `profile_auto_install_plugins` 缺省为 `true`（旧文件不需要迁移）。关掉后行为与 114 一致：只提示缺哪些，由用户点「安装」。基础标准有两道短路——`required_plugins` 返回空表、`should_auto_install_plugins` 再判目标不等于标准——产品承诺不能只靠单点判断。

**为什么不在切换时弹模态等安装：** 安装可能失败、可能要几十秒到几分钟，模态会把「换个工作场景」变成一次阻塞交易。后台 + 事件 + 可关闭的提示，既让用户知道在做什么，也不挡他立刻开始下一条任务。这也与 OD-ARCH-097 定下的右下角状态条体例一致。

**回执位置（窗口实测修正）：** 回执原来绝对定位在徽标下方，会与弹层正文压字、右端被窗口边缘裁掉。现在弹层开着时回执显示在弹层顶部，弹层关着时才挂在徽标下方，并带 `aria-label` 供读屏读取。

**证据：** `cargo test --offline --lib` 443 passed（新增 8）；`--tests` 集成 2/1/13/6/5/10/21/4/3/1 全绿；`OMNIDESK_NET_TESTS=1` 联网端到端 1 passed（8.17s，装齐后重跑不再装）；`pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 41/41；真实窗口 `preview_profile_autofill_121.py` 20/20（`evidence/od-arch-121/`，含开关开/关两路）；`npx tsc --noEmit`、`npm run build`、release 构建均通过。

**复审触发：** ①需要「事后发现插件被删就自动补」（本轮只在切换那一刻触发）；②安装耗时超过 180 秒成为常态，需要改成可配置或分阶段提示；③需要离线安装包，届时评估本地包来源与校验；④多 Profile 并行安装导致 registry 限流。


### D-012：DSH 会话恢复与 OmniDesk 任务恢复是不同能力
- 状态：已确定的设计边界；具体锁定版本能力须逐版本验证。
- 决策：分别描述历史可读、恢复既有 DSH session、确认中断任务状态、从断点继续任务；不能将 ACP session resume 等同于任务续跑。没有可靠终态证据时不得自动重放或伪报成功。
- 理由：现有 OmniDesk 适配器代码将非终态任务恢复为 unknown，明确不自动重放；ACP 恢复会话不能证明中断轮次结果。
- 影响：会话持久化设计须注明恢复保证、丢失窗口、事件完整性和版本兼容边界。
- 来源：engine/omnidesk_engine/acp_adapter.py、docs/OmniDesk-Engine-Interface-Test-Matrix.md。

### D-000：个人 MVP 先经独立审查，再进入试用
- 状态：OD-BUILD-002 在其范围内复验通过；完整 Tauri 工作路径待复核。
- 决策：作者自用不以 20% 节时为前置条件，但 MVP 必须通过关键代码边界审查及必要运行路径复核后，才进入严格受限的个人试用。OD-ARCH-001 的 R1–R3 与 OD-BUILD-002 补充的 R4 已有范围性通过结论；真实 Tauri 纵向路径、真实模型解释质量及授权适用范围仍分别受限或未验证。
- 影响：真实 Tauri 任务路径复核及适用授权边界明确前，不将现有 MVP 描述为可用于真实工作材料的日常工具。
- 来源：REVIEW.md、GOALS.md。

### D-001：OmniDesk 是独立个人工作台
- 状态：已确定。
- 决策：OmniDesk 有自己的任务体验、领域规则、数据组织与成果管理；不是 DSH 套壳、换皮或复制品。
- 理由：产品价值在于把专业工作组织成可靠、可复核的流程。
- 影响：DSH 的 UI、私有数据结构和运行时内部实现不得成为 OmniDesk 的产品界面或领域契约。
- 来源：GOALS.md 与归档的历史产品路线材料。

### D-002：DSH 是可替换的执行引擎依赖，Runtime 保持不变
- 状态：已确定。
- 决策：早期可以通过 OmniDesk 自有适配边界复用 DSH；不修改 runtime/ 内 DSH Runtime，不长期维护 Fork。
- 影响：任务、成果与领域规则应可独立于 DSH 保存和迁移。
- 来源：AGENTS.md、GOALS.md、docs/OmniDesk-Desktop-Architecture.md。

### D-003：当前复用最小任务外壳，单一试点验证
- 状态：已确定为当前实现方向；关键代码边界已有分项通过结论，完整 Tauri 使用路径待复核后再判断是否进入受限个人试用。
- 决策：共用任务生命周期；任务差异由各自模板/规则表达。先验证“脱敏数据分析与生态研判”；不建设通用 Agent 平台、工作流编辑器、Registry 或插件市场。
- 影响：周报、材料写作、项目管理目前不能因共用外壳而视为已实现或已获准处理真实数据。
- 来源：docs/OmniDesk-Task-Shell.md 与 tests/reports/personal-mvp-vertical-slice-report.md；实现须经 REVIEW.md 独立审查。

### D-004：安全与用途授权从严解释
- 状态：原则已确定；具体许可范围按用户提供的答复摘要记录。
- 决策：不得扩大推定许可。当前答复仅覆盖满足其脱敏条件、用于数据分析与生态研判的个人分析使用，并明确不得分发；其他用途、端点、工具、设备或环境按证据逐项判断。
- 影响：脱敏、提示词、启发式扫描不等于授权或代码强制控制。
- 来源：用户在本任务中的答复及 docs/OmniDesk-Authorization-and-DataFlow-Gap-Audit.md。仓库未声称持有授权原件。

### D-005：个人使用先于产品化量化门槛
- 状态：已确定。
- 决策：20% 净节省率不是作者开始个人使用的前提；使用中低负担收集体验和返工信息。扩大试用或产品化另作决策。
- 来源：GOALS.md 与用户确定的个人先试用原则。

### D-006：工作区差异必须保全
- 状态：持续有效。
- 决策：任何任务先记录 Git 状态；不重置、清理、覆盖或擅自提交用户/BUILDER 的已有改动。
- 来源：用户明确工作规则与 AGENTS.md。

### D-007：任务外壳可复用，专业能力分别界定
- 状态：已确定为设计原则，后续能力仍需分别实现和验收。
- 决策：周报、材料撰写、日常项目管理可共享基本任务生命周期、状态呈现与输入输出组织；领域规则、权限、模板、评测和授权须独立定义。当前只实现脱敏数据分析与生态研判试点。
- 影响：共用骨架不代表其他任务类型可执行，也不扩大用户转述的许可范围。
- 来源：GOALS.md、用户确认的产品范围。

### D-008：Skill 与 MCP 分工，适配层保持最小
- 状态：已确定。
- 决策：Skill 定义专业流程、规则、输入输出和质量检查；MCP 只承接确有需要且已获准的外部工具接入；Engine Adapter 负责最小任务生命周期与事件转换，不扩展为通用通信平台。
- 影响：MCP 细节不得扩散进领域工作流；不因协议演进预建通用层。
- 来源：用户确认的 OmniDesk 架构原则；GOALS.md。

### D-009：文件优先，数据库由实际瓶颈触发评估
- 状态：已确定。
- 决策：当前以 Markdown、JSON/YAML 与 Git 保存可版本化资产；仅在并发写入冲突、状态查询瓶颈或文件检索评测优化后仍不达标时评估 SQLite。向量库、图数据库不提前引入。
- 影响：达到触发条件只启动方案评估，不自动批准迁移或采用数据库。
- 来源：用户确认的 OmniDesk 架构原则；GOALS.md。

### D-010：产品化由收益、质量、意愿和痛点联合触发
- 状态：已确定为进入 Tauri 评估的门槛；当前尚未满足或验证。
- 决策：净节省工时超过 20%、质量与合规指标达标、至少一位同事明确愿意实际使用、且存在桌面应用可解决的真实痛点，四项需同时满足后才进入产品化评估；满足后仍需另行批准。
- 影响：个人 MVP 可先在审查通过和许可边界内自用，不以 20% 为前置条件；产品化门槛不扩大数据授权。
- 来源：用户确认的产品路线与 GOALS.md。

### D-013：会话存储按原始 cwd 分组，工作区/ACP 按 realpath；OmniDesk 必须规范化 cwd
- 状态：已确定（OD-ARCH-003 锁定版本源码证实）。
- 决策：DSH 会话**存储**按**原始 cwd 字符串**分组（`session-persistence-jsonl/format.ts` 的 `projectKey`），而**工作区注册表**与 **ACP `session/list`/`session/resume`** 按 `fs.realpath` 规范化后的路径分组。同一物理目录的别名写法与真实路径写法会产生两个存储项目目录，但共享同一工作区身份。OmniDesk 在向引擎提交或恢复任务前**必须对 cwd 做 realpath 规范化**，使存储分组与工作区/ACP 身份一致。
- 理由：这是“磁盘上有会话、界面里看不到/找不到”的已证实根因之一；修复只需规范化路径，**无需复制对话正文**。
- 影响：引擎适配层需要单一 cwd 规范化落点；不可解析路径按现有契约明确报错，不静默退化为字符串相等。跨写法 resume 本身由 DSH 的 realpath canon 允许，但分组仍会分裂。
- 来源：OD-ARCH-003（REVIEW.md 顶部同名章节），`packages/session/session-persistence-jsonl/src/format.ts`、`packages/workspace/workspace/src/{index,paths}.ts`、`packages/acp/acp/src/index.ts`。

### D-014：原生会话格式为已发布格式，DSH 内部状态不可当作可丢弃
- 状态：已确定（锁定版本文件事实）。
- 决策：锁定版本的会话格式 V4 已是**已发布**格式（`latestReleasedVersion: 4`、`evidenceTag: dsh-v0.2.0-rc.2`）。该版本文档明确写明 alpha/beta/rc 产品发布即建立已发布格式义务，GitHub 的 prerelease 标记并不会让持久化用户数据变成可丢弃。因此 v0.4 中“DSH 内部状态可丢弃”的表述不成立，隔离运行根中的会话属于不可随意丢弃的数据。
- 理由：已发布格式承担兼容义务；把原生状态当一次性缓存会低估迁移与丢失风险。
- 影响：任何迁移、覆盖、删除隔离运行根的会话都须单独评估并获用户明确授权；不得据“内部状态可丢弃”简化备份与保留策略。
- 来源：OD-ARCH-003；`runtime/deepseek-harness/docs/session-format-status.md`。

### D-015：恢复语义四分解，禁止无证据自动重放
- 状态：已确定（OD-ARCH-003 隔离脚本运行证实）。
- 决策：严格区分“历史可读”“恢复既有 DSH session”“确认中断任务状态”“从断点续跑”四种能力。原生 resume **只收束**中断轮次（补写 `step/end`、`turn/end`、`session/end-seed`），**不续跑**未完成轮次；中断轮次不留下 assistant 文本。没有可靠终态证据时不得自动重放或伪报成功。
- 理由：隔离一次性合成验证显示收束记录在 resume 时刻生成，而非中断时刻；`recover_task` 对非终态标 `unknown` 且不重放。
- 影响：会话持久化设计须注明恢复保证、丢失窗口、事件完整性与版本兼容边界。这是对 D-012 的锁定版本证据补强。
- 来源：OD-ARCH-003（`/tmp/arch003/out/restart-recovery.json` 等一次性运行输出）；`engine/omnidesk_engine/acp_adapter.py`。

### D-016：Rust 桌面宿主默认使用 ACP Profile
- 状态：已实施于当前未提交工作区；ARCHITECT 独立复核通过（计划范围，见 REVIEW.md）。宿主默认传递已由 `EngineConfig::from_env()` 直接单测证明。
- 决策：Rust EngineSupervisor 子进程默认 Profile 为 `acp`；显式 `OMNIDESK_DSH_PROFILE` 仍可覆盖。Web 开发入口使用的 `yvan-work` 是另一条路径，不作为 ACP 子进程默认值。
- 理由：锁定的 `yvan-work` 是 Web bundle，不能响应宿主预期的 ACP stdio 握手。
- 影响：需保持默认值测试经过 `EngineConfig::from_env()`、`child_env()` 和 `initialize_params()`；不能只凭引擎自身默认值或手动注入环境变量验收。
- 来源：OD-BUILD-003 实现与 REVIEW.md 独立审查；补齐计划见 PLANS.md OD-BUILD-004。

### D-017：端点准入暂无宿主可核实事实，保持默认拒绝
- 状态：已确定（OD-ARCH-009 源码审计结论）；实现方案待批准。
- 决策：在存在宿主可核验的端点、服务商、设备、运行环境与用途书面证据之前，设计与验收要求是默认拒绝；仅隔离 `DSH_HOME` 不构成端点隔离，脱敏与提示词也不构成准入。**该拒绝要求尚未在主工作区的所有入口核实实现**（OD-ARCH-009 独立复核）；因此不得把它当作已落实的代码保护。
- 理由：`EngineChannel::spawn` 与 `serve.py` 均继承完整宿主环境，`DEEPSEEK_BASE_URL`、凭证与代理变量可改变数据去向；`OMNIDESK_DSH_PATCHES` 等普通环境变量可注入 provider/baseURL；DSH 默认挂载 web、bash、fs 与遥测通道。
- 影响：未来若实现，须在宿主构成白名单子进程环境，校验实际生效端点与 patch 内容，合成模式限定回环且不得同时持有真实凭证；无有效证据即拒绝启动。评估方案不等于批准实现。
- 来源：OD-ARCH-009（STATUS.md、REVIEW.md），`desktop/src-tauri/src/engine_ipc.rs`、`engine_supervisor.rs`、`engine/omnidesk_engine/serve.py`、`runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-base/cordis.patch.yml`。

### D-018：新成果默认落 OmniDesk 数据区，旧 YvanAI 成果保持原样- 状态：已实施于当前未提交工作区；主体代码审查及 OD-ARCH-012 失败清理补充验证均已通过（见 REVIEW.md）。
- 决策：桌面新生成的离线成果与分析成果默认写入 `~/OmniDesk/workspace/artifacts`；`OMNIDESK_LOCAL_DATA_ROOT` 仍可显式覆盖，供隔离测试使用。旧 `~/YvanAI/projects/Omnidesk/analysis` 仅作为历史位置，其中的既有文件**不读取正文、不迁移、不改名、不覆盖**。
- 理由：此前默认值指向旧 YvanAI 路径，会让新成果与旧试点数据混在一处，既难说明归属，也让「迁移」与「继续使用」产生歧义。改为明确的 OmniDesk 位置，同时不动既有文件，是最小且可回退的边界。
- 影响：**不构成资产统一已完成。** 旧目录仍可能持有真实成果，是否迁移须另行评估并经用户明确授权。两份保存路径共用同一个写入函数：`create_new` 防覆盖，写入/刷盘失败时删除本次半成品。
- 来源：OD-ARCH-011；`desktop/src-tauri/src/task_shell.rs`（`DEFAULT_LOCAL_SUBDIR`、`resolve_local_data_root`、`write_new_file_durable`）及对应单元测试。

### D-019：对话为中枢；三类场景由模型起草，代码只做确定性与边界
- 状态：产品方向已确定（OD-ARCH-026，用户已批准执行）；实现位于当前未提交工作区，待 ARCHITECT 独立复核。
- 决策：首页以模型对话为主要工作入口，左侧悬浮图标进入周报与随记、文案写作、项目跟踪。三类场景共享同一交互骨架与任务生命周期，各自由**可编辑、带版本**的流程包（`omnidesk.offline-flows.v1`）规则驱动起草。内容整理与提炼由模型完成；代码只负责权限、任务状态、文件读写、来源记录、日期/状态计数/逾期判定等确定性工作与文件边界控制。成果只能来自模型初稿并经人工确认后保存，不得用本地模板拼接冒充模型产出；确定性输出单独成栏、标注「代码计算」，不作为成果。模型权限首次严格关闭，用户可按自身授权在设置中逐项开启，由宿主在提交时强制执行。
- 理由：OD-ARCH-025 独立复核判定旧形态（首页仪表盘 + 本地确定性汇总）不是用户需要的 AI 工作台，且本地汇总不能冒充模型产出。
- 影响：材料只读用户显式给出的**单个**文件（绝对路径、扩展名白名单、大小上限、UTF-8），不扫描目录、不递归、不解析数据库；流程包新增的 `draft.rules/sections` 为可选字段，旧包缺省时回落到内置默认规则，不可关闭的写作规则（不得虚构、禁入案件材料、待确认等）不由流程包控制。模型权限判定不扩大 D-004 所记录的许可范围，也不等于端点隔离证明。
- 来源：用户批准的 OD-ARCH-026（PLANS.md）；GOALS.md；`desktop/src/shell/HomeChat.tsx`、`desktop/src/OfflineFlows.tsx`、`desktop/src/aiDraft.ts`、`desktop/src-tauri/src/task_shell.rs`、`desktop/src-tauri/src/flow_pack.rs`。

### D-020：窗口外框一体化，并与确认的首页共同作为 UI 基线
- 状态：已实施并经用户现场验收（2026-10-19）；未提交、未推送、未打标签。
- 决策：主窗口采用 Tauri frameless（`decorations: false`），最小化/最大化/关闭由顶栏自绘控件承担；顶栏作为拖动区；用 8 个边缘/角落热区调用 `startResizeDragging` 补回缩放；窗口相关能力按最小集授予（minimize / toggle-maximize / close / is-maximized / start-dragging / start-resize-dragging / internal-toggle-maximize）。该外框与当前“对话中枢”首页共同构成**已确认 UI 基线**（GOALS.md）。
- 理由：用户要求系统标题栏融入软件，而不是保留系统外框；无边框后窗口管理器不再提供边框缩放，必须显式补回，否则会退化成不可缩放窗口。
- 影响：`decorations` 与权限编译在 Rust 二进制中，**改动必须重建并完整重启，前端 HMR 不生效**；Xvfb 无窗口管理器，不能证明系统装饰消失，真实桌面效果只能由用户本机会话验收。回退路径：`decorations` 改回 `true`、移除顶栏控件与缩放热区后重建。用户已确认重启后系统标题栏消失、拖动与— □ × 均正常；双击最大化、边缘缩放、最大化图标切换未被逐项点名验证。
- 来源：用户批准与验收记录（PLANS.md、STATUS.md、REVIEW.md）；`desktop/src-tauri/tauri.conf.json`、`desktop/src-tauri/capabilities/default.json`、`desktop/src/shell/WindowChrome.tsx`、`desktop/src/shell/TopNav.tsx`、`desktop/src/App.tsx`。

### D-023：项目级固定双 Agent——超级Plan（规划）与超级劳工（执行）
- 状态：已实施并已入库（2026-10-19 实施；文件进入提交 `593c49f`，随 OD-ARCH-029 批次推送 `origin/main`）；**四项复测 2026-10-20 晚全部闭环**（`git rev-parse` 放行 / 管道命令语义明确 / `git diff --check` 通过 / 嵌套派工经用户批准 `experimental.subagent_depth: 2` 后端到端调通）。
- 决策：在仓库内以 OpenCode V2 项目级 agent 固定两个角色，均不设 `default_agent`、不改全局模型路由与凭证。**超级Plan**（ID `super-plan`，model `opencode/glm-5.3-flash-free`）负责只读调研代码库与文档、需求分类、产出计划写入 `PLANS.md`，经用户明确批准后派工；权限为「默认全拒后逐条放行」：read/grep/glob、仅 `PLANS.md` 的 edit、只读 git 命令、仅 `super-worker` 的 subagent，并对 `.env*`/`*credentials*`/`*secret*`/`*.pem`/`*.key` 追加 read deny。**超级劳工**（ID `super-worker`，model `opencode/deepseek-v4.1-flash-free`）负责在获批范围内实施，权限等同 build 默认，边界靠 prompt 红线约束（范围刚性、禁改 `runtime/`、禁读凭证、禁触真实业务材料、开工记快照、执行记录写 `STATUS.md`、完成即停等待复核）。
- 理由：把「规划」与「执行」分成两个可识别、模型固定、权限分层的角色，使计划产出与实施证据可分别审查，降低单会话既规划又动手的越界风险；模型按用户指定分工（规划用 GLM-5.3-Flash Free，执行用 DeepSeek V4.1 Flash Free），不使用会话继承模型。
- 影响：两 agent 的 system prompt 内嵌 AGENTS.md 纪律，形成**配置级**防线而非代码级安全控制；super-worker 保留编辑与 shell 能力，若出现越界应先加权限白名单。规划方唯一可写文件是 `PLANS.md`，因此计划登记与状态更新仍由 ARCHITECT 落盘。当前已验证：两 agent 可被发现加载、super-plan 只读 git 白名单逐条命令生效（`git rev-parse`/`git diff --check` 通过，**管道复合命令被拒**——白名单仅按单条命令前缀匹配，安全上合理）、未放行命令被拒、未越界写入、frontmatter 静态解析通过；**派工链路已打通**：用户批准新增项目级 `.opencode/opencode.json` 设 `experimental.subagent_depth: 2`（实验性设置，写入后同会话即时生效），ARCHITECT→super-plan→super-worker 端到端调通（super-worker 自报模型 `deepseek-v4.1-flash-free` 并只读回报 HEAD）。复审触发条件：任一 agent 出现越界写入、模型绑定被改、`experimental.subagent_depth` 语义随 OpenCode 升级变动（该键为实验性）、或双绑定模型额度长期耗尽导致不可用。
- 来源：用户指令（2026-10-19）；PLANS.md「工具链计划 T-001」；STATUS.md 与 REVIEW.md 同名小节；`.opencode/agents/super-plan.md`、`.opencode/agents/super-worker.md`。

### D-021：三场景模型起草闭环的验收口径与已知边界
- 状态：已确定（OD-ARCH-028，用户 2026-10-19 批准执行；ARCHITECT 复核通过）。
- 决策：产品形态的口径从「三场景由模型起草」推进到「三个场景的完整闭环已在真实窗口被证明」：用户明确选择材料 → 模型起草 → 人工编辑 → 人工确认 → 写到仓库外数据目录（不覆盖）→ 可读回。**失败与取消不得产生任何成果文件，也不得显示模型正文。** 材料读取的白名单是**扩展名级**（绝对路径、普通文件、md/txt/csv/tsv/json/yaml/log、≤512 KiB、UTF-8、禁 DSH/agent/仓库），**不是内容级**：宿主不检测所选文件里是否含凭证，这一点必须公开为已知边界，不得表述为「已做凭证防护」。
- 理由：OD-ARCH-026 只在「周报」一个场景上点完了界面闭环，写作与项目只有引擎层证据；本轮补齐界面闭环并增加失败/取消负例，避免「三个模块都能用」成为未经验证的推断。
- 影响：真实模型质量、真实 Wayland 桌面、输入法与粘贴仍未验证；试点评价中的**人工澄清数与返工轮数必须由用户首次真实使用时逐次记录**，自动化验收结果（记 0）不得当作体验数据；引擎未上报 usage 时，Token 一律记「未知」，不估算、不累加。
- 来源：OD-ARCH-028 证据索引 `tests/verification/desktop/evidence/od-arch-028/README.md` 与 REVIEW.md 同名小节；`desktop/src/shell/OfflineFlows.tsx`、`desktop/src/aiDraft.ts`、`desktop/src-tauri/src/task_shell.rs`。

### D-022：提交与推送的授权边界（一次性备份授权，不成为常态）
- 状态：已确定（OD-ARCH-029，用户 2026-10-20 批准执行）。
- 决策：仓库此前**一律不提交、不推送、不打标签**（见 PLANS.md 顶部约束与各计划的「不提交」条款）。
  OD-ARCH-029 的一次性批准**只**授权把当时工作区的既有改动**分批提交并推送到 `origin/main`**，
  作为防单点丢失的备份；**不**授权常态化提交、**不**授权合并 `omniharness-ui` worktree、**不**授权打标签，
  **不**改变「不读凭证、不提交真实材料与仓库外数据」的硬边界。提交前必须对每个批次做凭证/敏感内容扫描
  并人工核对文件清单；命中疑似真实材料或凭证时**停止该批**并报告。
- 理由：工作区长期堆积上百项未提交改动，单点丢失的代价高于一次性备份的暴露面；分批 + 扫描 + 逐批登记
  使备份可审计、可回退。
- 影响：本次 8 批提交（`2705148`…`3f84ca5`）已推送到**公开**远端 `github.com/klzone/omniharness`；
  提交内容经扫描与人工核对，只含产品代码、流程包、合成样例、验收装置、合成证据与文档，
  **不含真实材料、凭证、会话日志或仓库外数据**。后续计划若要提交，仍需**单独批准**。
- 来源：OD-ARCH-029 计划与执行记录（STATUS.md / REVIEW.md 同名小节）、`docs/agent-toolchain-retest.md` 的同一边界纪律。

### D-024（**待用户裁定，尚未实施**）：接缝选择复盘——ACP 窄接口 vs 直接基于 DSH 应用

- **状态：候选方案，等待用户裁定；本轮不实施（仅核实源码事实）。**
- **用户指出的问题：** 原意是「在 DSH 原代码的基础上增加功能，替换或改造它的 UI」；
  现在的实现却是在 DSH 的 **ACP 窄接口**上重造了一套对话栈，于是流式、思考链、多轮、模型目录
  这些 DSH 本来就有的能力，只能一个个手工补（流式是本地逐字效果、思考链是补传文本、多轮是重放上文模拟）。
- **复盘结论（架构判断，不是推诿）：** 「引擎契约难」是**症状不是根因**。根因是**接缝选窄了**：
  DSH 自己的应用（本地 runtime 里的 `dsh-web` / `dsh-host-webserver` / `dsh-client-ui-*` / `dsh-api-*`）
  本来就具备 token 级流式、会话续接、思考块、模型与思考等级目录、Skill 调度；
  我们改从 `omni.submit_task`（每次 `session/new`）这条窄缝进去，能力自然要重做。
- **已核实的源码事实（本地 runtime，文件级证据）：**
  1. **ACP 无 token 级流式（真实限制）：** `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-acp/lib/index.js`
     的 `assistantUpdates()` 是把**已提交的**助手消息转成 `agent_thought_chunk` / `agent_message_chunk`
     —— 思考与正文在提交时**同时**到达，ACP 本身不吐增量。
  2. **真多轮并不需要新契约：** 同一文件里有 `session/new` / `session/prompt` / `session/resume`；
     我们的 `engine/omnidesk_engine/acp_adapter.py` 也**已经实现了 `session/resume`**（`recover_session`），
     但 `submit_task` 每次都走 `session/new`（约 1032 行），**没有把「同一个线程 → 同一个引擎会话」接起来**。
     ⇒ 现在的「上下文累积/重放」是我们自己造出来的绕路，不是引擎限制。
  3. **DSH web 应用会话面更宽：** `dsh-api-session-controller/lib/typert.host.js` 暴露
     `create` / `prompt` / `selectModel` / `modelCatalog` / `page` / `fork` / `cancel` / `search` 等 Remote —— 即
     **每会话选模型、模型目录、会话分叉、历史分页**引擎里本来就有，我们此前是手搓模型选择器与路由覆盖文件。
  4. **流式通道在 web 路径上存在：** `dsh-client-connection` 的 `openStream`（`/api` channel 流载体）与
     `dsh-host-webserver` 对 `text/event-stream` 的处理说明引擎有实时事件流，只是没经 ACP 暴露。
  5. **注入自有 UI 在机制上可行：** profile 的 `cordis.patch.yml` 支持插入/禁用 bundle 条目 ⇒ 路径 A 有落地手段。
- **两条候选路径（供裁定）：**
  - **A. 改 UI**：以 DSH 自己的应用为底座（profile 补丁注入我们自己的 UI bundle），在它上面加我们的功能；
  - **B. 换接缝**：保留我们的 React UI，但把引擎侧从 ACP 换成 **DSH 本地 web 应用的 HTTP API**
    （`dsh-api-session-*` / `dsh-api-settings-*` 等），从而直接继承流式、会话续接、思考与模型目录。
- **代价（必须一起权衡）：** DSH 的这些接口是**内部接口**（非公开契约，会随版本变），需要锁定 runtime 版本；
  换过去之后，OmniDesk 与 DSH 的耦合会明显变深——这正是当初选 ACP 窄接缝的原因（产品独立性，D-002/D-001）。
  **体验继承** 与 **产品独立性** 之间要做一个明确取舍。
- **建议的分步（先小后大，每步可独立验收）：**
  - **第 1 步（不换接缝，先修我们自己的绕路）：** ✅ **已完成（OD-ARCH-034，2026-10-20）** ——
    把「线程 ↔ 引擎会话」接起来（`submit_task` 接受既有 `engine_session_id`，前端线程持有该 id），
    去掉上下文重放，得到真多轮。**实施中修正的事实**：DSH 的 `session/resume` 只接受**已落盘、可重开**的会话，
    对当前进程里还活着的会话会报 `invalid_argument` ⇒ 进程内续发直接再发 `session/prompt`（`continued`），
    进程重启后才 `session/resume`（`resumed`）；两者都不可用才降级新建（`degraded`）。
    台账新增 `session_mode` 可查。**仍未解决：token 级流式**（下面的第 2/3 步才是它的解）。
  - **第 2 步（只验证、不改产品）：** 起 DSH 自带的 web 应用，走它的本地 API 发一条消息，核实
    token 级流式 / 思考块 / 模型目录 / 会话续接是否真的存在，形成证据。
  - **第 3 步（裁定）：** 第 2 步成立再决定走 A 还是 B；不成立就保留 ACP，接受「非流式」。

## 当前未决事项
- **由模型调用预制 Python 处理表格/文档/数据库、以及一般数据库接入：后置未决。** OD-ARCH-026 只实现三类场景的模型起草闭环，不实现该能力；另行规划与验收。
- **候选方向：个人工作台与 Discipline 能力包分层（OD-ARCH-003 已校准，仍待用户确认产品边界）。** 提议底层优先解决作者个人的会话可找回、升级可控和数据自主；纪检监察领域能力作为可选且按用途单独授权的能力包。OD-ARCH-003 已明确：合规政策必须绑定下层（通用底层同样接触会话正文、配置、插件与备份），且“DSH 内部状态可丢弃”不成立（见 D-014）。方向可作为采纳候选，但**产品边界仍待用户确认**后方可冻结。来源：用户提交的《OmniDesk 综合规划 v0.4》、OD-ARCH-003（REVIEW.md）。
- **独立逐消息追加日志：维持 no-go（暂缓），非阻塞。** OD-ARCH-003 已取得锁定版本/隔离合成证据：原生持久化覆盖历史可读、可列出、可 resume、恢复后可持续写入；真正缺口是可见性（b 类）与路径身份（c 类），均无需复制正文。仅当稳定复现故障显示原生持久化 + 备份 + 元数据索引仍达不到目标，且明确需复制正文时，才重新评估；触发评估不等于自动采用。理由与风险见 REVIEW.md 顶部第 5 节。
- 正式分发产品的最终技术栈仍未冻结。现存 Tauri/Rust/React/TypeScript/Python 工程是技术切片证据，不构成产品化批准；桌面产品化须另行满足门槛并获用户批准。
- ~/OmniDesk（隔离运行根）与 ~/YvanAI（既有外部数据/旧试点路径）在历史实现文档中语义不同。不得直接改名、迁移或推断当前桌面 MVP 已统一使用该布局；具体目录迁移须单独计划。
- 授权范围按用户提供的答复摘要严格解释：个人数据分析与生态研判、符合所述脱敏条件、不得分发。其他用途、模型、工具、设备和运行环境均不由该摘要自动授权。

### D-024 增补（2026-10-20，用户方向指示，待 034 证据后正式裁定）

- **用户明确方向：** 项目定位修正为「在 DSH-desktop 基础上改造桌面端 UI 与工作习惯适配」，不是重造 DSH、也不是长期维护一套并行对话栈。凡 DSH 已有能力（设置、模型目录、流式、会话管理）优先复用其本体，OmniDesk 不重复实现。
- **影响：** 倾向 D-024 路径 A（以 DSH 自有应用为底座，经 profile 补丁注入 OmniDesk 的 UI 扩展）。此前自研 React 工作台的定位从「产品本体」调整为「过渡/试验场」，其沉淀（数据布局、成果归档、权限开关、场景预设）作为注入 DSH 的扩展能力设计输入。
- **纪律不变式：** 路径 A 的注入属配置级扩展（profile cordis.patch.yml），不是 fork/修改 DSH 源码；runtime/ 源码仍不可改。正式裁定待 OD-ARCH-034 第 2 步（DSH web 应用本地 API 验证）证据齐后再做。

### D-024 第 3 步裁定（2026-10-21，用户批准）：走路径 B——对话通道换到 DSH 本地 Web API
- **证据：** 045 第 5 步探针实测 token 级流式/会话续接/取消/重启持久/模型目录全部存在（脚本运行级）。
- **裁定：** 引擎侧新增 web-api 适配通道（与 ACP 并存可切换），继承流式、思考链、模型目录；ACP 保留为降级。约束：锁定 runtime 版本；内部接口漂移即停。
- **记录：** OD-ARCH-048 立项执行。

### D-025：会话日志版本化与执行入口终检的语义边界（2026-10-21，已定；OD-ARCH-049 落地）
- **状态：** 已实现（`thread_store.rs` / `pending_writes.rs` / `engine_submit_task`），由单元测试与源码级接线测试钉住。
- **版本化口径：** 线程记录写入必带 `version`（当前 v1）；读取端对无版本/旧版本只做**内存内补字段**迁移，绝不在加载时改写磁盘原文件；版本高于当前支持、或迁移步骤缺失时**拒绝加载并逐条报原因**（`ThreadListing.issues`），不做降级解释、不静默跳过。
- **终检口径：** 任务提交入口在触达引擎之前检查同场景未决确认卡。结果只有两种——通过则任务原样执行；不通过则**整条任务拒绝**并点名卡片，绝不跳过部分内容。拦截粒度是**场景成果目录**（提交请求没有目标文件名）；非场景 intent 不进入拦截范围。
- **坏记录口径：** 待确认目录存在无法解析的记录时选择 fail-closed（拦住并说明），因为无法证明没有挡路卡片。
- **理由：** 与「数据诚实、不静默降级」的既有红线一致；宁可让用户处理一张卡，也不让任务在不确定状态下跑一半。
- **影响：** 单条坏线程文件不再拖垮整次加载，但会在界面提示；单个坏确认记录会挡住该场景任务，需手动清理（错误消息给出目录）。
- **证据：** `cargo test --offline` 175 passed；`thread_store` 4 个版本化用例；`pending_writes` 4 个终检测试；`submit_entry_checks_pending_scene_gate_before_touching_engine`。
- **复审触发：** ①任务提交开始携带目标文件名时，把终检收紧到文件级；②出现「一条坏记录挡住正常使用」的重复反馈时，改为按记录分类降级；③线程格式出现真正的字段级变更（v2）时，补逐步迁移与回归样例。

### D-026：模型路由以 DSH 设置为默认来源；设置界面内嵌为独立窗口（2026-10-21，已定；OD-ARCH-050 落地）
- **状态：** 已实现（`task_clear_model_route` / `task_open_dsh_settings` / `dsh_settings.rs` / `ModelPicker`）。
- **默认来源：** 新建任务与应用重启**不再要求**用户重选模型；没有 OmniDesk 覆盖文件时，直接使用 DSH Profile 里配置的 provider 与默认模型。用户在本应用里显式切换模型仍会写覆盖文件（仅非机密字段）并优先于 DSH 默认；「跟随 DSH 设置」会把覆盖文件归档（改名保留，不删除）后回到默认来源。
- **设置界面：** 不自研第二套模型编辑 UI；用临时端口起一个专用 `dsh web --no-open`，在应用内开独立窗口显示 DSH 自带设置页。会话令牌只留宿主内存、日志打码、窗口无 IPC/文件能力；实例可复用，应用退出时停止。
- **副作用边界：** 设置窗口关闭不得触发引擎收尾（仅主窗口收尾）；DSH 子进程必须收回 stdin（`DEVNULL`），否则会因共享打开描述被设为非阻塞而导致引擎假 EOF 退出。
- **证据：** cargo 160 passed；`test_web_api_unit.py` 26 passed（含 stdin 隔离回归）；真实窗口实测（模型目录、设置窗口、跟随模式、关闭后引擎存活）。
- **复审触发：** ①DSH 设置页在窗口内出现 CSP/权限问题；②需要在设置窗口里直接感知「引擎需重启」的状态；③真实模型端到端验证发现跟随模式的思考等级与预期不符。

### D-027：设置能力收回 OmniDesk 原生页，取代 D-026 的「设置界面内嵌 DSH」一条（2026-10-22，已定；OD-ARCH-053 落地）
- **状态：** 已实现（`model_settings.rs` / `dsh_settings.rs` / `profile_decl.rs` / `SettingsPanel.tsx`）。仅取代 D-026 中「设置界面：用临时端口起 DSH 自带设置页」这一段；D-026 的「默认来源」「副作用边界」两条继续有效，`task_open_dsh_settings` 与 stdin 隔离逻辑未删除。
- **理由：** 用户实测「打开 DSH 设置」弹出的是完整 DSH Web 应用首页（设置只是应用内弹层，官方无直达路由），用户会在两套界面之间来回且容易误改 DSH 原生配置。
- **新口径：** 模型端点、默认模型、思考等级、凭据名与 API Key、场景声明（profile）编辑、权限开关全部在 OmniDesk 原生设置页完成；DSH 完整应用入口降级到「关于（高级）」折叠区，文案注明会打开完整 DSH 应用，默认折叠。
- **密钥口径：** DSH 的 `llm-pi-ai` 只接受 `apiKeyEnv` 引用名，不接受明文密钥；因此 `api_key` 不再作为独立字段保存在 OmniDesk 文件里，而是经 DSH 官方 `credentials/set` RPC 写入 `$DSH_HOME/.credentials.yaml`，OmniDesk 只保存引用名。命令返回值、日志、evidence 一律不含密钥值，只显示「已配置/来源/可写」。
- **配置文件口径：** 模型路由实际加载源是 OmniDesk 自有覆盖文件（默认 `~/OmniDesk/app/config/omnidesk-model.yml`，`OMNIDESK_APP_DATA_ROOT` 可改），由 `scripts/model-routes.mjs` 生成、以 `--patch` 传给引擎；不是 DSH Profile 里的 `cordis.patch.yml`。计划原文的预期路径按实际改正。
- **场景声明口径：** 设置页只能打开当前 Profile 的声明（无路径参数），保存前先校验，写前留 `.bak-<epoch>` 备份，原子替换；校验失败禁止写入。仓库里的模板不因设置页保存而被改动。**路径已由 OD-ARCH-106 改为 `<数据根>/profiles/<Profile 名>/decl.yml`**（原口径是固定 `profiles/discipline.yml`，见 D-034）。
- **证据：** `cargo test --offline` 182 passed（lib，含 `model_settings` 7 个、`local_http` 3 个单测）；`npx tsc --noEmit` 0 错误；`npm run build` 通过；真实窗口四区截图（`tests/verification/desktop/evidence/od-arch-053/`）。
- **复审触发：** ①用户需要直接改 DSH 原生 provider 配置（如自定义 headers、账号级参数）而原生页表达不了时，评估增加「只读展示 + 跳转原生页」而不是继续扩字段；②DSH 改动 `credentials/*` RPC 形态或 `--patch` 参数语义；③出现「原生页保存后引擎起不来」的实测反馈。

### D-028：平台中立与 Profile 规则资产化（2026-10-23，已实现；OD-ARCH-068 落地）
- **状态：** 已实现（`desktop/src-tauri/src/profile_rules.rs`、`samples/profiles/discipline.rules.json`、设置页「规则资产」分页）。
- **定调：** OmniDesk 是通用个人工作台（DSH 底座 + 自有 UI），纪检内容只属于 Discipline Profile。装规则包 = 纪检工作台；不装 = 纯净底座。
- **规则外置：** 红线词表、写作铁律、各场景 rules/sections、默认思考档、隐私动作全部写在 `<应用数据根>/profiles/<Profile 名>/rules.json`（schema `omnidesk.profile-rules.v1`；OD-ARCH-106 前是固定 `profiles/discipline.rules.json`，见 D-034）。平台代码只留机制，零业务内容。
- **纯净底座口径：** 文件不存在 = 零关键词、零铁律、不掩码，界面显示「未加载（纯净底座）」。出厂默认只在仓库里编译期嵌入，**首次启动不自动创建**——用户必须在设置页显式「恢复默认规则包」，「移走文件 → 重启 → 零拦截」才是可复核行为。
- **损坏口径：** 文件损坏（JSON 错、schema 不符、未知字段、超限）一律报错并拒绝提交（`profile_rules_invalid`），**不**静默退回纯净底座，否则等于悄悄关掉用户以为还在的保护。
- **隐私口径：** 身份证 / 手机号默认 `mask`（保留前 3 后 2，长度不变），在宿主出站 `prompt_text` 上执行，本地会话原文不动，回执带 `privacy_masked`；用户可改回 `reject`。业务红线词命中仍拒绝。隐私关键词单独分组（`privacy_keywords`），避免 mask 模式下「关键词先拒、正则再放行」的自相矛盾。
- **分工：** `offline-flows.v1.json` 管「周报怎么组装」，profile-rules 管「什么不能说 + 什么规则必须带上 + 默认思考档」。两文件并存时以 profile-rules 为权威；场景 rules/sections 目前两边各存一份，靠验证脚本逐字比对，尚无自动漂移检测。
- **影响：** 删掉规则文件后平台对任意输入零拦截（合规上属预期行为，需在界面明示）；思考档默认需经 `setModelRoute` 生效（会重启引擎），故只在引擎空闲且用户未手动选档时应用一次。
- **证据：** `cargo test --offline` 223 lib + 2 + 13 + 3 全通过；`verify_profile_rules_068.py` 25 cases / 0 failed（`tests/verification/desktop/evidence/od-arch-068/`）。
- **复审触发：** ①真实端点实测发现随手记并未明显变快；②规则资产与 `offline-flows.v1.json` 出现内容漂移；③需要把规则包分发/版本化给第三方时，评估签名字段与升级迁移。

### D-029（2026-10-24）：引擎进程生命周期 = 双进程组 + owner.json 所有权登记（OD-ARCH-074）
- **状态：** 已实现并复核通过。引擎（serve.py）启动时自建进程组（组长），DSH node 子进程另开独立组；owner.json（0600，数据根 engine-state/）登记 pid/pgid/child_pid/child_pgid/boot_id/cmdline/channel。回收前四重核验（pid 存活 + pgrp 匹配 + cmdline 完全一致 + boot_id 一致），组杀前校验组内全为本产品进程，拒绝包含自身。
- **理由：** 引擎被 SIGKILL 后 DSH 子树必须仍有归属可查；单组方案无法在不误杀前提下回收孤儿。boot_id 防跨重启 PID 复用。
- **互斥口径：** 同数据根同时只允许一个活跃引擎；身份核验不过的组绝不动（unknown_live 拒绝启动）。
- **已知限制：** Rust 侧无 libc，信号走外部 /bin/kill（缺失时回收失败只记诊断）；「引擎活、宿主已崩」的回收依赖 host_pid 判别，真实双宿主场景未测。
- **证据：** tests/verification/desktop/evidence/od-arch-074/process-lifecycle-074.json（21/21）；cargo 5 + pytest 10 通过。

### D-030（2026-10-24）：插件清单冻结 + Skill 受控变更（用户定调，OD-ARCH-077 已落地）
- **状态：** 已实现（产品语义 + 能力管理页文案）。插件冻结由代码层强制：能力管理页无市场入口，宿主无 `capability_market` 命令；Skill 受控目前是产品语义（改内容即时生效、新增只走版本升级），不做文件锁。
- **定调：** 为保运行稳定性，插件清单随版本冻结——出厂哪些插件就固定哪些，新增/减少只能通过升级 OmniDesk 版本完成；能力管理页不再提供「新增插件」入口，只保留查看/启停/更新提示。Skill 文件平时可修改（内容编辑即时生效），但新增 Skill 不提供产品内入口，同样走版本升级流程；用户直接在文件系统手改仍自由，产品语义上是受控变更。
- **理由：** 运行时随意装插件是此前多次崩溃/会话丢失的诱因之一；冻结后升级路径可测试、可回滚，符合用户「先能天天用再谈扩展」的原则。
- **影响：** ①能力管理页去掉/不提供新增入口；②版本升级流程需带「插件清单 + Skill 清单 + 变更说明」三件套；③用户手改文件系统不在产品管控内（预期行为，作者即用户）。
- **实现落点：** `desktop/src/SettingsPanel.tsx`（能力管理页只保留查看/启停/更新提示，新增「新增插件与 Skill / 随版本升级」说明）；`desktop/src-tauri/src/lib.rs` 移除 `capability_market` 命令及其注册；`desktop/src/{api.ts,types.ts}` 同步去掉 `capabilityMarket`；保留只读的插件/Skill 列表与启停命令。
- **证据：** 真实窗口截图 `tests/verification/desktop/evidence/od-arch-077/09-capabilities.png` 显示冻结文案且无市场开关；`tests/verification/desktop/evidence/od-arch-077/facts.json` 中 `capability_freeze_copy` / `no_market_entry` / `no_capability_market_cmd` 均为 true（`passed: true`，`missing_checks: []`，`gate_errors: []`）；源码事实见上述文件；用户 2026-10-24 指令「对照 DSH 官方桌面，把除了插件与 skill 外的功能都开发出来……插件定制好后不再新增……skill 平时可以修改但不能随意增加」。
- **复审触发：** ①出现真实的高价值插件/Skill 需求且冻结带来明显不便时，评估「受控新增」（签名包/白名单）而非放开自由安装；②用户开始分发给同事（打包阶段）时，需补版本升级的用户侧体验设计。

### D-031（2026-10-26，OD-KB-001 P1 已实施，待独立审查）：知识库使用独立的只读根
- **状态：** 已按 P1 实施，等待 ARCHITECT 独立审查；审查通过前不视为已确定的产品契约。
- **决策：** 新增独立设置字段 `knowledge_vault_root`（单选一个根），不复用 `library_root`，也不让 `OMNIDESK_KNOWLEDGE_ROOT` 作用于它。`library_root` 继续承担工作记录、周报、技能文件与 `@` 引用的写入语义。
- **理由：** 消费者核查（P1-S01）显示 `library_root` 是产品自身写盘目录，工作记录与周报都会在其下创建文件；把用户只读笔记库合并进去，会让“只读”与“可写”落在同一路径，且 `@` 引用、技能写入、Obsidian 外部整理会相互干扰。分类归档绑定 `thread_categories[].vault` 是成果写入目标，同样不能当作读取根。
- **影响：** ①知识库只提供状态、选根、清除、扫描、读单篇笔记五个命令，无写入/删除/批量改写；②每次状态、扫描、读取都重走根准入与逐层符号链接校验，不信任已保存的旧路径；③旧的 `library_root` 设置与已有目录不迁移、不合并、不改名；④`knowledge_vault_root` 缺失时按“未选择”处理，不创建默认知识库目录。
- **边界：** 扫描只保留内存结果，不建持久索引；不读写 `.obsidian/` 配置；隐藏目录与常见依赖目录整棵跳过。
- **证据：** `desktop/src-tauri/src/knowledge_vault.rs`、`desktop/src-tauri/tests/knowledge_vault_host.rs`、`tests/verification/desktop/verify_knowledge_vault_p1.py`（14/14，`evidence/od-kb-001-p1/`）；执行记录见 STATUS.md 的 OD-KB-001 P1 段。
- **复审触发：** ①用户需要多个知识库根或跨根统一搜索；②需要把某个分类的知识库与成果归档指向同一目录；③真实库实测显示排除名单不足或扫描超时；④决定引入持久索引或 watcher 时，重新评估读取路径与失效竞态。

### D-032（2026-10-28，OD-ARCH-104 已实现，待真实模型实测）：随手记的语义判断归模型，写盘权归宿主
- **状态：** 已实现（desktop/src/workRecordVerdict.ts、desktop/src-tauri/src/worklog.rs 的 move_entries、desktop/src/shell/HomeChat.tsx 的 settleNote）；真实模型的分类准确率尚未实测。
- **决策：** 一句话是事实还是指令，由模型判定；宿主只做结构与事实校验，然后一次性落盘。模型在正文之后输出一个 omnidesk-verdict JSON 块，字段为 action（record/move/confirm/answer）、records[{text,date}]、moves[{date,index,to}]、reply。
- **理由：** 前端正则分不清「上杭教学和参观紫金矿业都是本周一的行动，你全部调整至周一的记录」这类句子；上一版把它整句当成一条记录写进周三，正是用户实测踩到的坑。语义判断本来就是模型的本职。
- **校验口径（代码不可省）：** ①records 的 text 必须是用户原话的连续片段，逐字比对（只忽略标点与空白），防模型编造；②日期必须是当周且不晚于今天；③moves 的 index 必须落在当日索引范围内，且条目原文与文件一致；④同一句不得既 record 又 move；⑤批量移动一次读盘、全部校验通过后只重写一次，条目总数与结构标记必须守恒。
- **失败口径（fail-safe）：** 没有判定块、JSON 不合规、校验不过——一律不移动、不改写已有条目，只把用户原话按当天追加，回复里明说「模型未给出判定……未做指令识别」。取消或中断的回合不写盘。
- **索引口径：** 宿主先读当周文件，整理成「日期 #序号 正文」的短索引随提示词送出；模型看不到文件本身，也无宿主命令通道（引擎 mcpServers 恒为空），只能经文本块交回判定。
- **影响：** 随手记提示词从「一句缺项追问」变成「先分类再输出判定」，每轮多几百字符、多等几秒模型；换来的是指令不会被写成记录。周报流程不变，仍走完整组装。
- **证据：** tests/verification/desktop/verify_worklog_104.py 67/67；verify_worklog_103.py 52/52；cargo test --offline --lib worklog:: 38 passed（批量移动、备份、条目守恒、CRLF、越界拒绝）。
- **复审触发：** ①真实模型实测中判定错误多于旧的正则方案；②出现「模型自己编了原话片段」或「索引与文件不一致导致整批作废」的重复反馈；③需要跨周移动时，扩展 move_entries 的读改边界并补迁移测试。

### D-033（2026-10-28，OD-ARCH-105 已实现）：业务判断只在 Skill，平台只提供数据与写盘边界
- **状态：** 已实现并验收通过（desktop/src/conversationPrompt.ts、desktop/src/workRecordVerdict.ts、desktop/src/shell/HomeChat.tsx、skills/memory-agent/SKILL.md）；真实模型准确率尚未实测。
- **决策：** OmniDesk 是通用工作台。判断「这句话是事实还是指令」「该记还是该挪」属于场景资产（Skill），不属于平台。平台在随手记路径上只做三件事：把宿主读到的今天、当周文件与条目索引交出去；声明「写入由工作台执行、模型不碰文件」；注入当前场景绑定的 Skill 原文。没有绑定 Skill 时只说没有可用指引，不臆测口径。
- **协议：** 判定块统一为 `{"append":[{"text","date"}],"move":[{"date","index","to"}],"note":"…"}`。不设 action 分类；两数组皆空表示只回一句话（确认、追问、回答），宿主不碰文件。平台因此不认识「记录 / 移动 / 确认 / 回答」这些业务概念。
- **理由：** OD-ARCH-104 把七条判断原则、JSON 示例与六要素追问写进平台提示词，与 Skill 重复，也让平台带上「工作记录」这个场景的业务限制——换场景、换 Skill 也不解绑。用户明确要求这种判断「都只是基于 skill」。
- **仍然保留在代码里的确定性校验（不下放）：** append 正文须为用户原话连续片段；日期在本 ISO 周且不晚于今天；move 的 date+index 须在当周索引中且正文一致；同一位置不得移两次；同一段话不得既追加又移动；任一不合规整份作废。这些回答的是「能不能安全执行」，不是「这句是什么意思」。
- **影响：** 随手记平台段从带业务规则的长段缩短到 248 字（不含注入 Skill）；提示词与输出都更短，首字与整轮延迟同步下降。Skill 成为唯一需要改判断口径的地方，平台代码一行不动。
- **证据：** tests/verification/desktop/verify_worklog_104.py 70/70（含 platform_owns_no_rule 系列）、verify_worklog_089.py 78/78（platform_prompt_is_thin 248 字）、cargo test --offline 442 passed；证据目录 tests/verification/desktop/evidence/od-arch-105/。
- **复审触发：** ①真实模型实测显示漏块率高，需要把「必须给块」从 Skill 升为平台协议；②出现第二个场景需要同一套 append/move 能力，评估是否把协议抽成通用宿主能力文档；③`workIntent.ts` 的关键词路由成为新的「写死判断」痛点时，改为由 Skill 声明触发条件。

### D-034（2026-10-28，OD-ARCH-106 已实现）：场景声明与规则资产随 Profile 挂载
- **状态：** 已实现（`profile_decl.rs` / `profile_rules.rs` / `lib.rs` 的 `profile_asset_scope`，`Dock.tsx` / `HomeCanvas.tsx` / `SettingsPanel.tsx`）；窗口级证据为 `tests/verification/desktop/evidence/od-arch-106/`。
- **决策：** 业务场景声明与规则资产属于 Profile，不属于平台。落点为 `<应用数据根>/profiles/<Profile 名>/decl.yml` 与 `.../rules.json`。基础标准 `omnidesk-base` 下两者都可以不存在：Dock 只显示平台固有入口（首页 / 历史会话 / 知识库 / 设置），对话按普通对话处理，不套场景提示词、不注入 Skill 原文、不装行业规则包。切到 `discipline` 才挂上场景入口、规则与 Skill 绑定。
- **资产目录取运行中 Profile，而不是磁盘设置。** `profile_asset_scope()` 读引擎 supervisor 的内存配置（`root` / `profile` / `channel`），空 Profile 退回该通道的标准名。理由是用户刚在顶栏改 Profile、还没重启时，磁盘设置已是新名字而引擎仍是旧名字；按磁盘算会让界面提前挂上专业场景，把场景提示词与 Skill 注入到一个没有对应插件的引擎上。
- **与官方 DSH 的关系：** 基础标准的插件面与官方 `dsh-base` 一致（本轮移除了 `skill-filesystem` / `tool-skill` 的 `disabled: true`），模型可按需加载技能；Skill 文件仍与官方 `skill-filesystem` 行为一致，只扫描 DSH home 下 `skills/` 一份，不按 Profile 复制。
- **迁移口径：** 旧 `profiles/discipline.yml` 与 `profiles/discipline.rules.json` 一次性**复制**到 `profiles/discipline/`，目标已存在则跳过，旧文件保留作备份；标准 Profile 不迁移。迁移失败的原因分别回给声明与规则界面（规则页用 `migration_warning`，不塞进会阻断提交的 `error`）。
- **影响：** 换 Profile 即换工作台形态，平台代码不再认识任何业务场景；「恢复默认规则包」在标准模式下被拒绝并说明原因。切 Profile 仍需重启引擎生效，本轮不做热切换。
- **复审触发：** ①需要 Profile 热切换；②需要第三份随 Profile 的界面资产（例如自定义分类映射）；③官方 DSH 改变 `skills/` 扫描口径或 Profile 目录结构；④用户实测「顶栏切了 Profile、界面已变但模型行为未变」再次出现。

### D-035（2026-10-28，OD-ARCH-108 已实施）：Skill 绑定只表达工作流底线，其余交给模型按需加载
- **状态：** 已实施（`samples/profiles/discipline.rules.json`、实机 `~/OmniDesk/app/profiles/discipline/rules.json`、`desktop/src/conversationPrompt.ts`、`desktop/src/shell/HomeCanvas.tsx`、`desktop/src/shell/HomeChat.tsx`、四份 SKILL.md）；真实模型按需加载的实际命中率尚未实测。
- **决策：** `skill_bindings` 的键按**流程**而不是按场景划分，每条流程只绑它离不开的那一个 Skill：`weekly-note` → `memory-agent`，`weekly-report` → `memory-report`，`writing` → `material-writing`，`project` → `project-agent`。`anysearch`、`knowledge-search`、`document-review`、`knowledge-agent` 不进任何绑定，由模型在需要时自己读。旧键 `weekly` 保留为随手记的兼容别名（`memory-agent`），刻意不指向周报：旧规则文件在随手记路径上的行为因此不变。
- **理由：** 用户要的是「场景绑定 skill，模型按需调用额外 skill」。原方案同一个 `weekly` 键同时服务两条流程，周报会把 `memory-agent`、`knowledge-search` 等一并背上（按实机旧绑定 `memory-agent / knowledge-search / anysearch` 计，周报引入的技能正文 4901 字，拆分后为 `memory-report` 的 1099 字），随手记则靠「只取第 1 个」的隐式顺序凑巧正确。把绑定拆到流程这一层，才既保住底线、又不再每轮多付几千字。
- **注入实现：** 意图在提交时才判定，preset 无法事先定型，所以 `preset.skillsByFlow` 同时带出 `note` / `report` 两组，`composeConversationPrompt` 按 mode 取用（`presetSkillsFor`），缺哪一组就回落到场景键。注入位置、单条上限与 2500 字截断线均未改。
- **按需加载的通道（已验证）：** OmniDesk 隔离根上 `--dump-config` 显示 host 层 `skill-filesystem` / `tool-skill` 被关闭，但 `preset-standard` 内两者启用，扫描根为 `~/OmniDesk/runtime/dsh-home/skills`，8 个 Skill 全在。模型读得到，只是要有理由才读——因此四份底线 Skill 各加一小段「什么时候自己加载哪个」。
- **代价与已知限制：** 模型判断不到位时不会去检索，这是用户明确选择的取舍，验收按已知限制记录，不算缺陷；一次按需加载要多一次工具往返。`document-review` 与 `knowledge-agent` 目前没有任何场景会主动用到，保留在目录里等按需。
- **知识库不改：** `KnowledgeVaultPage` 只有宿主侧的扫描 / 读取 / 搜索 / 保存命令，没有 `submitTask`，检索是确定性本地全文检索。因此「知识库绑定 knowledge-search」没有注入点，只能纯按需。若以后给知识库加对话式问答，那是独立计划。
- **证据：** `cargo test --offline`、`pytest tests -q`、`npx tsc --noEmit`、`npm run build`、`verify_skill_bindings_082.py`（含 note/report 各自只注入自己那一个）、`verify_worklog_089.py`、`tests/verification/phase-worklog/verify_env_085.py`、`verify_skill_scope_107.py`。
- **复审触发：** ①真实模型实测中「该搜却不搜」的比例明显偏高，需要回到预注入；②出现第三条共用同一场景的流程，考虑把 `skillsByFlow` 的键改成任意流程名；③知识库加入模型问答，需要单独的绑定键；④用户想自己调整哪条流程绑什么，规则编辑器要能直出这些键。

### D-036（2026-10-28，OD-ARCH-104 插件挂载已实施，待真实模型实测）：办公 Skill 随专业 Profile 挂载，基础标准保持官方插件面

**状态：** 已实施（`samples/profiles/discipline.profile.patch.yml`、实机 `~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.patch.yml`、`tests/verification/desktop/verify_plugin_scope_104.py`）；office 三个技能在真实会话中的实际调用尚未实测；出厂播种缺口未修。

**决策：** 办公能力（Word / PowerPoint / Excel 工作流）以 `@deepseek-ai/dsh-skill-office` 的形式只挂在 `discipline` Profile 的补丁层，写作一条无 id 的顶层 `insert`。`omnidesk-base` 不加，保持「自定义 UI + DSH 官方底座」的插件面与官方 `dsh-base` 一致。

**为什么是宿主层而不是 preset 层：** DSH 的补丁语义里 `config` 是整体替换，`preset-standard` 也没有 `group: true`，不存在「往 preset 的 plugins 数组追加」的路径。而 `skill` 注册表本来就留在宿主层并且是 host + per-scope 分层（`dsh-web-app/cordis.patch.yml:493-500` 有明确注释）：宿主级 provider 落 global 层，preset 的 `skill-filesystem` 落 preset 层，agent 读合并后的目录。因此把 office 挂在宿主层，preset 的 `tool-skill` 就能列到并加载它，且不用复制 preset 里 30 余条插件声明。

**为什么不改 Rust：** `EngineConfig.patch_files` 与 Profile 自身的 `cordis.patch.yml` 两条链路都已支持；这属于 Profile 资产层，`runtime/` 零差异。

**理由：** 用户要求「基础标准 = 自定义 UI + DSH CLI 底座，和官方 DSH-desktop 没差别；切到自定义 Profile 才加载场景、Skill 与插件」。办公插件是场景能力而不是平台能力，挂在基础标准里会让「基础 = 官方」这条边界失守。

**已知缺口：** `engine_supervisor.rs::seed_profile_if_missing()` 只播种 `web` Profile，且 `package.json` 已存在即返回；没有任何安装路径把 `samples/profiles/discipline.profile.patch.yml` 复制到用户机器。所以新装与升级拿不到这条补丁，需后续计划决定补种策略。

**证据：** `verify_plugin_scope_104.py` 34/34；`dsh --dump-config` 对比 discipline（含 `skill-office`，顶格、无 disabled）与 omnidesk-base（0 次）；真实启动 `--profile discipline --no-open --port 0` 打印 web 地址且 stderr 为空；插件直连探针列出 `office-docx / office-pptx / office-xlsx` 且正文含 `Installed LibreOffice Kit`。

**复审触发：** ①真实模型会话里看不到 office 三个技能，说明宿主层合并口径变了或 preset 未加载 `tool-skill`；②官方把 skill 注册表改成 per-preset 独占，需要把条目挪进 preset；③修补种逻辑时（决定出厂落盘方式）；④需要第二类办公插件（例如 PDF 或图片处理）时，评估是否合并成一条办公插件组。

### D-042（2026-10-29，OD-ARCH-115 已实施）：OmniDesk 写的 Skill 归它管，切走就收回

**状态：** 已实施（`desktop/src-tauri/src/profile_assets.rs` 的 `reconcile_managed_skills` / `record_managed_skills` / `retire_unwanted` / `restore_parked`、`engine_supervisor.rs::assemble_profile_assets`、`lib.rs::profile_choice_set`）；真实模型端到端未测。

**决策：** OmniDesk 装配进全局 `<dsh_home>/skills/` 的 Skill，必须登记归属；切到不需要它们的 Profile（含基础标准）时移入 `<app_root>/profiles/.parked-skills/`，切回来再归位。停放是**移动**，不是删除。

**为什么必须这么做：** DSH 的 `dsh-skill-filesystem` 把 `<dsh_home>/skills/` 当全局用户根无条件扫描，与 Profile 无关。此前切回 `omnidesk-base` 只做到「不装配」，那 8 个纪检技能仍会被引擎加载——「干净底座」只在配置层成立，Skill 层是假的。

**三条边界：**

1. **登记必须发生在装配之后。** `reconcile` 跑在装配前（此时目标 Skill 还没落盘，登记不了），所以装配成功后由 `record_managed_skills` 补登记。少了这一步，登记表永远是空的，收回也就无从谈起——本轮实现里正是这个缺口让三条集成测试首跑全红。
2. **归位优先于覆盖。** 停放区里的目录先 `rename` 回 `skills/`，目标已存在则不覆盖；随后日常装配按 `drift` 保护判定，用户改过的正文原样保留，不会被内嵌副本冲掉。
3. **基础标准仍可零写入。** 没有登记项时 `reconcile` 是纯空操作：不建 `skills/`、不建停放区、不写登记表。`standard_profile_creates_nothing_at_all` 继续有效。

**停放区已有同名目录时：** 先 `backup_dir` 成 `.bak-<epoch>`，不静默覆盖上一份停放内容。

**证据：** `cargo test --offline --test profile_assets_host` 20 passed，新增 `switching_back_to_standard_parks_managed_skills`、`switching_back_to_profile_returns_parked_skills`、`parked_skill_keeps_user_edit_after_return`；`cargo test --offline` 423 lib + 集成全绿；`verify_profile_assets_109.py` 34/34。

**复审触发：** ①DSH 改成按 Profile 隔离扫描 `skills/`，届时停放机制可以简化为只做登记；②需要多个 Profile 同时持有同名 Skill 的不同版本；③停放区需要界面上可见的清理入口。

### D-048（2026-10-29，OD-ARCH-120 已实施）：插件安装钉运行时版本，并接受 pnpm 的构建脚本放行

**状态：** 已实施（`profile_assets.rs::dsh_runtime_version` / `install_version` / `relax_dep_builds` / `install_plugin` 重试分支）；真实联网安装已在 `/tmp` 隔离根走应用入口验证通过。

**决策：**

1. **安装 spec 的版本来源 = 显式声明 → 运行时 DSH 版本。** 没有显式版本时，从运行中的 CLI 包根读 `@deepseek-ai/dsh` 的 `version` 并拼成 `<pkg>@<ver>`，绝不发裸包名。
2. **pnpm 的 `ERR_PNPM_IGNORED_BUILDS` 视为可自动放行的非致命失败。** 首次失败且输出（stdout 与 stderr 合并判定）里出现该错误时，把目标 Profile 的 `pnpm-workspace.yaml` 置 `strictDepBuilds: false`，然后**重试一次**；回执如实记录「已允许依赖构建脚本」。

**为什么：** 实测两条路都走不通——裸包名被 DSH 的 peer 兼容检查拒掉（registry 默认 `0.1.6-alpha.2` vs 运行时 `0.2.1-alpha.2`），钉了版本又被 pnpm 11 的构建审批拦住。而 OmniDesk 走的是 DSH 的非交互安装通道，界面上没有 `pnpm approve-builds` 的入口，用户除了报错没有别的出路。

**边界：**

- 只关 `strictDepBuilds` 这一个开关，不写逐包 `allowBuilds` 白名单（依赖树在装之前算不出来），也不改 `runtime/`。
- 只在该 Profile 自己的 `pnpm-workspace.yaml` 上写，且保留原有键、幂等不重复。
- 只在「首次失败且确实是这个错误」时重试一次，不做无限重试，也不掩盖其它失败原因。
- 放行范围限于用户主动点「安装」的那一个包；切换 Profile 仍不联网。

**影响：** 「缺失插件 → 点安装」第一次成为真正可完成的动作；失败回执仍保留 DSH 原样输出。

**证据：** `cargo test --offline --lib` 435 passed（新增 `runtime_version_reads_dsh_package`、`install_version_prefers_explicit_then_runtime`、`relax_dep_builds_keeps_existing_keys`）；`OMNIDESK_NET_TESTS=1` 端到端 1 passed（真实装 `@deepseek-ai/dsh-skill-office`，第二次 `attempted` 为空）；集成、`pytest` 89/30、`verify_profile_assets_109.py` 41/41、真实窗口 26/26 均不回归。

**复审触发：** ①DSH 的安装通道提供非交互的逐包批准参数（届时改回白名单更精确）；②pnpm 改变该错误码或输出流；③同一 Profile 需要装带原生编译且确实必须执行脚本的插件（此时需要显式提示用户，而不是默默放行）。
+
### D-047（2026-10-29，OD-ARCH-119 已实施）：插件身份是「包根」，子路径导出不是独立包

**状态：** 已实施（`profile_lifecycle.rs::package_root` / `detect_plugins`、`profile_assets.rs::required_plugins`）；真实联网安装未测。

**决策：** OmniDesk 对「这个 Profile 需要哪些插件」的判定，一律以 npm **包根**为身份。`cordis.patch.yml` 里 `name: '@scope/pkg/sub/entry'` 是模块子路径，归一到 `@scope/pkg` 之后再探测与展示；`package.json` 的 `dependencies` 键同样归一。归一顺序统一为 `sort + dedup`。`install_plugin` 的两段式校验保持不变。

**为什么必须这么做：** 子路径在磁盘上永远没有自己的 `package.json`，`probe_package` 必然报 `missing`；而安装通道拒绝非两段式名字。两件事叠加就是「永远报缺、永远装不上」的死角——新用户从底座建一个 Profile 就会立刻撞上，且没有任何操作能让它消失。真实窗口里表现为资产页写着「一键缺失（2）」、点安装报名字不合法。

**归一化的边界：** 只折叠 scoped 的三段及以上。`@a/b` 原样；非 scoped 的 `plain/sub` 原样返回，交给安装侧的合法性判断——宁可让它显式失败，也不静默改写成另一个包名。

**影响：** 旧登记表里已写死的子路径会在下次探测时自然纠正，不需要迁移脚本；同一 Profile 的插件清单在界面上顺序稳定。基础标准的插件面仍为空。

**证据：** `detect_plugins_folds_subpath_exports_to_package_root`、`package_root_only_strips_scoped_subpaths`；`cargo test --offline --lib` 432 passed；集成全绿；`preview_profile_custom_118.py` 真实窗口 26/26 且「一键安装缺失插件」整段消失、`plugins` 变为 `[]`。

**复审触发：** ①DSH 允许把非 scoped 子路径当独立包发布；②需要在 Profile 里并列同一包的多个子入口并要求逐个安装；③插件身份需要支持 tag / 别名（届时包根规则要扩展）。

### D-046（2026-10-29，OD-ARCH-118 已验收）：自建 Profile 必须可建、可切、可删、可找回

**状态：** 已验收（验收脚本 `tests/verification/desktop/preview_profile_custom_118.py`，真实窗口 26/26）；真实模型端到端与真实联网装插件未测。

**决策：** 用户自建 Profile 的生命周期是一条必须整段成立的链路，不能只验中间一段：

1. **建**：从「基础标准」新建时，产出的是干净工作台——不复制业务 Skill、不继承业务分类、Dock 不长出业务场景；配置只落 `dsh-home/profiles/<name>/` 与注册表。
2. **切**：确认切换后 `engine_profile` 立即等于新名字，分类表收敛为 `["plain"]`。
3. **看**：设置页「Profile 资产」必须列出它，并给出「移除」入口。
4. **删**：当前在用的 Profile 不允许移除（前端 `disabled` + 后端拒绝），必须先切回别的；移除是**移入** `app-data/profiles/.trash/<name>-<epoch>-<pid>`，同时清掉 `dsh-home/profiles/<name>`——配置可找回、引擎侧不残留。

**为什么必须整段验：** 这条链路上每一步都有独立的状态写入，只验「建出来有文件」会漏掉「切过去引擎没换」「资产页看不见」「删了但线索残留」这三类问题；而「建完直接删」在业务上根本不允许，只写单测就会误以为它可用。真实窗口一次跑完才是用户实际的动作序列。

**测试口径约束：** 自建 Profile 的顶栏徽标文案是自定义显示名，共用定位器只认「标准 / 专业」，验收脚本因此自带 `open_popover_any`；该函数必须看到弹层真的展开才返回成功（资产页里有同名条目）。「资产页还列着它」不能用一行文本证明——WebKit 不把资产行标题的 span 放进 AT-SPI 树——改用「行内『移除』按钮可见 + 注册表仍有条目」两处并证。

**影响：** 新增一条端到端验收脚本；产品代码本轮零改动。后续任何改动 `profile_lifecycle.rs` 的 create/remove、`profile_choice_set`、资产页自建列表的改动，都必须重跑本脚本。

**证据：** `preview_profile_custom_118.py` 26/26、`EXIT=0`；`pytest tests -q` 89 passed / 30 subtests；`verify_profile_assets_109.py` 41/41；facts 中 `trash_entries` 与 `dsh_trash_entries` 同为 `my-desk-1793247214-118499`。

**复审触发：** ①允许删除当前在用的 Profile（需先定义运行时如何停机）；②回收目录需要界面上的「恢复」入口；③自建 Profile 需要从非基础来源复制业务资产（届时第 1 条「干净起手」要按来源分档）；④DSH 改成按 Profile 隔离扫描 `skills/`。

### D-045（2026-10-29，OD-ARCH-117 已实施）：复制 Profile 必须带分类表，停放区必须可读

**状态：** 已实施（`profile_lifecycle.rs::create` 的分类复制分支、`profile_assets::ParkedSkill` / `list_parked`、`lib.rs::profile_assets_parked`、`api.ts` / `types.ts` / `SettingsPanel.tsx` 的「已停放」一节）；真实模型端到端未测。

**决策：** ①从 A 复制出 B 时，分类表随配置一起复制，取值顺序为「A 正在生效的设置 → A 的 `categories.json` → 出厂内嵌清单」，落盘统一走 `profile_categories::save` 以便把 `profile` 字段改写成 B；复制失败不阻断创建，B 仍可走 116 的「继承一次」。②`profiles/.parked-skills/` 提供只读清单（名字、路径、条目数），目录不存在时返回空表且不创建目录；界面只展示、不提供删除，**停放是「移走不删除」，归位由切回需要它的 Profile 触发**。

**理由：** 复制是用户「照这套工作台再来一份」的明确意图，分类表不带就等于把「当时停在哪个 Profile」这个偶然事实写进了结果。停放区在 115 之后成了唯一解释「业务技能为什么消失了」的地方；没有只读出口，用户只能翻文件系统。

**影响：** 复制出的 Profile 自带分类，之后两边互不影响（再改不回流）。停放区多一个只读命令与设置页小节；基础标准态仍然零写入——`list_parked` 只读盘、失败即返回空表。

**证据：** `create_from_profile_copies_categories`、`list_parked_is_read_only_and_sorted`、`cargo test --offline --lib` 430 passed、`--tests` 集成全绿、`verify_profile_assets_109.py` 41/41、`preview_profile_switch_110.py` 16/16（`parked_skills` 8 项、`skills_after_base` 为空）。

**复审触发：** ①停放区需要手工归位或清理入口；②同一 Skill 需要在多个 Profile 并存不同版本；③DSH 改成按 Profile 隔离扫描 `skills/`，停放机制可简化为只做登记。

### D-044（2026-10-29，OD-ARCH-116 已实施）：对话分类属于 Profile，基础标准只留「未分类」

**状态：** 已实施（`settings.rs` 的 `standard_thread_categories` / `REQUIRED_CATEGORY_KEY`、新模块 `profile_categories.rs`、`samples/profiles/discipline.categories.json`、`lib.rs::profile_choice_set`、`threadCategories.ts`、`SettingsPanel.tsx`）；真实模型端到端未测。

**决策：** 对话分类表是 Profile 资产，不是全局设置。存放位置是 `<app_root>/profiles/<name>/categories.json`（schema `omnidesk.profile-categories.v1`），出厂 Profile 的清单在编译期嵌入，基础标准态只允许「未分类」（`plain`）一类。`Settings::default()` 与缺字段的旧设置文件都按「只有 plain」读回。

**为什么不继续放全局 `settings.v1.json`：** 分类是业务词汇——「工作记录 / 文案写作 / 项目跟进」只对纪检专业成立。放在全局字段里，切回基础标准后首页胶囊、历史会话下拉、设置页分类卡片仍在说业务话，干净底座在分类这一层漏了。上一轮 114/115 已让配置、场景、规则、Skill、插件跟着 Profile 走，分类是同一性质，必须有同样的归属。

**实现要点：**

1. 切换前把离开的非标准 Profile 的当前分类表**回存**到它的 `categories.json`，用户在该 Profile 下的改名与新增不会切一次就没。
2. 切入后按「目标 Profile 的文件 → 出厂内嵌清单 → 从当前分类继承一次」的次序取值；基础标准收敛为只有 `plain`，且**不写** `profiles/omnidesk-base/` 任何文件。
3. `validate_categories` 不再强制四键齐全，只要求 `plain` 存在且键唯一；缺 `weekly/writing/project` 视为该 Profile 没有这个业务分类。
4. 既有线程的 tag 一个都不改：切走后标签显示回落为「未分类」，切回原 Profile 恢复真名。这是显示口径，不是数据迁移。
5. 前端在 `onProfileChanged` 里重新拉分类，`SettingsPanel` 的 `profileEpoch` effect 清掉分类草稿与错误；`threadCategories.ts` 的初始值改成只有 plain，避免启动瞬间闪出业务分类。

**证据：** `cargo test --offline --lib profile_categories` 5 passed；`--test profile_categories_host` 4 passed；`cargo test --offline --tests` 428 lib + 全部集成套件绿；`npx tsc --noEmit` 无输出；`npm run build` 通过；`pytest tests -q` 89 passed；`verify_profile_assets_109.py` 38/38；`preview_profile_switch_110.py` 22/22 静态 + 真实窗口 16/16（`base_categories: ["plain"]`）。

**已知限制：** 分类表只在切换 Profile 时同步，手改 `categories.json` 需再切一次或重启（与场景声明文件同口径）；既有线程 tag 不迁移。

**复审触发：** ①需要在同一 Profile 内保留多套分类方案（例如按项目切换）；②自定义 Profile 的分类需要跨机器迁移或导入导出；③线程 tag 需要随 Profile 改名做批量重映射时。

### D-043（2026-10-29，OD-ARCH-114 已实施）：切换时把缺的插件摆到台面上，安装只由用户触发

**状态：** 已实施（`profile_assets.rs` 的 `required_plugins` / `ensure_plugins` / `EnsureReceipt`、`lib.rs::profile_plugins_ensure`、`TopNav.tsx`、`SettingsPanel.tsx`）；真实联网安装未在正式实例上跑。

**决策：** 切换 Profile 的回执必须列出仍未就绪的插件（`plugins_missing`），界面据此给出「安装缺失插件」入口。实际安装走 `dsh plugin --profile <p> add` 官方通道，**只由用户点击触发**，装配与切换路径不联网。

**为什么不在切换时自动装：** 装配是本地写盘，用户点「切换」的预期是切换；装插件会联网拉包，可能在受限网络里长时间挂起，也可能引入清单外的依赖变化。切换是同步操作，把网络写塞进去会让「切换失败」与「插件没网」混成一个错误。

**实现要点：**

1. `plugins_missing` 来自装配结果里 `kind == "plugin" && status != "ready"` 的项，基础标准恒为空数组。
2. `ensure_plugins` 只装探测为非 ready 的包，单个失败不中断其余，装完重新探测，`ok` 仅在全部 ready 时为 true；`still_missing` 原样回报给界面。
3. 前端在切换后若仍有缺项，弹层不自动关闭，把「缺哪几个包」直接列出来；选项是「安装」与「稍后」，不点就不联网。设置页的「一键安装」也改走同一个批量命令，减少 N 次 IPC。
4. 插件清单用所有权类型 `PluginSpec`，自定义 Profile 的登记表条目是 `String`，不为凑 `&'static str` 而 `Box::leak`。

**证据：** `verify_profile_assets_109.py` 34/34（新增「回执带缺失清单」「批量安装命令已注册」「装配路径不调用 ensure_plugins」三项静态断言）；`cargo test --offline` 423 lib + 集成全绿；`npx tsc --noEmit`、`npm run build` 通过。

**复审触发：** ①需要离线安装（内网镜像）时，安装通道要能换成可配置源；②插件安装需要进度反馈而非一次性等待；③出现「装上了但引擎仍起不来」的版本冲突案例。

### D-041（2026-10-29，OD-ARCH-113 已实施）：资产页能就地修复，且 Skill 巡检要比内容

**状态：** 已实施（`desktop/src-tauri/src/profile_assets.rs`、`lib.rs` 的 `profile_assets_repair`、`SettingsPanel.tsx`、`api.ts`、`types.ts`）；真实模型端到端未测。

**决策：** 「资产页只读、修复靠切换 Profile 或重启」这个缺口补上：用户能在设置页直接修复。同时 Skill 的就绪判定从「`SKILL.md` 在不在」改成逐文件比对摘要。

**三条子决策：**

1. **写盘由用户显式触发，默认仍不覆盖。** `sync_bytes` / `sync_skill_files` / `materialize_references` / `record_file` / `materialize` 都加了 `overwrite_drift` 参数，日常装配（启动、切换 Profile）传 `false`，只有 `profile_assets_repair` 传 `true`。恢复出厂内容时先 `backup_of` 成 `.bak-<epoch>`，再写回。`drift` 保护没有因为多了一个按钮而放松。
2. **Skill 就绪 = 每个文件都对。** 新增 `expected_skill_files`（仓库优先、内嵌兜底）与 `inspect_skill`，逐文件比 sha256，得出 `ready / stale / drift / missing`。旧逻辑只看 `SKILL.md` 存在与否，用户改了正文仍显示「已就位」——这是假绿，必须修。
3. **基础标准仍然零写入。** `materialize_with(scope, standard, true)` 在标准 Profile 上照旧直接返回，连目录都不建；修复按钮在基础标准下不渲染。

**为什么不把「缺插件」塞进修复按钮：** 修复只写本地出厂资产，不联网。插件是另一条路径（`profile_plugin_install`，`dsh plugin add`），资产页另给「一键安装缺失插件（N）」，逐个安装、逐项回执，失败不中断其余。混在一起会让「点一下修复」意外产生网络请求。

**证据：** `cargo test --offline` 423 lib + 集成 2+1+13+6+5+10+17+3+1 全绿，其中 `profile_assets_host` 新增 `inspect_detects_edited_skill_body`、`repair_fills_missing_and_backs_up_drift`、`repair_standard_profile_still_writes_nothing`；`verify_profile_assets_109.py` 29/29；`npx tsc --noEmit`、`npm run build`、`pytest tests -q` 均通过。

**复审触发：** ①需要在修复时不备份（例如空间受限环境）；②出现 Profile 之外还需要「一键恢复」的资产（规则、分类配置）；③Skill 正文改由远端分发，届时 `stale` 与网络版本比对要重新设计。

### D-040（2026-10-29，OD-ARCH-112 已实施）：引擎与插件探测必须同源

**状态：** 已实施（`desktop/src-tauri/src/engine_supervisor.rs` 的 `locate_engine_dir` 与 `EngineConfig::from_env`、`desktop/src-tauri/src/profile_assets.rs` 的 `default_dsh_cli_for` / `locate_cli`）；真实模型端到端未测。

**决策：** 引擎启动时解析的引擎目录与 DSH CLI，必须与插件探测/安装用的解析走同一个函数。分发环境下两者不得各自推导路径。

**三条子决策：**

1. **推导只有一份。** `default_dsh_cli_for(repo_root, app_root)` 是唯一入口，`default_dsh_cli()` 与引擎 `from_env` 都调用它。复制同一段路径拼接是「面板说有、引擎没有」的直接成因。
2. **候选按安装布局兜底，但只认真实存在的结构。** 引擎目录依次试 `<repo>/engine`、`<app_root 上一级>/engine`、`$HOME/OmniDesk/engine`，且必须含 `omnidesk_engine/`；全不中时返回首候选交给 `validate()` 如实报错，不伪造可用。
3. **路径推导可测，就别碰进程环境变量。** `locate_cli(repo_root, app_root, home)` 显式接收三个根，单测不读写 `OMNIDESK_*`。并行测试改环境变量会污染同进程的其它用例，制造假失败。

**为什么不内嵌运行时：** 引擎目录与 DSH 运行时是独立安装物，体积与许可边界都不适合塞进二进制。这一层只做「找得到」，不做「自带」。

**证据：** `cargo test --offline` 423 lib + 集成 2+1+13+6+5+10+14+3+1 全绿；`distributed_layout_resolves_engine_and_cli_without_repo` 覆盖 `repo_root` 不存在时的定位；`npx tsc --noEmit`、`npm run build`、`pytest tests -q` 均通过。

**复审触发：** ①安装器改为非 `~/OmniDesk/{app,runtime,engine}` 布局；②出现第二个需要独立定位的运行时组件；③用户希望应用自带运行时（需重新评估体积与许可）。

### D-039（2026-10-29，OD-ARCH-111 已实施）：Profile 的 Skill 与写作资产随二进制内嵌

**状态：** 已实施（`desktop/src-tauri/src/profile_seed.rs`、`profile_assets.rs`、`lib.rs`、`engine_supervisor.rs`）；真实模型端到端未测。

**决策：** 出厂 Profile 依赖的 Skill 正文与写作资产在编译期内嵌进二进制，作为仓库目录不可用时的唯一来源。开发机仍优先用仓库 `skills/`，分发到用户机器后自动退回内嵌副本。

**为什么必须内嵌：** `profile_assets` 原来的仓库根来自 `task_shell::repo_root_hint()`，其兜底是 `env!("CARGO_MANIFEST_DIR")`——编译期源码绝对路径。deb / AppImage 装到用户机器上，那个目录不存在或不可用，于是「切换 Profile」能装出配置、场景声明与规则，8 个 Skill 与写作 references 却全部落 `missing_source`。用户看到的正是「切过去了但工作台是空的」。内嵌约 110K，对 7.5MB 二进制无实质影响，也与本模块既有的 `include_str!`（manifest / patch / decl / rules）做法一致。

**四条子决策：**

1. **仓库优先、内嵌兜底。** 仓库里该 Skill 有 `SKILL.md` 就用仓库，开发期改 `skills/` 立即生效；否则用内嵌正文。代价是两边可能漂移，用逐字节比对测试兜住，而不是靠人工同步。
2. **两条来源写同一套状态。** 新增 `sync_skill_files`：仓库目录与内嵌表都先归一成 `(相对路径, 字节)`，再走既有三态写入（created / updated / unchanged / drift），并逐文件登记摘要到 `.assets.v1.json`。这样「开发机写入 → 用户机器内嵌读取」不会把同一份内容误判成用户改动。
3. **内嵌不放松 drift 保护。** 内嵌是「多一个来源」，不是「有权覆盖」。用户改过的 Skill 在无仓库环境下同样报 `drift`、同样不动。
4. **路径判断改成「可用即用、不存在就走兜底」。** 三处传递仓库根（`profile_choice_set`、`profile_assets_view`、`assemble_profile_assets`）统一加 `is_dir()` 过滤；`default_dsh_cli()` 按「仓库根 → `<app_root 上一级>/runtime` → `~/OmniDesk/runtime`」逐个 `is_file()` 探测，都不中则返回首选候选，由上层如实报「找不到 DSH CLI」。

**不做的事：** 不内嵌 `node_modules` 与 DSH 运行时本身（体积与许可边界都不合适，且运行时是独立安装物）；不改 Profile 白名单与切换时序；不覆盖用户改动；不改变「基础标准零写入」的结论。

**证据：** `cargo test --offline` 422 lib + 51 集成全绿；新增 4 个集成测试（`embedded_seed_fills_skills_without_repo`、`embedded_seed_matches_repo_sources_byte_for_byte`、`embedded_skill_user_edit_is_not_overwritten`、`inspect_without_repo_expects_embedded_seed`）；`verify_profile_assets_109.py` 24/24；`verify_profile_switch_110.py` 22/22；`npx tsc --noEmit`、`npm run build`、`pytest tests -q` 均通过。

**复审触发：** ①Skill 总体积涨到影响二进制分发（例如加入大量范例正文）时，评估改为随包的资源目录而非内嵌；②需要不重建二进制就热更新 Skill 时，重新评估来源优先级与签名校验；③新增第二类需要出厂 Skill 的非标准 Profile 时，把内嵌表从静态清单改为按 Profile 分组的清单；④安装器把 DSH 运行时放到默认布局之外时，重新排 `default_dsh_cli()` 的候选顺序。


### D-038（2026-10-29，OD-ARCH-110 已实施）：切换 Profile = 换引擎，必须一次到位

**状态：** 已实施（`desktop/src-tauri/src/lib.rs` 的 `adopt_engine_config` 与 `profile_choice_set`、`desktop/src/App.tsx`、`shell/TopNav.tsx`、`SettingsPanel.tsx`）；真实模型端到端未测。

**决策：** Profile 是「换一个工作台」，不是「存一个名字等下次启动」。切换必须在同一次操作里完成换引擎、换场景、换规则，中途不得出现「新规则 + 旧引擎」的组合。

**五条子决策：**

1. **顺序固定为：装配 → 写盘 → 重读配置 → 立屏障 → 换配置 → 停旧引擎 → 结局。** 屏障必须早于换配置。否则「换完配置、还没停旧引擎」这段窗口里，新任务会按新 Profile 的规则与注入 Skill 被放行，却被送进仍在运行的旧 Profile 引擎。
2. **两份配置来自同一份 `EngineConfig`。** `adopt_engine_config` 按值接收 `fresh`，supervisor 与 `HostSnapshot` 各取一份克隆。界面显示的 Profile、准入判定用的 Profile、引擎启动用的 Profile 必须同源，否则会出现三处各说各话。
3. **失败关闭。** 旧引擎未能确认停止时 `change.fail(reason)`，屏障保留，回执 `engine_stopped: false`，前端不刷新场景。宁可让用户看到「切换未完成、需重试」，也不接受静默的权限错配。
4. **配置解析失败回滚设置。** 先写盘再 `from_env`；解析失败就把 `engine_profile` 写回切换前的值，避免「磁盘说 A、引擎跑 B」。
5. **不做任务排队与排空。** 切换立即中断在途生成。要做「等当前任务跑完再切」得引入任务队列与切换态，超出本轮范围；那一类需求应作为独立特性重新评估。

**为什么不重启应用：** 引擎本来就是按需惰性启动的（`stop_engine` 后下一条任务自然 `start()`），场景声明、规则与注入 Skill 又都读运行中的配置。因此「停旧引擎 + 换配置 + 前端重读声明」已经足够，重启整个窗口只会多一次白屏与状态丢失。

**为什么环境变量优先于界面设置：** `EngineConfig::from_env` 的解析优先级是 env > settings > 通道默认。env 是开发与测试用的隔离开关，被界面悄悄盖掉会让自动化验证失去意义。代价是：任何脚本要测「顶栏切换」，都必须把 `OMNIDESK_DSH_PROFILE` 留空。

**证据：** `cargo test --offline` 422 lib + 51 集成（2+1+13+6+5+10+10+3+1）全绿；`verify_profile_switch_110.py` 22/22；`preview_profile_switch_110.py` 真实窗口一轮三步 11 项 checks 全 true（`evidence/od-arch-110/`）；`npx tsc --noEmit`、`npm run build`、`pytest tests -q` 均通过。

**复审触发：** ①用户要求「切换前排空在途任务」；②出现需要跨 Profile 迁移会话上下文的需求；③多窗口 / 多实例同时用不同 Profile；④引擎停止耗时超过 10 秒成为常态，需要改成可配置或后台化。

### D-037（2026-10-29，OD-ARCH-109 已实施）：Profile 是一份可安装的资产包

**状态：** 已实施（`desktop/src-tauri/src/profile_assets.rs`、`profile_lifecycle.rs`、`lib.rs`、`engine_supervisor.rs`、`SettingsPanel.tsx`、`TopNav.tsx`）；真实模型端到端未测。

**决策：** OmniDesk 本体保持干净初始环境；Profile 携带自己需要的配置、场景声明、规则包、Skill 与插件清单。切换 Profile 时按清单自动装配，标准 Profile 一个字节都不写。

**五条子决策：**

1. **标准即零写入。** `profile == omnidesk-base`（web-api 通道）时 `materialize` 直接返回，连 `dsh-home/profiles` 都不创建。这是「基础标准 = 自定义 UI + DSH 官方底座」这条产品承诺的代码级落实。
2. **幂等三态，永不覆盖用户改动。** 文件不存在 → 补写；当前摘要等于上次我们写入的摘要 → 更新（先 `.bak-<epoch>` 备份）；其余 → 记 `drift` 并说明原因，一律不动。判定依据是 `<app-root>/profiles/<name>/.assets.v1.json` 里记的哈希，不靠猜。
3. **装配发生在应用启动期，失败不阻断启动。** 引擎是按需启动的（首条任务才 `start()`），把装配挂在引擎启动上会导致冷启动后永不装配——真实窗口实测如此。现在 `build_state()` 在窗口创建前同步装配一次，`start()` 保留幂等兜底；结果写 `logs/profile-assets.log`，错误同时 `eprintln`，但不让整机打不开。
4. **插件安装必须由用户点击。** 随运行时携带的包永远 `ready`，不联网；只有本地解析不到的才算 `missing` 并显示「安装」按钮，执行 `dsh plugin --profile <p> add <pkg>`，显式传 `DSH_HOME` 以免装进 `~/.dsh`。安装结果原样展示 stdout/stderr 与退出码。
5. **自建 Profile 靠登记表管理，移除可恢复。** `app_root/profiles/index.v1.json`（schema `omnidesk.profiles.v1`）登记 `name / label / created_at / origin / plugins`；出厂 Profile 与当前 Profile 不可删除，移除走 `profiles/.trash/<name>-<epoch>-<pid>/` 重命名而非 `rm`。自建 Profile 的 Skill 只按自身 `rules.json` 的 `skill_bindings` 补齐，不写配置文件——配置由用户自己负责。

**为什么不走「每个 Profile 独立 Skill 目录」：** skill-filesystem 在 web app 里由 `preset-standard` 拥有，补丁对 `config` 是整体替换，要隔离就得复制整份 30 余项 preset 配置，升级即漂移。保持 107/108 的共享目录 + 按流程预注入模型。

**证据：** `cargo test --offline` 420 lib + 14 集成全绿；`verify_profile_assets_109.py` 24/24；`preview_profile_assets_109.py` 真实窗口两轮全通过（`evidence/od-arch-109/`）；`npx tsc --noEmit`、`npm run build`、`pytest tests -q` 均通过。

**复审触发：** ①打包分发后仓库 `skills/` 不在原位，需要把 Skill 正文内嵌进二进制；②出现第二类需要出厂装配的非标准 Profile（例如月报），评估是否把静态表扩展为可配置清单；③用户希望 Profile 间真正隔离 Skill 目录，需重新评估 preset 整体替换的代价；④插件安装需要离线包时，评估本地包目录来源。

