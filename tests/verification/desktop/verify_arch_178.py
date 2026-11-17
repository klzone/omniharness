#!/usr/bin/env python3
"""OD-ARCH-178 阶段 B+C：全路径档位证据门与 ModelPicker 目录驱动。

证据等级：
  * 文件事实 / 静态检查：4 个 HomeChat 提交站点都带证据；其余 submitTask
    站点不写 reasoningEffort；api 层有最后一道门；选择器按声明集合渲染。
  * 脚本运行：编译真实 desktop/src 的档位解析模块，在 Node 中断言
    未声明/声明含 off/目录优先/Profile 回退/按钮集合/提交门禁。

不调用模型、不开窗口、不读凭证。

用法：python3 tests/verification/desktop/verify_arch_178.py [--json PATH]
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
EVIDENCE = HERE / "evidence" / "od-arch-178"


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, timeout=300)


PROBE = r"""
const ev = require(process.argv[2]);
const dec = require(process.argv[3]);

const cases = [];
function check(name, ok, detail) {
  cases.push({ name, ok: !!ok, detail: detail === undefined ? null : String(detail) });
}

function catalog(efforts, id = "flash") {
  const entry = {
    id,
    name: "Flash",
    description: "",
    provider: "acme",
    provider_name: "Acme",
    default_effort: null,
  };
  if (efforts !== undefined) {
    entry.efforts = efforts.map((x) => ({ id: x, name: x, description: "" }));
  }
  return {
    available: true,
    channel: "web-api",
    groups: [],
    models: [entry],
  };
}

function routes(declaration) {
  const modelEntry = { id: "flash" };
  if (declaration !== undefined) modelEntry.reasoningEfforts = declaration;
  return [
    {
      profile: "discipline",
      default: null,
      providers: [{ id: "acme", displayName: "Acme", models: ["flash"], modelEntries: [modelEntry] }],
    },
  ];
}

function resolve(overrides = {}) {
  return ev.resolveModelEffortDeclaration({
    catalog: catalog([]),
    routes: routes(undefined),
    sourceProfile: "discipline",
    provider: "acme",
    model: "flash",
    ...overrides,
  });
}

// 1) 未声明：目录空、Profile 缺省 → 空集合。
const undeclared = resolve();
check("undeclared_levels_empty", undeclared.levels.length === 0, undeclared.levels.join(","));
check("undeclared_source_null", undeclared.source === null, undeclared.source);
const undeclaredOptions = ev.modelEffortOptions(undeclared.levels);
check(
  "undeclared_only_default_button",
  undeclaredOptions.length === 1 &&
    undeclaredOptions[0].key === "default" &&
    undeclaredOptions[0].label === "跟随模型默认",
  JSON.stringify(undeclaredOptions),
);

// 2) 目录声明含 off → 渲染 off 按钮，默认按钮仍在。
const withOff = resolve({ catalog: catalog(["off", "low", "medium", "high"]) });
check("catalog_off_levels", withOff.levels.includes("off") && withOff.source === "catalog", withOff.source);
const offOptions = ev.modelEffortOptions(withOff.levels);
check("catalog_off_button", offOptions.some((o) => o.key === "off" && o.label === "关闭"), JSON.stringify(offOptions));
check("catalog_default_button_first", offOptions[0].key === "default" && offOptions[0].label === "跟随模型默认");

// 3) 目录优先：目录非空时不看 Profile 的不同声明。
const priority = resolve({
  catalog: catalog(["low", "high"]),
  routes: routes({ off: "none", low: "low", high: "high" }),
});
check("catalog_wins", priority.levels.join(",") === "low,high" && priority.source === "catalog", priority.levels.join(","));

// 4) 目录为空时回退 Profile 原始声明；false/空对象保持空集合。
const profileFallback = resolve({ catalog: catalog([]), routes: routes({ off: "none", high: "high" }) });
check("profile_fallback", profileFallback.levels.join(",") === "off,high" && profileFallback.source === "profile", profileFallback.levels.join(","));
const profileFalse = resolve({ catalog: catalog([]), routes: routes(false) });
check("profile_false_empty", profileFalse.levels.length === 0);
const profileEmpty = resolve({ catalog: catalog([]), routes: routes({}) });
check("profile_empty_empty", profileEmpty.levels.length === 0);

// 5) 未知声明档位保留 id，已知档位用中文标签。
const custom = ev.modelEffortOptions(["minimal", "max", "mystery"]);
check("custom_labels", custom.some((o) => o.key === "minimal" && o.label === "最低") && custom.some((o) => o.key === "max" && o.label === "最高") && custom.some((o) => o.key === "mystery" && o.label === "mystery"), JSON.stringify(custom));

// 6) 提交门禁：只有证据明确支持才下发；未声明 / 无证据 / default 一律 null。
check("gate_default_null", ev.gatedReasoningEffort("default", { catalogLevels: ["off"] }) === null);
check("gate_no_evidence_null", ev.gatedReasoningEffort("off", null) === null);
check("gate_undeclared_null", ev.gatedReasoningEffort("off", { catalogLevels: [], profileLevels: null }) === null);
check("gate_off_declared", ev.gatedReasoningEffort("off", { catalogLevels: ["off", "low"] }) === "off");
check("gate_profile_off", ev.gatedReasoningEffort("off", { catalogLevels: [], profileLevels: ["off"] }) === "off");
check("gate_missing_tier_null", ev.gatedReasoningEffort("high", { catalogLevels: ["off", "low"] }) === null);
check("gate_profile_false_null", ev.gatedReasoningEffort("high", { catalogLevels: [], profileLevels: false }) === null);

// 7) 目录/Profile 的证据与选择器口径一致。
const loadedEvidence = ev.effortEvidenceFromLoaded({
  catalog: catalog(["off", "low"]),
  routes: routes({ off: "none" }),
  sourceProfile: "discipline",
  provider: "acme",
  model: "flash",
});
check("evidence_catalog_short_circuit", loadedEvidence.catalogLevels.join(",") === "off,low" && loadedEvidence.profileLevels === null);
check("evidence_agrees_with_gate", dec.effortEvidenceIncludes(loadedEvidence, "off") && !dec.effortEvidenceIncludes(loadedEvidence, "high"));

// 8) 普通对话也走同一意图策略：用户选中档位需证据；未手动选但有 off 声明时直出。
const pick = dec.pickWorkIntentEffort;
check("chat_selected_off_no_evidence", pick({ intent: "chat", userSelected: true, selectedEffort: "off" }) === null);
check("chat_selected_off_with_evidence", pick({ intent: "chat", userSelected: true, selectedEffort: "off", evidence: { catalogLevels: ["off"] } }) === "off");
check("chat_default_off_with_evidence", pick({ intent: "chat", userSelected: false, selectedEffort: "default", evidence: { catalogLevels: ["off"] } }) === "off");
check("chat_default_no_evidence", pick({ intent: "chat", userSelected: false, selectedEffort: "default" }) === null);

process.stdout.write(JSON.stringify({ cases }) + "\n");
"""


def static_checks() -> tuple[list[dict], list[str]]:
    home = (SRC / "shell" / "HomeChat.tsx").read_text(encoding="utf-8")
    api = (SRC / "api.ts").read_text(encoding="utf-8")
    picker = (SRC / "shell" / "ModelPicker.tsx").read_text(encoding="utf-8")
    evidence = (SRC / "modelEffortEvidence.ts").read_text(encoding="utf-8")
    other_submit = [
        SRC / "shell" / "WritingWorkbench.tsx",
        SRC / "shell" / "ReviewArchive.tsx",
        SRC / "shell" / "WritingOutlineEditor.tsx",
        SRC / "shell" / "KnowledgeIngestPanel.tsx",
        SRC / "shell" / "WritingSectionEditor.tsx",
        SRC / "shell" / "WorkRecordLibrary.tsx",
        SRC / "aiDraft.ts",
    ]
    cases: list[dict] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        cases.append({"name": name, "ok": bool(ok), "detail": str(detail) if not ok else ""})

    check("home_submit_sites_count", home.count("host.submitTask(") == 4, str(home.count("host.submitTask(")))
    check("home_all_submit_sites_have_evidence", home.count("effortEvidence: currentEffortEvidence()") == 4, str(home.count("effortEvidence: currentEffortEvidence()")))
    check("home_analyze_site_gated", home.count('reasoningEffort: pickRoutedWorkIntentEffort("analyze")') == 1)
    check("home_routed_sites_gated", home.count("reasoningEffort: effort,") == 2)
    check(
        "home_chat_site_gated",
        home.count("reasoningEffort: routedEffort") == 1
        and 'pickRoutedWorkIntentEffort(intent ?? "chat")' in home,
    )
    check("home_initial_effort_follow_default", 'const [modelEffort, setModelEffort] = useState("default")' in home)
    check("home_fallback_on_unsupported", 'if (droppedEffort) setModelEffort("default")' in home and "gatedReasoningEffort(pickedEffort, targetEvidence)" in home)
    check("home_catalog_fallback_effect", "resolveModelEffortDeclaration({" in home and "if (declaration.levels.includes(modelEffort)) return;" in home)
    check("home_effort_evidence_shared", "effortEvidenceFromLoaded" in home and "catalogEntry?.efforts" in evidence and "modelEntry?.reasoningEfforts" in evidence)

    check("api_gates_submit_effort", "gatedReasoningEffort(input.reasoningEffort, input.effortEvidence)" in api)
    check("api_has_no_ungated_forward", "reasoningEffort: input.reasoningEffort ?? null" not in api)
    check("api_declares_evidence_param", "effortEvidence?: WorkIntentEffortEvidence | null;" in api)
    check("api_route_precheck_still_uses_two_layers", "planModelRouteEffort(evidence, requested)" in api and "effortEvidenceFromLoaded({" in api)

    leaked = [p.name for p in other_submit if "reasoningEffort" in p.read_text(encoding="utf-8")]
    check("other_submit_sites_do_not_send_effort", not leaked, ",".join(leaked))

    check("picker_catalog_driven", "resolveModelEffortDeclaration({" in picker and "modelEffortOptions(declaration.levels)" in picker)
    check("picker_maps_declared_buttons", "effortOptions.map((lv) => (" in picker and "EFFORT_LEVELS.map" not in picker)
    check("picker_default_only_when_empty", "该模型未声明思考档位，已跟随模型默认。" in picker)
    check("picker_unsupported_hint", "该模型不支持「" in picker and 'data-testid="effort-advisory"' in picker)
    check("picker_keeps_testid_style", 'data-testid={"effort-option-" + lv.key}' in picker)
    check("effort_helper_pure_default", 'return [{ key: DEFAULT_EFFORT_KEY, label: DEFAULT_EFFORT_LABEL }, ...declared];' in evidence)

    failures = [c["name"] for c in cases if not c["ok"]]
    return cases, failures


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", default=None)
    args = ap.parse_args()

    static_cases, static_failures = static_checks()
    runtime_cases: list[dict] = []
    runtime_error: str | None = None

    tmp = Path(tempfile.mkdtemp(prefix="od178-"))
    compiled = run(
        [
            "npx", "--no-install", "tsc",
            str(SRC / "modelEffortEvidence.ts"),
            str(SRC / "workIntentDecision.ts"),
            str(SRC / "workIntent.ts"),
            str(SRC / "workRecordVerdict.ts"),
            str(SRC / "recordReview.ts"),
            "--outDir", str(tmp / "out"),
            "--module", "commonjs",
            "--target", "ES2022",
            "--skipLibCheck",
            "--ignoreConfig",
        ],
        DESKTOP,
    )
    if compiled.returncode != 0:
        runtime_error = compiled.stderr[-2000:]
    else:
        probe = tmp / "probe.js"
        probe.write_text(PROBE, encoding="utf-8")
        proc = run(
            [
                "node", str(probe),
                str(tmp / "out" / "modelEffortEvidence.js"),
                str(tmp / "out" / "workIntentDecision.js"),
            ],
            DESKTOP,
        )
        if proc.returncode != 0:
            runtime_error = proc.stderr[-2000:]
        else:
            runtime_cases = json.loads(proc.stdout.strip().splitlines()[-1])["cases"]

    all_cases = static_cases + runtime_cases
    failures = [c for c in all_cases if not c["ok"]]
    for case in all_cases:
        print(("PASS " if case["ok"] else "FAIL ") + "[ARCH-178] " + case["name"] +
              ((" :: " + case["detail"]) if not case["ok"] and case["detail"] else ""))
    if runtime_error:
        print("FAIL [ARCH-178] runtime_probe :: " + runtime_error)
    print(
        f"static={sum(1 for c in static_cases if c['ok'])}/{len(static_cases)} "
        f"runtime={sum(1 for c in runtime_cases if c['ok'])}/{len(runtime_cases)} "
        f"failed={len(failures) + (1 if runtime_error else 0)}"
    )

    if args.json or not failures:
        EVIDENCE.mkdir(parents=True, exist_ok=True)
        out = Path(args.json) if args.json else EVIDENCE / "verify.json"
        out.write_text(
            json.dumps(
                {
                    "plan": "OD-ARCH-178 阶段 B+C",
                    "evidence_level": "文件事实 + 静态检查 + 编译真实 TS 后在 Node 运行纯函数",
                    "static": static_cases,
                    "runtime": runtime_cases,
                    "runtime_error": runtime_error,
                    "passed": not failures and runtime_error is None,
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        print(f"evidence: {out}")
    return 0 if not failures and runtime_error is None else 1


if __name__ == "__main__":
    raise SystemExit(main())
