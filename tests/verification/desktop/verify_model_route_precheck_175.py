#!/usr/bin/env python3
"""OD-ARCH-175 阶段 3 · 切换预检：情形 1/2/3 判定与 UI 文案。

证据等级：文件事实（源码）+ 脚本运行（TypeScript 转译后在 Node 中调用
desktop/src/api.ts 的真实 setModelRoute，用 mock invoke 记录实际下发的档位）。
边界：不启动引擎、不读凭证、不接触真实业务材料；只使用一次性 /tmp 目录。

用法：python3 verify_model_route_precheck_175.py
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
DESKTOP = REPO / "desktop"
API = DESKTOP / "src" / "api.ts"
ADMISSION = DESKTOP / "src" / "engineAdmission.ts"
HOME = DESKTOP / "src" / "shell" / "HomeChat.tsx"
PICKER = DESKTOP / "src" / "shell" / "ModelPicker.tsx"
TYPES = DESKTOP / "src" / "types.ts"
TSC_BIN = DESKTOP / "node_modules" / ".bin" / "tsc"
EVIDENCE = HERE / "evidence" / "od-arch-175"

UNDECLARED = "该模型未声明思考档位，若不支持将自动回退默认档。"
FALLBACK = "已按默认思考等级切换（该模型不支持所选档位）。"

CORE_STUB = r'''
let handler = async (cmd) => {
  throw new Error("unexpected invoke: " + cmd);
};
export function setInvokeHandler(fn) {
  handler = fn;
}
export async function invoke(cmd, args) {
  return handler(cmd, args);
}
'''

EVENT_STUB = r'''
export async function listen() {
  return () => {};
}
'''

HARNESS = r'''
import { shell, invalidateEngineFactsCache } from "./api.mjs";
import { setInvokeHandler } from "./tauri-core.mjs";

const state = { catalog: [], routes: defaultRoutes(), calls: [] };

function entry(efforts) {
  const item = {
    id: "flash",
    name: "Flash",
    description: "",
    provider: "acme",
    provider_name: "Acme",
    default_effort: null,
  };
  if (efforts !== "missing") item.efforts = efforts;
  return item;
}

function routesWith(reasoningEfforts) {
  const modelEntry = { id: "flash" };
  if (reasoningEfforts !== "missing") modelEntry.reasoningEfforts = reasoningEfforts;
  return {
    ok: true,
    engine_profile: "discipline",
    overlay: "",
    profiles: [
      {
        profile: "discipline",
        default: null,
        providers: [
          {
            id: "acme",
            displayName: "Acme",
            api: null,
            apiKeyEnv: null,
            baseURL: null,
            models: ["flash"],
            modelEntries: [modelEntry],
          },
        ],
      },
    ],
  };
}

function defaultRoutes() {
  return routesWith("missing");
}

setInvokeHandler(async (cmd, args) => {
  state.calls.push({ cmd: cmd, args: args });
  if (cmd === "engine_list_models") {
    if (state.catalog === "error") throw new Error("engine catalog unavailable");
    return {
      channel: "web-api",
      default: {},
      routable_providers: [],
      groups: [],
      models: state.catalog,
      failures: [],
    };
  }
  if (cmd === "task_list_model_routes") {
    if (state.routes === "error") throw new Error("route listing unavailable");
    return state.routes;
  }
  if (cmd === "task_set_model_route") {
    return {
      ok: true,
      route: {
        provider: args.provider,
        model: args.model,
        endpoint: "https://synthetic.invalid/v1",
        category: "synthetic",
      },
      note: "synthetic switch",
    };
  }
  throw new Error("unexpected command: " + cmd);
});

async function runCase(spec) {
  state.catalog = spec.catalog;
  state.routes = spec.routes;
  state.calls = [];
  invalidateEngineFactsCache();
  try {
    const result = await shell.setModelRoute({
      sourceProfile: "discipline",
      provider: "acme",
      model: "flash",
      reasoningEffort: spec.requested,
    });
    return {
      name: spec.name,
      threw: false,
      result: result,
      calls: state.calls,
    };
  } catch (error) {
    return {
      name: spec.name,
      threw: true,
      error: error && error.message ? error.message : String(error),
      calls: state.calls,
    };
  }
}

const cases = [
  {
    name: "case1_not_listed",
    catalog: [],
    routes: defaultRoutes(),
    requested: "high",
  },
  {
    name: "case1_catalog_unavailable",
    catalog: "error",
    routes: defaultRoutes(),
    requested: "high",
  },
  {
    name: "case1_both_unavailable",
    catalog: "error",
    routes: "error",
    requested: "high",
  },
  {
    name: "case1_efforts_missing",
    catalog: [entry("missing")],
    routes: defaultRoutes(),
    requested: "high",
  },
  {
    name: "case1_catalog_empty_profile_absent",
    catalog: [entry([])],
    routes: defaultRoutes(),
    requested: "high",
  },
  {
    name: "case2_catalog_match",
    catalog: [entry([{ id: "off" }, { id: "low" }, { id: "medium" }, { id: "high" }])],
    routes: defaultRoutes(),
    requested: "high",
  },
  {
    name: "case2_profile_match_when_catalog_empty",
    catalog: [entry([])],
    routes: routesWith({ off: null, high: "high" }),
    requested: "high",
  },
  {
    name: "case3_catalog_not_in_set",
    catalog: [entry([{ id: "off" }, { id: "high" }])],
    routes: defaultRoutes(),
    requested: "medium",
  },
  {
    name: "case3_profile_false",
    catalog: [entry([])],
    routes: routesWith(false),
    requested: "high",
  },
  {
    name: "case3_profile_empty_object",
    catalog: [entry([])],
    routes: routesWith({}),
    requested: "high",
  },
  {
    name: "case3_not_listed_profile_levels",
    catalog: [],
    routes: routesWith({ off: null, high: "high" }),
    requested: "medium",
  },
  {
    name: "default_no_precheck",
    catalog: "error",
    routes: "error",
    requested: "default",
  },
];

const results = [];
for (const spec of cases) {
  results.push(await runCase(spec));
}
process.stdout.write(JSON.stringify(results));
'''

EXPECT = {
    "case1_not_listed": ("high", UNDECLARED, 1, 1),
    "case1_catalog_unavailable": ("high", UNDECLARED, 1, 1),
    "case1_both_unavailable": ("high", UNDECLARED, 1, 1),
    "case1_efforts_missing": ("high", UNDECLARED, 1, 1),
    "case1_catalog_empty_profile_absent": ("high", UNDECLARED, 1, 1),
    "case2_catalog_match": ("high", None, 1, 0),
    "case2_profile_match_when_catalog_empty": ("high", None, 1, 1),
    "case3_catalog_not_in_set": ("default", FALLBACK, 1, 0),
    "case3_profile_false": ("default", FALLBACK, 1, 1),
    "case3_profile_empty_object": ("default", FALLBACK, 1, 1),
    "case3_not_listed_profile_levels": ("default", FALLBACK, 1, 1),
    "default_no_precheck": ("default", None, 0, 0),
}


def call_count(case: dict, command: str) -> int:
    return sum(1 for c in case.get("calls", []) if c.get("cmd") == command)


def static_checks() -> tuple[dict[str, bool], list[str]]:
    api = API.read_text(encoding="utf-8")
    home = HOME.read_text(encoding="utf-8")
    picker = PICKER.read_text(encoding="utf-8")
    types = TYPES.read_text(encoding="utf-8")
    start = api.index("setModelRoute(input")
    end = api.index("listEngineModels(refresh", start)
    body = api[start:end]
    checks = {
        "api_removes_blocking_throw": "模型路由未改动" not in body
        and "请选「跟随模型默认」后再切换" not in body,
        "api_uses_policy": "planModelRouteEffort(evidence, requested)" in body,
        "api_submits_decision": "reasoningEffort: decision.submit" in body,
        "api_returns_advisory": "advisory: decision.advisory" in body,
        "api_returns_effective_effort": "reasoningEffort: decision.submit" in body,
        "api_advisories_present": UNDECLARED in api and FALLBACK in api,
        "home_consumes_advisory": "r.advisory" in home and "setModelNote(r.advisory" in home,
        "home_syncs_effective_effort": "setModelEffort(r.reasoningEffort)" in home,
        "picker_keeps_advisory_slot": 'data-testid="effort-advisory"' in picker,
        "picker_uses_one_line_copy": "未声明思考档位；若不支持会按默认档切换。" in picker
        and "该模型不支持「" in picker,
        "types_efforts_optional": "efforts?: ModelCatalogEffort[] | null;" in types,
    }
    failures = [name for name, ok in checks.items() if not ok]
    return checks, failures


def main() -> int:
    if not TSC_BIN.exists():
        raise SystemExit(f"缺少 TypeScript：{TSC_BIN}")
    tmp = Path(tempfile.mkdtemp(prefix="od175-verify-"))
    out = tmp / "out"
    out.mkdir()
    report: dict = {
        "plan": "OD-ARCH-175 阶段 3",
        "evidence_level": "文件事实 + 脚本运行（真实 api.ts 转译后 Node 调用 + mock invoke）",
        "cases": {},
        "checks": {},
        "failures": [],
    }
    try:
        proc = subprocess.run(
            [
                str(TSC_BIN),
                str(API),
                str(ADMISSION),
                "--ignoreConfig",
                "--outDir",
                str(out),
                "--module",
                "esnext",
                "--target",
                "es2022",
                "--moduleResolution",
                "bundler",
                "--skipLibCheck",
            ],
            cwd=DESKTOP,
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            print(proc.stdout)
            print(proc.stderr, file=sys.stderr)
            return 1
        for name in ("api", "engineAdmission", "types"):
            js = out / f"{name}.js"
            if js.exists():
                js.rename(out / f"{name}.mjs")
        api_js = out / "api.mjs"
        api_text = api_js.read_text(encoding="utf-8")
        api_text = api_text.replace(
            'from "@tauri-apps/api/core"', 'from "./tauri-core.mjs"'
        )
        api_text = api_text.replace(
            'from "@tauri-apps/api/event"', 'from "./tauri-event.mjs"'
        )
        api_text = api_text.replace(
            'from "./engineAdmission"', 'from "./engineAdmission.mjs"'
        )
        api_js.write_text(api_text, encoding="utf-8")
        (out / "tauri-core.mjs").write_text(CORE_STUB, encoding="utf-8")
        (out / "tauri-event.mjs").write_text(EVENT_STUB, encoding="utf-8")
        (out / "harness.mjs").write_text(HARNESS, encoding="utf-8")
        proc = subprocess.run(
            ["node", "harness.mjs"],
            cwd=out,
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            print(proc.stdout)
            print(proc.stderr, file=sys.stderr)
            return 1
        results = json.loads(proc.stdout)
        for case in results:
            name = case["name"]
            submit, advisory, engines, routes = EXPECT[name]
            checks = {
                "did_not_throw": case["threw"] is False,
                "effective_effort": case.get("result", {}).get("reasoningEffort") == submit,
                "advisory": case.get("result", {}).get("advisory") == advisory,
                "submitted_effort": any(
                    c.get("cmd") == "task_set_model_route"
                    and c.get("args", {}).get("reasoningEffort") == submit
                    for c in case.get("calls", [])
                ),
                "engine_catalog_calls": call_count(case, "engine_list_models") == engines,
                "profile_route_calls": call_count(case, "task_list_model_routes") == routes,
            }
            report["cases"][name] = {
                "expected": {
                    "submit": submit,
                    "advisory": advisory,
                    "engine_list_models": engines,
                    "task_list_model_routes": routes,
                },
                "actual": {
                    "threw": case["threw"],
                    "result": case.get("result"),
                    "calls": case.get("calls"),
                },
                "checks": checks,
            }
            for check, ok in checks.items():
                if not ok:
                    report["failures"].append(f"{name}:{check}")
        checks, failures = static_checks()
        report["checks"] = checks
        for failure in failures:
            report["failures"].append(f"static:{failure}")
        EVIDENCE.mkdir(parents=True, exist_ok=True)
        evidence_path = EVIDENCE / "precheck.json"
        evidence_path.write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        case_pass = sum(
            1
            for entry in report["cases"].values()
            if all(entry["checks"].values())
        )
        print(
            f"cases {case_pass}/{len(report['cases'])}; "
            f"static {len(checks) - len(failures)}/{len(checks)}"
        )
        print(f"evidence: {evidence_path}")
        if report["failures"]:
            for failure in report["failures"]:
                print(f"FAIL {failure}", file=sys.stderr)
            return 1
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
