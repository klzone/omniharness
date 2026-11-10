# OD-ARCH-147c 证据摘要：agent-instructions 检索收敛规则

日期：2026-11-10　执行：本助手（未派子代理）　HEAD：b833487867f52b17a4ea6ae480279257a3cc7305

## 1. 机制与选择理由

- `@deepseek-ai/dsh-agent-instructions` 的 Config 只有发现与预算字段
  （`maxBytes` / `maxSourceBytes` / `projectRootMarkers` / `instructionFileCandidates` /
  `localInstructionFileCandidates`），没有可注入指令正文的字段（实读
  `runtime/dsh-runtime/node_modules/@deepseek-ai/dsh-agent-instructions/lib/index.js`）。
- 因此采用 persona 文本：`@deepseek-ai/dsh-persona` 的 `config.prefix` / `config.suffix`
  会注册 `DEPLOYMENT_PERSONA_PREFIX/SUFFIX` 提示段落（实读其 `apply()`）。本卡写入
  `suffix`，保留原有 prefix 身份句不变。
- 关键约束：`dsh-app-boot` 的 `applyEntryPatches` 只索引顶层行和 `group: true` 行的
  子行（`if (entry.group && Array.isArray(entry.config)) buildMap(entry.config)`）。
  `preset-standard` 不是 group，其 `config.plugins` 内的 persona 行无法被 `- id: persona`
  定向 patch（隔离 DSH_HOME 实测：`patch: entry "persona" not found`）。
- 结果：omnidesk-base 的补丁原本就整段覆盖 `preset-standard`，只需追加 persona.suffix；
  web / discipline 的补丁原先依赖上游预设，必须整段 restate
  `@deepseek-ai/dsh-web-app/presets/standard.patch.yml` 的 `config.plugins` 才能改到 persona。
  该段与上游逐字段一致（22 个插件行，含 `tool-plugin-market`），仅 persona 增加 suffix；
  js-yaml 结构化对比：唯一差异是 `preset.config.plugins.0.config.suffix`。
- 代价：上游预设增删插件时需同步本段（已写入文件注释）。

## 2. 指令正文（三个 Profile 完全一致）

```yaml
          suffix: |-
            检索收敛规则：
            - 不要给 web_fetch 的 URL 拼接 r.jina.ai 等第三方 reader 代理前缀。web_fetch 已内置直连、无头浏览器、云端抓取的回退链；遇到反爬页面改用 web_search 或换其它来源。
            - 连续 3 次工具调用都没有带来新的可用事实时，停止检索，直接基于已有信息作答。
            - 连续 15 步只有工具调用、没有任何文本输出时，下一步必须先写一段阶段性结论，再继续。
```

## 3. 改动文件

| 文件 | 改动 |
| --- | --- |
| `samples/profiles/omnidesk-base.profile.patch.yml` | persona 行追加 suffix（原有预设块不动） |
| `samples/profiles/web.profile.patch.yml` | 末尾追加 preset-standard 覆盖块（上游预设原样 + persona suffix） |
| `samples/profiles/discipline.profile.patch.yml` | 末尾追加同一 preset-standard 覆盖块 |
| `runtime/dsh-home-seed/profiles/omnidesk-base/cordis.patch.yml` | 与 base 样例同步 suffix |
| `runtime/dsh-home-seed/profiles/web/cordis.patch.yml` | 与 web 样例同步覆盖块 |

seed 两文件内容与 samples 一致；`runtime/dsh-home-seed/` 被 `.gitignore:60` 忽略，不进入 git status。
147a 已写入的 insert 段落原样保留：base 样例第 145 行起、discipline 样例第 12 行起。

## 4. 实际运行的验证

1. YAML 解析（runtime 自带 js-yaml 4.3.2，`!!js` 自定义 tag）：5 个文件全部解析为顶层列表，
   每个 preset-standard 均带 4 行 persona suffix。
2. 组装验证（隔离 DSH_HOME=/tmp/od147c-home，`node @deepseek-ai/dsh/lib/bin.js --profile <p> --dump-config`）：
   - web：rc=0，dump 1263 行，含 `检索收敛规则`；stderr 3 行仅因隔离根缺 dsh-free-search / cost-meter 包
     （147a 装配状态，非本卡引入）。
   - discipline：rc=0，dump 1253 行，stderr 无输出。
   - omnidesk-base：rc=0，dump 1260 行，stderr 无输出。
   三个 dump 的 `- id: preset-standard` 均在，persona suffix 文本在。
3. `export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib`
   - 沙箱内：1090 passed / 4 failed。
   - 沙箱外（进程表可见）：**1093 passed / 1 failed**；唯一失败是
     `engine_lifecycle_wiring_tests::every_tauri_command_runs_off_the_ui_thread`，
     指向 HEAD 既有的 `desktop_user_name` 同步命令（`git show HEAD:desktop/src-tauri/src/lib.rs`
     第 851 行即为 `#[tauri::command]` 同步函数），与本卡无关，且不在允许修改范围内。
   - 沙箱内另 3 项 `process_owner::tests` 失败是沙箱禁止进程表访问（`is_alive` / `stat child`）；
     沙箱外单跑该组：8 passed / 0 failed。
4. `cd desktop && npx tsc --noEmit`：rc=0，无输出。

## 5. 未验证与已知影响

- 真实模型行为（不再拼 r.jina.ai、3 次无新增事实早停、15 步无文本先出结论）属运行时效果，未验证。
- `tests/verification/desktop/verify_plugin_scope_104.py` 的 `sample_no_entry_override` 断言
  （discipline 样例不得出现顶层 `- id:` 覆盖）会因本卡 mechanism 变为失败；`sample_single_insert`
  断言已被 147a 的两条 insert 打破。两者均不在本卡验收命令内，脚本未改动（超出授权范围），
  留 ARCHITECT 裁定是否更新断言。
- preset-standard 整段覆盖冻结了 web / discipline 的上游预设清单，后续 DSH 升级需同步本段。

## 6. Git 状态（本卡收尾时）

```
 M PLANS.md
 M STATUS.md
 M samples/profiles/discipline.profile.json
 M samples/profiles/discipline.profile.patch.yml
 M samples/profiles/omnidesk-base.profile.json
 M samples/profiles/omnidesk-base.profile.patch.yml
 M samples/profiles/web.profile.patch.yml
 M scripts/diagnose-omnidesk.py
 M scripts/prepare-runtime-assets.sh
 M tests/verification/desktop/verify_diagnose_omnidesk.py
```

未提交、未推送、未打标签。PLANS.md / STATUS.md / scripts / json / verify_diagnose 的改动为 147a、147b
并行工作与本卡无关；本卡只触碰上表 5 个 patch 文件。
