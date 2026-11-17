#!/usr/bin/env python3
"""OD-ARCH-176：工作记录场景意图分类、落盘决策与 off 档位预判。

证据等级：
  * 文件事实 / 静态检查：提交链确实带上档位，快路径已移除；
  * 脚本运行：编译真实 desktop/src 的判定模块与提示词组装器，喂 mock 模型输出断言分流。

不调用模型、不开窗口。真实模型端到端由 ARCHITECT 在阶段 3 组织。

用法：python3 tests/verification/desktop/verify_arch_176.py [--json PATH]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
SRC = DESKTOP / "src"
ENGINE = REPO / "engine" / "omnidesk_engine"


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


PROBE = r"""
const dec = require(process.argv[2]);
const prompt = require(process.argv[3]);
const wi = require(process.argv[4]);
const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}
function wrap(obj) {
  return "<omnidesk-intent>" + JSON.stringify(obj) + "</omnidesk-intent>";
}

// 1) mock 模型输出 → 统一契约解析与分流。
const noteOut = wrap({
  intent: "note",
  record: { append: [{ text: "下午在古田干部学院听课", date: "2026-11-17" }], move: [], merge: [] },
  review: { errors: [], missing: ["地点"], task: { suggest: false, title: "" }, question: "" },
  reply_hint: "已记到 2026-11-17。",
});
const parsedNote = dec.parseWorkIntentContract(noteOut);
check("parse_note_ok", parsedNote.contract && parsedNote.contract.intent === "note", parsedNote.contract && parsedNote.contract.intent);
check("parse_note_record", parsedNote.contract && parsedNote.contract.record && parsedNote.contract.record.append.length === 1);
check("parse_note_review_inline", parsedNote.contract && parsedNote.contract.review && parsedNote.contract.review.missing[0] === "地点");
check("parse_note_reply", parsedNote.contract && parsedNote.contract.reply === "已记到 2026-11-17。");
check("note_route_writes", dec.decideWorkIntentRoute("note").action === "note" && dec.decideWorkIntentRoute("note").writes);

const instrOut = wrap({ intent: "instruction", reply_hint: "我去查一下本周记录。" });
const parsedInstr = dec.parseWorkIntentContract(instrOut);
check("parse_instruction_ok", parsedInstr.contract && parsedInstr.contract.intent === "instruction");
check("instruction_route_no_write", dec.decideWorkIntentRoute("instruction").action === "reply" && !dec.decideWorkIntentRoute("instruction").writes);

const chatOut = wrap({ intent: "chat", reply_hint: "不客气。" });
check("chat_route_no_write", dec.parseWorkIntentContract(chatOut).contract.intent === "chat" && !dec.decideWorkIntentRoute("chat").writes);

const reviewOut = wrap({ intent: "period-review", period: "monthly", reply_hint: "正在汇集本月材料。" });
const pr = dec.parseWorkIntentContract(reviewOut).contract;
check("period_review_route", dec.decideWorkIntentRoute("period-review").action === "period-review");
check("period_review_period", pr && pr.period === "monthly", pr && pr.period);

const reportOut = wrap({ intent: "report", reply_hint: "正在带本周记录。" });
check("report_route", dec.decideWorkIntentRoute("report").action === "report");
const summaryOut = wrap({ intent: "daily-summary", reply_hint: "正在带今天的记录。" });
check("daily_summary_route", dec.decideWorkIntentRoute("daily-summary").action === "daily-summary");

const uncertainOut = wrap({ intent: "uncertain", reply_hint: "这句拿不准。" });
check("uncertain_route_asks", dec.decideWorkIntentRoute(dec.parseWorkIntentContract(uncertainOut).contract.intent).action === "ask");
check("unknown_intent_asks", dec.decideWorkIntentRoute(dec.parseWorkIntentContract(wrap({ intent: "???" })).contract.intent).action === "ask");
check("bad_json_null", dec.parseWorkIntentContract("<omnidesk-intent>{oops}</omnidesk-intent>").contract === null);
check("bad_record_null", dec.parseWorkIntentContract(wrap({ intent: "note", record: { append: "x" } })).contract.record === null);

const fenced = "<omnidesk-intent>" + String.fromCharCode(96).repeat(3) + "json\n" +
  JSON.stringify({ intent: "chat", reply_hint: "好。" }) + "\n" + String.fromCharCode(96).repeat(3) + "</omnidesk-intent>";
check("fenced_json_ok", dec.parseWorkIntentContract(fenced).contract.intent === "chat");

// 2) 思考档位：档位必须被证据「证明支持」才下发（真实 DSH 的 session/selectModel
// 会拒绝未声明模型的任何显式档位，含 off——OD-ARCH-176 线上回归）；
// 未声明模型本就没有思考可关，不传档位即等效直出；起草不传；用户手动档位优先。
const pick = dec.pickWorkIntentEffort;
check("effort_no_evidence_drops_tier", pick({ intent: "classify", userSelected: false, selectedEffort: "off" }) === null);
check("effort_instruction_no_evidence_drops_tier", pick({ intent: "instruction", userSelected: false, selectedEffort: "high" }) === null);
check("effort_note_no_evidence_drops_tier", pick({ intent: "note", userSelected: false, selectedEffort: "high" }) === null);
check("effort_report_default", pick({ intent: "report", userSelected: false, selectedEffort: "high" }) === null);
check("effort_period_default", pick({ intent: "period-review", userSelected: false, selectedEffort: "high" }) === null);
check("effort_manual_high_without_evidence_drops_tier", pick({ intent: "classify", userSelected: true, selectedEffort: "high" }) === null);
check("effort_manual_high_with_evidence", pick({
  intent: "classify", userSelected: true, selectedEffort: "high",
  evidence: { catalogLevels: ["low", "high"], profileLevels: null },
}) === "high");
check("effort_manual_default", pick({ intent: "classify", userSelected: true, selectedEffort: "default" }) === null);

// 2a) off 能力预判：声明集合包含 off 才下发 off；未声明/目录不可用一律 null。
check(
  "effort_catalog_includes_off",
  pick({
    intent: "classify", userSelected: false, selectedEffort: "off",
    evidence: { catalogLevels: ["off", "low", "high"], profileLevels: null },
  }) === "off",
);
check(
  "effort_catalog_missing_off",
  pick({
    intent: "classify", userSelected: false, selectedEffort: "off",
    evidence: { catalogLevels: ["low", "high"], profileLevels: null },
  }) === null,
);
check(
  "effort_undeclared_drops_tier",
  pick({
    intent: "classify", userSelected: false, selectedEffort: "off",
    evidence: { catalogLevels: [], profileLevels: null },
  }) === null,
);
check(
  "effort_catalog_unavailable_drops_tier",
  pick({ intent: "classify", userSelected: false, selectedEffort: "off", evidence: null }) === null,
);
check(
  "effort_profile_includes_off",
  pick({
    intent: "classify", userSelected: false, selectedEffort: "off",
    evidence: { catalogLevels: null, profileLevels: ["off", "high"] },
  }) === "off",
);
check(
  "effort_profile_missing_off",
  pick({
    intent: "classify", userSelected: false, selectedEffort: "off",
    evidence: { catalogLevels: [], profileLevels: ["low", "high"] },
  }) === null,
);
check(
  "effort_profile_false_drops_tier",
  pick({
    intent: "classify", userSelected: false, selectedEffort: "off",
    evidence: { catalogLevels: [], profileLevels: false },
  }) === null,
);
check(
  "effort_manual_off_missing_is_default",
  pick({
    intent: "classify", userSelected: true, selectedEffort: "off",
    evidence: { catalogLevels: ["low", "high"], profileLevels: null },
  }) === null,
);

// 2b) 模型判出的周期 → 宿主按完整自然周期取范围。
const refDay = new Date(Date.UTC(2026, 10, 17));
const monthly = wi.periodReviewTargetForPeriod("monthly", refDay, 480);
check("period_monthly_range", monthly.start === "2026-11-01" && monthly.end === "2026-11-30", monthly.start + "~" + monthly.end);
const quarter = wi.periodReviewTargetForPeriod("quarterly", refDay, 480);
check("period_quarter_range", quarter.start === "2026-10-01" && quarter.end === "2026-12-31");
const weekly = wi.periodReviewTargetForPeriod("weekly", refDay, 480);
check("period_weekly_range", weekly.start === "2026-11-16" && weekly.end === "2026-11-22", weekly.start + "~" + weekly.end);

// 3) 分类提示词：带本周索引、统一契约与「指令不落盘」硬约束。
const skill = { id: "memory-agent", content: "---\nname: memory-agent\ndescription: 判定工作记录\n---\n正文" };
const preset = { key: "weekly", label: "工作记录", rules: null, sections: null, skills: [skill], skillsByFlow: { note: [skill] } };
const text = prompt.composeConversationPrompt({
  preset,
  turns: [],
  current: "你自己去搜索一下本周的记录",
  mode: "classify",
  noteIndex: { week: "2026-W47", today: "2026-11-17", entries: "2026-11-17 #0 开会" },
});
check("prompt_has_contract", text.includes(dec.INTENT_OPEN) && text.includes(dec.INTENT_CLOSE));
check("prompt_has_index", text.includes("2026-W47") && text.includes("2026-11-17 #0 开会"));
check("prompt_instruction_not_record", text.includes("指令") && text.includes("绝不能写进工作记录"));
check("prompt_note_review_inline", text.includes("review") && text.includes("同一调用完成质量分析"));
check("prompt_no_write_claim", text.includes("不要声称已经写入"));
check("prompt_skill_pointer", text.includes("memory-agent"));

process.stdout.write(JSON.stringify({ cases }) + "\n");
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": str(detail)})

    home = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    decision = (SRC / "workIntentDecision.ts").read_text(encoding="utf-8")
    prompt_src = (SRC / "conversationPrompt.ts").read_text(encoding="utf-8")
    lib = (DESKTOP / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    serve = (ENGINE / "serve.py").read_text(encoding="utf-8")
    adapter = (ENGINE / "web_api_adapter.py").read_text(encoding="utf-8")
    core = (ENGINE / "task_core.py").read_text(encoding="utf-8")
    period_skill = (REPO / "skills" / "period-review" / "SKILL.md").read_text(encoding="utf-8")

    # ---- 静态：快路径确实移除，分类入口确实存在 ----
    check("home_no_statement_fastpath", "isPlainStatementNote(text)" not in home)
    check("home_classify_mode", 'mode: "classify"' in home)
    check("home_settles_intent", "settleWorkIntent" in home and "parseWorkIntentContract" in home)
    check("home_instruction_no_write", "这句没有写入工作记录" in home)
    check("home_note_confirmation", "已记录到 " in home and "formatRecordReviewLines" in home)
    check("home_review_inline", "contract?.review" in home and "analyzeRecordReview" not in home)
    check("home_asks_when_unsure", "这条要记吗？" in home and "answerNoteAsk" in home)
    check("home_period_inline", "worklogPeriodReviewCollect" in home and "submitRoutedWorkIntent" in home)
    check("home_reuses_period_settle", "periodPendingRef" in home)
    check("home_effort_chain", "reasoningEffort: effort" in home and "provider: factsNow?.provider" in home)
    check(
        "home_off_effort_preflight",
        "effortEvidenceFromLoaded" in home and "evidence: effortEvidenceFromLoaded" in home,
    )
    check(
        "home_off_effort_two_layer_evidence",
        "catalogEntry?.efforts" in home and "modelEntry?.reasoningEfforts" in home,
    )
    check(
        "home_off_effort_all_calls_routed",
        home.count("pickRoutedWorkIntentEffort(") >= 4
        and home.count("pickWorkIntentEffort({") == 1,
        f"routed={home.count('pickRoutedWorkIntentEffort(')} direct={home.count('pickWorkIntentEffort({')}",
    )
    check(
        "decision_off_evidence_gate",
        "effortEvidenceExcludes" in decision
        and "evidence?: WorkIntentEffortEvidence" in decision,
    )
    check(
        "period_review_inline_draft",
        "收到随请求送达的汇集材料时直接按对应深度模板起草" in period_skill
        and "不回复「等待材料」类文案" in period_skill,
    )
    check(
        "period_review_empty_honest",
        "材料缺失或为空时如实说明" in period_skill and "不要自己找文件" in period_skill,
    )
    check(
        "period_review_hard_constraints_kept",
        "案件材料、线索、未公开审查调查信息" in period_skill
        and "已确认的复盘不允许被重新生成覆盖" in period_skill,
    )

    # ---- 静态：桌面 → Rust → engine 管线 ----
    check("api_forwards_effort", "reasoningEffort: input.reasoningEffort ?? null" in api)
    check("api_forwards_route", "provider: input.provider ?? null" in api and "model: input.model ?? null" in api)
    check("rust_accepts_effort", "reasoning_effort: Option<String>" in lib and 'params["reasoning_effort"]' in lib)
    check("rust_forwards_route", 'params["provider"]' in lib and 'params["model"]' in lib)
    check("serve_forwards_effort", "reasoning_effort=params.get(\"reasoning_effort\")" in serve)
    check("core_has_effort_field", "reasoning_effort: Optional[str] = None" in core)
    check("adapter_applies_before_prompt", adapter.index("_apply_reasoning_effort(session_id, request)") < adapter.index("worker.start()"))
    check("adapter_uses_select_model", "self.select_model(" in adapter and "session/selectModel" in adapter)
    check("adapter_no_restart", "stop_engine" not in adapter and "restart" not in adapter.lower())
    check("decision_pure", "from ." not in decision and "require(" not in decision)
    check("prompt_classify_block", 'input.mode === "classify"' in prompt_src)

    # ---- 运行：编译真实 TS 并喂 mock 分类输出 ----
    tmp = Path(tempfile.mkdtemp(prefix="od176-"))
    compiled = run(
        [
            "npx", "--no-install", "tsc",
            str(SRC / "workIntent.ts"), str(SRC / "workIntentDecision.ts"), str(SRC / "conversationPrompt.ts"),
            "--outDir", str(tmp / "out"), "--module", "commonjs",
            "--target", "ES2022", "--skipLibCheck", "--ignoreConfig",
        ],
        DESKTOP,
    )
    if compiled.returncode != 0:
        check("ts_compiles", False, compiled.stderr[-1500:])
    else:
        check("ts_compiles", True)
        probe = tmp / "probe.js"
        probe.write_text(PROBE, encoding="utf-8")
        proc = run(
            [
                "node", str(probe),
                str(tmp / "out" / "workIntentDecision.js"),
                str(tmp / "out" / "conversationPrompt.js"),
                str(tmp / "out" / "workIntent.js"),
            ],
            DESKTOP,
        )
        if proc.returncode != 0:
            check("probe_exit", False, proc.stderr[-1500:])
        else:
            cases.extend(json.loads(proc.stdout.strip().splitlines()[-1])["cases"])

    failed = [c for c in cases if not c["ok"]]
    for c in cases:
        print(("PASS " if c["ok"] else "FAIL ") + "[ARCH-176] " + c["name"] +
              ((" :: " + c["detail"]) if (not c["ok"] and c["detail"]) else ""))
    print(f"cases={len(cases)} passed={len(cases) - len(failed)} failed={len(failed)}")

    if args.json:
        Path(args.json).write_text(
            json.dumps({"cases": cases, "passed": not failed}, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
