# OmniDesk Phase 1 验证材料（可重复执行）

本目录是 `tests/reports/phase1-verification-report.md` 的可执行配套。所有会写入的动作都发生在
`/tmp` 下的一次性沙箱或一次性副本内；仓库、`~/OmniDesk`、`~/YvanAI`、`~/.dsh`、`~/.agents`
全程只读或仅做元数据快照。

## 1. 只读副作用验证（`setup.sh --check`）

```bash
python3 tests/verification/phase1/run_verification.py --out tests/verification/phase1/evidence
```

- 正常路径：真实环境执行 `scripts/setup.sh --check`，期望退出码 0。
- 失败路径：一次性沙箱（无底座 + 无隔离根），期望退出码非 0，且底座与 Skill 两项失败都被报告，
  且缺失的隔离根未被创建。
- 只读性：对五个根做检查前后元数据快照（大小 / mtime）比对，期望 changed/added/removed 全为 0。
  快照不读取文件内容，并排除会持续变化的 `~/.dsh/sessions` 子树。

产物：`evidence/readonly-check-evidence.json`、`evidence/readonly-check-summary.txt`。

## 2. 场景 7：隔离 Skill 同步加载

```bash
python3 tests/verification/phase1/run_scenario7.py --out tests/verification/phase1/evidence
```

- 复制仓库为一次性副本，建立一次性隔离根；在副本内给 `material-writing/SKILL.md` 打标记，
  显式同步，再通过 DSH 真实的 `FileSystemSkillProvider` 做发现与正文加载，验证加载的是新版本。
- 需要先由 `scripts/bootstrap-runtime.sh` 安装锁定底座（提供 `@deepseek-ai/dsh-skill-filesystem`）。
- 依赖 `probe_provider.mjs`（按绝对路径导入 DSH 的 Skill 提供者）。

产物：`evidence/scenario7-evidence.json`、`evidence/scenario7-summary.txt`。

## 3. 材料写作端到端（场景 1-6）与资产可定位性

```bash
# 默认：资产可定位性（脚本级证据）+ 真实调用尝试（无凭证则如实记为阻塞）
python3 tests/verification/phase1/run_material_writing_scenarios.py \
    --out tests/verification/phase1/evidence

# 具备模型凭证时，通过环境变量提供（与隔离 Profile 的 apiKeyEnv 一致）：
#   export AGNESCOM_API_KEY=...   # yvan-work Profile 默认路由用的就是这个变量
AGNESCOM_API_KEY=... python3 tests/verification/phase1/run_material_writing_scenarios.py \
    --out tests/verification/phase1/evidence
```

- 端到端调用不写入 `~/OmniDesk`：脚本先把隔离根的 `profiles/`、`skills/` 浅拷到 `/tmp` 下的一次性
  `dsh-home` 副本（默认排除 `sessions/` 与 `.credentials.yaml`），所有 Session 只落在一次性副本中，
  脚本结束即删除。
- 素材全部来自 `fixtures/`（虚构占位内容），提示词由脚本拼装；不输入任何真实人员、单位、案件或内部信息。
- 六个场景：模板/规则齐备、无匹配模板、关键数据缺失、事实矛盾、范例目录为空、数据边界拒绝。
- 无凭证时，DSH 能启动、装配 Profile、创建 Session，但模型调用处返回 `MISSING_CREDENTIAL`；
  脚本据此把六个场景记为 `blocked_no_credential`（证据等级 `unverified`），不降级为通过。
- 有凭证时，脚本解析 headless 的 `final` 事件文本，按场景做可观察判定，证据等级标为 `model`。

产物：`evidence/material-writing-evidence.json`、`evidence/material-writing-summary.txt`。

素材目录：`fixtures/`（含 `README.md` 说明与 4 份虚构素材）。

## 证据等级

以上三组均为脚本 / 隔离环境实测或静态检查：真实执行了脚本与真实 DSH 组件，场景 1-6
在凭证就绪前未发生模型推理。它们不能证明模型会遵守 Skill 内容或模板/规则被模型实际读取，
也不能把场景 1-6 视为已通过。原因见验证报告第 3.3、3.6 节。
