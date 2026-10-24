#!/usr/bin/env python3
"""OD-ARCH-020：AI 初稿子流程的离线验证。

直接编译 `desktop/src/aiDraft.ts` 的真实源码，用 Node 跑确定性分支：

1. 提示词只由界面填写内容组成，并包含事实/冲突/待确认/禁入规则；
2. 准入拒绝时进入 refused，且不回填任何文本；
3. 只有 succeeded 且正文非空才回填；失败、取消、超长、空正文都不回填；
4. 等待超时不回填，并把最近状态写进提示；
5. 取消只在首次请求发送，重复点击与终态后均跳过；
6. 取消请求发出后以宿主回读状态为准，不把“已发出”当“已取消”。

**不**验证真实模型输出、真实宿主或窗口交互。不读取凭证，不访问网络，
不启动引擎，不触碰用户数据。

用法：
  python3 tests/verification/desktop/verify_ai_draft.py [--json PATH]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
DESKTOP = HERE.parents[2] / "desktop"
SOURCE = DESKTOP / "src" / "aiDraft.ts"
RULES = HERE.parents[2] / "samples" / "profiles" / "discipline.rules.json"


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=180)


def compile_and_probe() -> dict:
    tmp = Path(tempfile.mkdtemp(prefix="od-aidraft-"))
    compile_result = run(
        [
            "npx",
            "--no-install",
            "tsc",
            str(SOURCE),
            "--outDir",
            str(tmp / "out"),
            "--module",
            "commonjs",
            "--target",
            "ES2022",
            "--skipLibCheck",
            "--ignoreConfig",
        ],
        DESKTOP,
    )
    compiled = tmp / "out" / "aiDraft.js"
    if compile_result.returncode != 0 or not compiled.exists():
        return {
            "compile_exit": compile_result.returncode,
            "compile_stderr": compile_result.stderr[-2000:],
            "probe_exit": None,
            "cases": [],
        }

    probe = tmp / "probe.cjs"
    probe.write_text(PROBE_JS.replace("__MODULE__", json.dumps(str(compiled))), encoding="utf-8")
    probe_result = run(["node", str(probe), str(RULES)], tmp)
    try:
        cases = json.loads(probe_result.stdout) if probe_result.stdout else []
    except json.JSONDecodeError:
        cases = []
    return {
        "compile_exit": compile_result.returncode,
        "probe_exit": probe_result.returncode,
        "probe_stderr": probe_result.stderr[-2000:],
        "cases": cases,
    }


PROBE_JS = r"""
const m = require(__MODULE__);
const out = [];

function record(name, passed, got, expect) {
  out.push({ name, passed, got, expect });
}

function collector() {
  const seen = { phases: [], taskIds: [], statuses: [], drafts: [], messages: [] };
  return {
    seen,
    effects: {
      onPhase: (v) => seen.phases.push(v),
      onTaskId: (v) => seen.taskIds.push(v),
      onStatus: (v) => seen.statuses.push(v),
      onDraft: (v) => seen.drafts.push(v),
      onMessage: (v) => seen.messages.push(v),
    },
  };
}

// OD-ARCH-068：写作铁律已迁出平台代码，改由 Profile 规则资产提供。
const RULES_FILE = process.argv[2];
const writingRules = require(RULES_FILE).global.writing_rules;

const input = {
  sceneLabel: '合成周报',
  purpose: '向合成受众汇报合成进度',
  audience: '合成评审组',
  requirements: '不超过两页，不含真实数据',
  material: '合成事项 A 已完成；合成事项 B 数据缺失。',
  writingRules,
};

// 1. 提示词assembly
{
  const p = m.buildDraftPrompt(input);
  const rules = [
    '不硬套模板', '不得虚构数据', '待确认', '矛盾时明确指出冲突', '案件材料', '事实来源',
  ];
  record('prompt-includes-user-input',
    p.includes(input.purpose) && p.includes(input.audience) &&
    p.includes(input.requirements) && p.includes(input.material) && p.includes(input.sceneLabel),
    p.length, 'contains all user fields');
  record('prompt-includes-safety-rules',
    rules.every((r) => p.includes(r)),
    rules.filter((r) => !p.includes(r)), 'no missing rule');
  record('prompt-does-not-claim-assets',
    !/已读取|已加载|参见模板库|范例库/.test(p),
    'no asset claim', 'no fabricated asset claim');
  const empty = m.buildDraftPrompt({ sceneLabel: 'x', purpose: '', audience: '', requirements: '', material: '' });
  record('prompt-empty-fields-marked',
    empty.includes('未填写') && empty.includes('未提供素材'),
    empty.includes('未填写'), 'explicit empty markers');
}

// 1b. 材料与可编辑规则（OD-ARCH-026）
{
  const p = m.buildDraftPrompt({
    ...input,
    materials: [
      { name: '合成材料.md', sha256: 'a'.repeat(64), content: '合成材料正文：事项 C 尚未开始。' },
    ],
    sceneRules: '按周报结构整理，先给概览再给明细。',
    sceneSections: '本周概述\n事项明细',
  });
  record('prompt-includes-selected-material',
    p.includes('合成材料.md') && p.includes('合成材料正文：事项 C 尚未开始。') && p.includes('aaaaaaaaaaaa'),
    p.includes('合成材料.md'), 'material name + content + sha prefix');
  record('prompt-includes-editable-scene-rules',
    p.includes('按周报结构整理') && p.includes('本周概述') && p.includes('可在工作台流程包中调整'),
    'scene rules + sections', 'editable rules and sections present');
  record('prompt-keeps-hard-rules-with-custom-scene-rules',
    ['写作规则（来自当前 Profile 规则资产）', '不得虚构数据', '案件材料', '待确认'].every((r) => p.includes(r)),
    'hard rules kept', 'hard rules cannot be disabled by the flow pack');
  record('prompt-uses-default-scene-rules-when-absent',
    m.buildDraftPrompt({ sceneLabel: 'x', purpose: '', audience: '', requirements: '', material: '' }).includes(m.DEFAULT_SCENE_RULES),
    'fallback', 'default scene rules fallback');
}

const opts = { intervalMs: 0, timeoutMs: 5000, wait: async () => {} };

async function submission(name, host, expect, runOpts) {
  const c = collector();
  await m.runAiDraftSubmission(host, input, 'synthetic-intent', c.effects, runOpts || opts);
  const drafts = c.seen.drafts;
  const finalDraft = drafts.length ? drafts[drafts.length - 1] : null;
  const lastMessage = c.seen.messages.length ? c.seen.messages[c.seen.messages.length - 1] : '';
  const passed =
    c.seen.phases[0] === 'running' &&
    c.seen.phases[c.seen.phases.length - 1] === expect.phase &&
    (expect.draft === null ? finalDraft === null : finalDraft === expect.draft) &&
    (expect.messageContains === undefined || String(lastMessage).includes(expect.messageContains)) &&
    (expect.submits === undefined || host.submits === expect.submits) &&
    (expect.results === undefined || host.results === expect.results);
  record(name, passed, {
    phases: c.seen.phases, taskIds: c.seen.taskIds, statuses: c.seen.statuses,
    finalDraft, lastMessage, submits: host.submits, results: host.results,
  }, { phase: expect.phase, draft: expect.draft, messageContains: expect.messageContains });
}

(async () => {
  // 2. 准入拒绝
  {
    let submits = 0;
    const host = {
      submits, results: 0,
      async submitTask() { host.submits += 1; throw new Error('端点未确认，提交被拒绝'); },
      async taskStatus() { return { status: 'running', note: '' }; },
      async taskResult() { host.results += 1; return { content: '不该出现', note: '' }; },
      async cancelTask() { return { phase: 'x', final_state: null, note: '' }; },
    };
    await submission('submit-refused', host,
      { phase: 'refused', draft: null, messageContains: '端点未确认', submits: 1, results: 0 });
  }

  // 3. 成功且正文非空才回填
  {
    let polls = 0;
    let captured = null;
    const host = {
      submits: 0, results: 0,
      async submitTask(arg) { host.submits += 1; captured = arg; return { omni_task_id: 'omni-syn-1', status: 'running' }; },
      async taskStatus() { polls += 1; return { status: polls < 2 ? 'running' : 'succeeded', note: '' }; },
      async taskResult() { host.results += 1; return { content: '合成初稿正文', note: '' }; },
      async cancelTask() { return { phase: 'x', final_state: null, note: '' }; },
    };
    await submission('success-fills-draft', host,
      { phase: 'done', draft: '合成初稿正文', messageContains: '未复核', submits: 1, results: 1 });
    record('submit-scan-target-is-user-input',
      typeof captured?.userInputText === 'string' &&
      captured.userInputText.includes(input.material) &&
      captured.userInputText.includes(input.purpose) &&
      !captured.userInputText.includes('写作规则') &&
      !captured.userInputText.includes('案件材料'),
      (captured?.userInputText || '').slice(0, 40), 'user material only, no rule text');
    record('submit-prompt-still-has-rules',
      typeof captured?.promptText === 'string' &&
      captured.promptText.includes('写作规则') &&
      captured.promptText !== captured.userInputText,
      (captured?.promptText || '').length, 'prompt keeps rules separately');
  }

  // 3b. 材料正文进入扫描目标，规则文本不进入
  {
    let captured = null;
    const host = {
      submits: 0, results: 0,
      async submitTask(arg) { host.submits += 1; captured = arg; return { omni_task_id: 'omni-syn-mat', status: 'running' }; },
      async taskStatus() { return { status: 'succeeded', note: '' }; },
      async taskResult() { host.results += 1; return { content: '合成初稿', note: '' }; },
      async cancelTask() { return { phase: 'x', final_state: null, note: '' }; },
    };
    const c = collector();
    await m.runAiDraftSubmission(host, {
      ...input,
      materials: [{ name: '合成材料.csv', sha256: 'b'.repeat(64), content: '合成材料正文标记' }],
      sceneRules: '场景规则标记',
    }, 'synthetic-intent', c.effects, { ...opts, workspaceDir: '/tmp/synthetic-workspace' });
    record('submit-includes-material-and-workspace',
      typeof captured?.userInputText === 'string' &&
      captured.userInputText.includes('合成材料正文标记') &&
      !captured.userInputText.includes('写作规则') &&
      captured.promptText.includes('合成材料正文标记') &&
      captured.promptText.includes('场景规则标记') &&
      captured.promptText.includes('写作规则（来自当前 Profile 规则资产）') &&
      captured.workspaceDir === '/tmp/synthetic-workspace',
      (captured?.userInputText || '').slice(0, 50), 'material content scanned; rules and workspace wired');
  }

  // 4. 失败/取消/空正文/超长正文/超时：都不回填
  const nonSuccess = [
    ['status-failed', 'failed', { content: '不应回填', note: '合成失败' }, 'failed'],
    ['status-cancelled', 'cancelled', { content: '', note: '' }, 'cancelled'],
    ['status-interrupted', 'interrupted', { content: '', note: '中断' }, 'failed'],
    ['empty-content', 'succeeded', { content: '   ', note: '' }, 'failed'],
    ['omitted-content', 'succeeded', { content: '', content_omitted: true, content_length: 999999, note: '' }, 'failed'],
  ];
  for (const [name, status, result, phase] of nonSuccess) {
    const host = {
      submits: 0, results: 0,
      async submitTask() { host.submits += 1; return { omni_task_id: 'omni-syn-2', status: 'running' }; },
      async taskStatus() { return { status, note: '' }; },
      async taskResult() { host.results += 1; return result; },
      async cancelTask() { return { phase: 'x', final_state: null, note: '' }; },
    };
    await submission(name, host, { phase, draft: null, submits: 1, results: 1 });
  }

  // 5. 等待超时
  {
    const host = {
      submits: 0, results: 0,
      async submitTask() { host.submits += 1; return { omni_task_id: 'omni-syn-3', status: 'running' }; },
      async taskStatus() { return { status: 'running', note: '' }; },
      async taskResult() { host.results += 1; return { content: '超时不该取结果', note: '' }; },
      async cancelTask() { return { phase: 'x', final_state: null, note: '' }; },
    };
    await submission('poll-timeout', host,
      { phase: 'failed', draft: null, messageContains: '超时', submits: 1, results: 0 },
      { intervalMs: 0, timeoutMs: 0, wait: async () => {} });
  }

  // 6. 取消闸门
  {
    const base = { taskId: 'omni-syn-4', phase: 'running', status: 'running', requestSent: false };
    const cases = [
      ['cancel-no-task', { ...base, taskId: null }, false, 'no_task'],
      ['cancel-first', { ...base }, true, 'first_request'],
      ['cancel-repeat', { ...base, requestSent: true }, false, 'already_requested'],
      ['cancel-terminal', { ...base, status: 'succeeded' }, false, 'terminal'],
      ['cancel-idle', { ...base, phase: 'idle' }, false, 'not_running'],
    ];
    for (const [name, snapshot, send, reason] of cases) {
      const got = m.cancelGate({
        hasTask: !!snapshot.taskId, phase: snapshot.phase, status: snapshot.status, requestSent: snapshot.requestSent,
      });
      record(name, got.send === send && got.reason === reason, got, { send, reason });
    }
  }

  // 7. 取消以宿主回读为准
  async function cancelCase(name, behaviour, expect) {
    const calls = { cancel: 0, status: 0 };
    const sent = [];
    const messages = [];
    const statuses = [];
    const host = {
      async submitTask() { return { omni_task_id: 'x', status: 'running' }; },
      async taskStatus() { calls.status += 1; if (behaviour.statusThrows) throw new Error('synthetic-status'); return { status: behaviour.status, note: '' }; },
      async taskResult() { return { content: '', note: '' }; },
      async cancelTask() { calls.cancel += 1; if (behaviour.cancelThrows) throw new Error('synthetic-cancel'); return behaviour.outcome; },
    };
    const result = await m.requestAiDraftCancel(
      host,
      { taskId: 'omni-syn-5', phase: 'running', status: 'running' },
      { onRequestSent: () => sent.push(1), onMessage: (v) => messages.push(v), onStatus: (v) => statuses.push(v) },
      false,
    );
    const last = messages.length ? messages[messages.length - 1] : '';
    const passed =
      result === expect.result && calls.cancel === expect.cancel && calls.status === expect.status &&
      (expect.messageContains === undefined || last.includes(expect.messageContains)) &&
      (expect.statusValue === undefined || statuses[statuses.length - 1] === expect.statusValue);
    record(name, passed, { result, calls, messages, statuses }, expect);
  }

  await cancelCase('cancel-confirmed',
    { outcome: { phase: 'confirmed_cancelled', final_state: 'cancelled', note: '' }, status: 'cancelled' },
    { result: 'sent', cancel: 1, status: 1, messageContains: '取消已确认', statusValue: 'cancelled' });
  await cancelCase('cancel-unconfirmed',
    { outcome: { phase: 'acknowledged', final_state: 'running', note: '' }, status: 'running' },
    { result: 'sent', cancel: 1, status: 1, messageContains: '取消未确认', statusValue: 'running' });
  await cancelCase('cancel-ipc-failure',
    { cancelThrows: true, status: 'running' },
    { result: 'sent', cancel: 1, status: 1, messageContains: '未获确认' });

  process.stdout.write(JSON.stringify(out));
})().catch((e) => { process.stderr.write(String(e)); process.exit(1); });
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()

    if not SOURCE.exists():
        print(f"找不到源码：{SOURCE}")
        return 2

    report = compile_and_probe()
    cases = report.get("cases") or []
    failed = [c for c in cases if not c.get("passed")]
    for c in failed:
        print(f"FAIL {c['name']}")
        print(json.dumps(c, ensure_ascii=False, indent=2))
    if not cases:
        print(json.dumps(report, ensure_ascii=False, indent=2)[-3000:])
    print(
        f"compile_exit={report.get('compile_exit')} probe_exit={report.get('probe_exit')} "
        f"cases={len(cases)} failed={len(failed)}"
    )
    print("RESULT:", "pass" if cases and not failed else "fail")

    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if cases and not failed else 1


if __name__ == "__main__":
    sys.exit(main())
