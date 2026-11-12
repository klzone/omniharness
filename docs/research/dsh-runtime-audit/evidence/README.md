# 研究证据落盘规则

## 目录

- `phase1/`：版本、来源、Profile、模型路由、已有证据适用性、Pi 对象边界
- `phase2/`：六个核心机制审计
- `phase3/`：DSH/Pi 对照实验
- `phase4/`：归因与改造可行性

## 命名

- 任务输出：`<step-id-lower>-<topic>.json` 或 `.md`
- 原始命令记录：放在同一 JSON 的 `commands` 数组，不另存终端日志正文
- 大体积 dump、会话日志或模型原文不得直接进入 Git；只记录摘要、哈希、路径和可复现命令

## JSON 最小结构

```json
{
  "step_id": "DSH-P1-01",
  "generated_at": "2026-11-12T07:05:36Z",
  "evidence_levels": ["F", "R"],
  "facts": {},
  "commands": [
    {"cmd": "...", "exit_code": 0, "purpose": "..."}
  ],
  "unknowns": [],
  "boundaries": {
    "credentials_read": false,
    "session_bodies_read": false,
    "product_modified": false,
    "runtime_modified": false
  }
}
```

## 禁止

- 不粘贴 `.credentials.yaml`、`.env`、Authorization、Cookie 或密钥值。
- 不粘贴会话正文、任务正文、用户成果正文或真实业务材料。
- 不把完整 `--dump-config`、`app.asar` 内容或依赖树复制进仓库。
- 不把“命令成功”写成“机制正确”；机制结论必须关联代码入口、运行观察或受控实验。
