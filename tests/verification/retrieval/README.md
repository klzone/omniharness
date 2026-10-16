# 知识检索链路验收（可重复执行）

本目录是 `docs/OmniDesk-Knowledge-Retrieval-Acceptance-Spec.md` 的可执行配套。
所有会写入的动作都发生在 `/tmp` 下的一次性副本内；仓库、`~/OmniDesk`、`~/.dsh`、
`~/.agents` 全程只读。

## 运行

```bash
# 真实模型（使用隔离实例自身凭证；默认不复制凭证）
python3 tests/verification/retrieval/run_retrieval_scenarios.py \
    --out tests/verification/retrieval/evidence --allow-credential-copy

# 仅复核已有证据、不调用模型（确定性重跑判定）
python3 tests/verification/retrieval/run_retrieval_scenarios.py \
    --out tests/verification/retrieval/evidence --rejudge

# 只跑指定场景
python3 tests/verification/retrieval/run_retrieval_scenarios.py \
    --out tests/verification/retrieval/evidence --only S1,S6 --allow-credential-copy
```

## 判定器自检（无模型调用）

```bash
python3 tests/verification/retrieval/selftest_citation_judge.py
```

自检用真阳性（编造条款必须被识别）与真阴性（逐字引用、省略式提及不得误报）样本，
证明判定器既有牙齿、又不过度误报。**证据等级：脚本**。

## 分别验证的两个命题

- **命题 A（检索真实性）**：解析 headless 事件流的 `tool_call` / `tool_result`，
  核实模型实际调用了什么工具、触碰了哪些夹具文件。
- **命题 B（引用真实性）**：抽取交付文本中的条款级引用，与夹具原文**逐字比对**；
  无法逐字命中的条款即记为「疑似编造」。

两者必须分别验证：检索到正确文本 != 模型准确引用；正确答案 != 使用了指定知识库。

## 安全边界

- 不修改 DSH Runtime；不触碰 `~/.dsh`、`~/.agents` 或原版 DSH 数据；
- 不读取、不复制、不索引 `knowledge/regulations/` 下的 234 份正式制度文件（只用合成夹具）；
- 端到端调用在 `/tmp` 一次性 `dsh-home` 副本中进行，Session 只落在副本，脚本结束即删除；
- `--allow-credential-copy` 仅复制隔离实例自身凭证（不读取、不打印内容）。

## 产物

- `evidence/retrieval-evidence.json`：逐场景结构化证据（含实际工具调用、触碰文件、逐字比对结果）；
- `evidence/retrieval-summary.txt`：人类可读摘要；
- `evidence/raw/<SID>.jsonl`：headless 原始事件流（字节级证据，可 `--rejudge` 复算）。
