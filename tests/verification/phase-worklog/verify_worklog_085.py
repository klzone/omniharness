#!/usr/bin/env python3
"""OD-ARCH-085 合成评测：工作记录全链路（读 / 存 / 取 / 输出 + 知识库两跳检索）。

全部数据为合成样例，运行在 /tmp 隔离副本数据根，不读取也不写入 ~/OmniDesk 的真实内容。

覆盖：一句话记录、多轮讨论、长文要点、周报起草→确认→落盘、建卡防重复、
索引同步、两跳检索命中（断言无整目录读取）、无 Obsidian 降级 + 有 CLI 两条路径。

用法：
  python3 tests/verification/phase-worklog/verify_worklog_085.py            # 跑评测并写证据
  python3 tests/verification/phase-worklog/verify_worklog_085.py --no-write # 只跑不落证据

证据：tests/verification/phase-worklog/evidence/facts.json
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
EVIDENCE = HERE / "evidence"

# 085 开工基线（2026-10-25，HEAD 0d17b1f）：desktop/src 下已有 OD-ARCH-084 未提交改动。
# 085 验收口径是 src-tauri/ 冻结；OD-ARCH-086 获批解冻 src-tauri/ 并改设置页，
# OD-ARCH-088 获批解冻 engine/omnidesk_engine/ 并新增提问卡组件，
# 因此这里改为登记清单，超出清单仍视为越界。
FROZEN_BASELINE = {
    "desktop/src": [
        "desktop/src/App.tsx",
        "desktop/src/SettingsPanel.tsx",
        "desktop/src/api.ts",
        "desktop/src/markdown.css",
        "desktop/src/shell/HomeChat.tsx",
        "desktop/src/shell/MarkdownText.tsx",
        "desktop/src/shell/TopNav.tsx",
        "desktop/src/types.ts",
        # OD-ARCH-088：引擎结构化提问卡。
        "desktop/src/shell/QuestionCard.tsx",
        # OD-ARCH-089：意图由输入判定（取消手动开关）+ 随手记确定性落盘 + 提示词拆分。
        "desktop/src/conversationPrompt.ts",
        "desktop/src/workIntent.ts",
    ],
}
# OD-ARCH-088 获批修改的两个适配层文件；engine/ 其余文件仍视为冻结。
ENGINE_ALLOWED = {
    "engine/omnidesk_engine/serve.py",
    "engine/omnidesk_engine/web_api_adapter.py",
}
SKILLS = REPO / "skills"

sys.path.insert(0, str(HERE))
import worklog_store as ws  # noqa: E402

CASES: list[dict] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CASES.append({"name": name, "ok": bool(ok), "detail": str(detail)})


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fresh_root(tag: str) -> Path:
    root = Path(tempfile.mkdtemp(prefix=f"od085-{tag}-"))
    return root


# ---------------------------------------------------------------------------
# 0. Skill 正文：写死的规则必须真的在文件里（文件事实）
# ---------------------------------------------------------------------------


def check_skill_text() -> None:
    ma = (SKILLS / "memory-agent" / "SKILL.md").read_text(encoding="utf-8")
    ks = (SKILLS / "knowledge-search" / "SKILL.md").read_text(encoding="utf-8")
    # OD-ARCH-089：周报职责从 memory-agent 拆到独立的 memory-report 技能。
    # 周报相关断言改读新文件；随手记相关断言仍读 memory-agent（职责未变）。
    mr = (SKILLS / "memory-report" / "SKILL.md").read_text(encoding="utf-8")

    check("ma_mentions_iso_week_file", "工作记录/2026-Wxx.md" in ma.replace("库/", ""), "")
    check("ma_forbids_rewrite", "禁止整文件重写" in ma or "禁止整文件重写、禁止覆盖" in ma, "")
    check("ma_append_only_stated", "只追加" in ma, "")
    check("ma_destinations_all_four", all(d in ma for d in ws.DESTINATIONS), "")
    check("ma_no_hard_expand", "不硬扩" in ma, "")
    check("ma_discussion_shape", "问题 → 结论" in ma or "问题**：" in ma, "")
    check("ma_long_text_points", "只记要点" in ma, "")
    check("mr_report_confirm_gate", "只有用户明确确认后才落盘" in mr, "")
    check("mr_report_naming", "W<两位周次>-周报.md" in mr, "")
    check("mr_report_sections", all(s in mr for s in ws.REPORT_SECTIONS), "")
    check("mr_no_overwrite_report", "不覆盖" in mr, "")
    # 拆分后随手记侧不得再带周报章节（省 token 的实质）。
    check("ma_has_no_report_sections", not any(s in ma for s in ws.REPORT_SECTIONS), "")
    check("ma_notes_report_moved", "memory-report" in ma, "")

    check("ks_index_first", "索引.md" in ks, "")
    check("ks_two_hops", "两跳" in ks, "")
    check("ks_forbids_dir_scan", ("禁止" in ks and "递归遍历" in ks) or "一律禁止" in ks, "")
    check("ks_one_or_two_cards", "1–2 张卡" in ks, "")
    check("ks_card_300", "≤ 300 字" in ks, "")
    check("ks_frontmatter", all(k in ks for k in ("tags:", "sources:", "date:")), "")
    check("ks_wikilink", "[[" in ks, "")
    check("ks_pending_verify_tag", "待核实" in ks, "")
    check("ks_dedupe_before_write", "先查索引防重复" in ks, "")
    check("ks_index_sync_after_write", "同步索引" in ks, "")
    check("ks_obsidian_two_paths", "Obsidian CLI" in ks and "降级" in ks, "")
    check("ks_one_card_one_concept", "一卡一概念" in ks, "")

    # 注入链路对单个 Skill 正文截断到 2500 字（desktop/src/aiDraft.ts），规则必须全在预算内
    for name, text in (("memory-agent", ma), ("memory-report", mr), ("knowledge-search", ks)):
        check(f"{name}_fits_injection_budget", len(text) <= 2500, f"{len(text)} 字符")


# ---------------------------------------------------------------------------
# 1. 一句话记录：不硬扩，只追加
# ---------------------------------------------------------------------------


def check_one_liner(root: Path) -> None:
    store = ws.WorkLogStore(root, today=dt.date(2026, 10, 25))
    store.ensure_skeleton()
    sentence = "下午跟三组核对了材料清单，还差两份。"
    path = store.append_record(
        "2026-W43",
        dt.date(2026, 10, 21),
        [{"text": sentence, "destination": "inbox", "source_sentence": sentence}],
    )
    text = path.read_text(encoding="utf-8")
    check("one_liner_file_is_weekly", path.name == "2026-W43.md", path.name)
    check("one_liner_text_verbatim", sentence in text, "")
    check("one_liner_single_bullet", text.count("- **事**：") == 1, str(text.count("- **事**：")))
    check("one_liner_has_no_sections", "本周概述" not in text and "事项进展" not in text, "")
    check("one_liner_has_destination", "**去向**：inbox" in text, "")

    # 硬扩必须被拒绝
    try:
        store.append_record("2026-W43", dt.date(2026, 10, 22), [
            {"text": sentence + "，体现了扎实的工作作风，为下一步打牢基础。",
             "destination": "inbox", "source_sentence": sentence}
        ])
        check("one_liner_rejects_expansion", False, "扩写未被拦截")
    except ws.WorkLogError as exc:
        check("one_liner_rejects_expansion", "扩写" in str(exc), str(exc))

    # 缺去向必须被拒绝
    try:
        store.append_record("2026-W43", dt.date(2026, 10, 22), [{"text": "随手一句"}])
        check("entry_requires_destination", False, "缺去向未被拦截")
    except ws.WorkLogError as exc:
        check("entry_requires_destination", "去向" in str(exc), str(exc))

    # 非法去向必须被拒绝
    try:
        store.append_record("2026-W43", dt.date(2026, 10, 22),
                            [{"text": "随手一句", "destination": "随便"}])
        check("entry_rejects_bad_destination", False, "非法去向未被拦截")
    except ws.WorkLogError:
        check("entry_rejects_bad_destination", True, "")


# ---------------------------------------------------------------------------
# 2. 两次追加后文件必须是纯追加结果
# ---------------------------------------------------------------------------


def check_append_only(root: Path) -> None:
    store = ws.WorkLogStore(root, today=dt.date(2026, 10, 25))
    store.ensure_skeleton()
    p = store.append_record("2026-W44", dt.date(2026, 10, 26), [
        {"text": "上午审了两份表格。", "destination": "本周报引用", "source_sentence": "上午审了两份表格。"}
    ])
    first = p.read_bytes()
    digest1 = hashlib.sha256(first).hexdigest()
    p2 = store.append_record("2026-W44", dt.date(2026, 10, 27), [
        {"text": "下午对接了系统方。", "destination": "归档", "source_sentence": "下午对接了系统方。"}
    ])
    second = p2.read_bytes()
    check("append_same_file", p == p2, p.name)
    check("append_keeps_prefix_bytes", second.startswith(first), f"{len(first)} -> {len(second)}")
    check("append_only_grew", len(second) > len(first), "")
    check("append_old_digest_prefix_stable",
          hashlib.sha256(second[:len(first)]).hexdigest() == digest1, "")
    check("append_two_date_sections", second.decode().count("## 2026-10-2") == 2,
          second.decode())
    # 模拟重新打开再追加一次，旧内容仍必须是前缀
    store.append_record("2026-W44", dt.date(2026, 10, 27), [
        {"text": "晚上补了台账。", "destination": "inbox", "source_sentence": "晚上补了台账。"}
    ])
    third = p.read_bytes()
    check("append_third_time_prefix", third.startswith(second), "")
    check("append_same_day_marks_continuation", "2026-10-27（续）" in third.decode(), "")
    # 同日两次追加：一个原节 + 一个「续」节，既有行不重复也不被合并改写
    body = third.decode()
    check("append_dates_not_duplicated",
          body.count("## 2026-10-26") == 1 and body.count("## 2026-10-27\n") == 1, "")
    check("append_entries_all_present",
          all(s in body for s in ("上午审了两份表格", "下午对接了系统方", "晚上补了台账")), "")


# ---------------------------------------------------------------------------
# 3. 多轮讨论：提炼「问题 → 结论」
# ---------------------------------------------------------------------------


def check_discussion(root: Path) -> None:
    store = ws.WorkLogStore(root, today=dt.date(2026, 10, 25))
    store.ensure_skeleton()
    p = store.append_record("2026-W43", dt.date(2026, 10, 23), [
        {"question": "台账是每周汇总还是每月汇总？",
         "conclusion": "定为每月汇总，每月 5 日前报出。",
         "destination": "知识卡"},
    ], mode="discussion")
    text = p.read_text(encoding="utf-8")
    check("discussion_has_question", "**问题**：台账是每周汇总还是每月汇总？" in text, "")
    check("discussion_has_conclusion", "**结论**：定为每月汇总" in text, "")
    check("discussion_has_destination", "**去向**：知识卡" in text, "")
    check("discussion_is_compact", text.count("\n") <= 8, f"{text.count(chr(10))} 行")
    check("discussion_drops_procedure", "来回来去" not in text, "")

    try:
        store.append_record("2026-W43", dt.date(2026, 10, 24),
                            [{"question": "只提了问题", "destination": "inbox"}], mode="discussion")
        check("discussion_requires_conclusion", False, "缺结论未被拦截")
    except ws.WorkLogError as exc:
        check("discussion_requires_conclusion", "结论" in str(exc), str(exc))


# ---------------------------------------------------------------------------
# 4. 长文：只记要点
# ---------------------------------------------------------------------------


def check_long_text(root: Path) -> None:
    store = ws.WorkLogStore(root, today=dt.date(2026, 10, 25))
    store.ensure_skeleton()
    long_source = (
        "根据年度工作安排，本单位于第三季度组织开展了专项检查工作。检查覆盖 12 个部门，"
        "查阅资料 340 份，开展个别谈话 26 人次，形成问题清单 8 项。检查发现的主要问题包括："
        "个别部门制度执行不够严格；台账记录要素不全；整改跟踪缺少闭环记录。针对上述问题，"
        "已逐项明确整改责任人和完成时限，要求 10 月底前完成整改并报送整改情况报告。"
    )
    points = "检查覆盖 12 个部门、查阅资料 340 份；形成问题清单 8 项；要求 10 月底前完成整改"
    p = store.append_record("2026-W43", dt.date(2026, 10, 24), [
        {"text": points, "destination": "本周报引用"},
    ], mode="long")
    text = p.read_text(encoding="utf-8")
    check("long_text_keeps_numbers", "12 个部门" in text and "340 份" in text and "8 项" in text, "")
    check("long_text_is_shorter", len(points) < len(long_source) / 3,
          f"{len(points)} vs {len(long_source)}")
    check("long_text_no_verbatim_copy", long_source[:30] not in text, "")
    check("long_text_preserves_deadline", "10 月底前" in text, "")

    try:
        store.append_record("2026-W43", dt.date(2026, 10, 24), [
            {"text": "一；二；三；四；五；六；七；八", "destination": "归档"}
        ], mode="long")
        check("long_text_rejects_too_many_points", False, "超 6 条要点未被拦截")
    except ws.WorkLogError as exc:
        check("long_text_rejects_too_many_points", "要点" in str(exc), str(exc))

    long_point = "这次检查工作覆盖面广、组织严密、推进有序、成效明显，得到了上级部门的充分肯定和高度评价，也为下一步工作打下了坚实基础并积累了宝贵经验"
    try:
        store.append_record("2026-W43", dt.date(2026, 10, 24),
                            [{"text": long_point, "destination": "归档"}], mode="long")
        check("long_text_rejects_verbatim_paragraph", False, "整段复述未被拦截")
    except ws.WorkLogError as exc:
        check("long_text_rejects_verbatim_paragraph", "复述" in str(exc), str(exc))


# ---------------------------------------------------------------------------
# 5. 周报：起草 → 确认 → 落盘
# ---------------------------------------------------------------------------


def check_report(root: Path) -> None:
    store = ws.WorkLogStore(root, today=dt.date(2026, 10, 25), fs=ws.RecordingFS())
    store.ensure_skeleton()
    store.append_record("2026-W43", dt.date(2026, 10, 21), [
        {"text": "完成专项检查方案定稿。", "destination": "本周报引用", "source_sentence": "完成专项检查方案定稿。"},
        {"text": "顺手记一条待查的旧台账。", "destination": "inbox", "source_sentence": "顺手记一条待查的旧台账。"},
    ])
    store.append_record("2026-W43", dt.date(2026, 10, 22), [
        {"question": "整改期限定多久？", "conclusion": "定为 10 月底前。", "destination": "本周报引用"},
    ], mode="discussion")
    reports_dir = root / ws.REPORT_DIR
    before = sorted(p.name for p in reports_dir.iterdir()) if reports_dir.is_dir() else []

    draft = store.draft_report("2026-W43")
    text = draft["text"]
    check("report_draft_has_all_sections", all(s in text for s in ws.REPORT_SECTIONS), "")
    check("report_draft_only_referenced_items", "专项检查方案定稿" in text, "")
    check("report_draft_excludes_other_destinations", "旧台账" not in text, "")
    check("report_draft_includes_discussion", "10 月底前" in text, "")
    after_draft = sorted(p.name for p in reports_dir.iterdir()) if reports_dir.is_dir() else []
    check("report_draft_writes_nothing", before == after_draft, f"{before} -> {after_draft}")

    # 未确认：明确不写，连空文件都不该出现
    res = store.confirm_report("2026-W43", text, approved=False)
    check("report_unconfirmed_not_written", res["written"] is False, str(res))
    after_reject = sorted(p.name for p in reports_dir.iterdir()) if reports_dir.is_dir() else []
    check("report_dir_still_empty_after_reject", after_reject == before, str(after_reject))

    # 确认后落盘，命名写死
    res2 = store.confirm_report("2026-W43", text, approved=True)
    target = Path(res2["path"])
    check("report_written_after_confirm", res2["written"] is True, str(res2))
    check("report_name_fixed_rule", target.name == "2026-W43-周报.md", target.name)
    check("report_in_reports_dir", target.parent == reports_dir, str(target.parent))
    check("report_content_matches_draft", target.read_text(encoding="utf-8").strip() == text.strip(), "")

    # 同名不覆盖
    res3 = store.confirm_report("2026-W43", text + "\n（第二版）", approved=True)
    check("report_no_overwrite_second_write", Path(res3["path"]).name == "2026-W43-周报-2.md",
          Path(res3["path"]).name)
    check("report_first_file_intact", "（第二版）" not in target.read_text(encoding="utf-8"), "")


# ---------------------------------------------------------------------------
# 6. 建卡防重复 + 索引同步
# ---------------------------------------------------------------------------


def check_cards_and_index(root: Path) -> None:
    store = ws.WorkLogStore(root, today=dt.date(2026, 10, 25))
    store.ensure_skeleton()
    r1 = store.create_card(
        "专项检查频次", "concept", ["监督", "专项检查", "频次"],
        "一类单位每季度不少于一次。",
        "专项检查对一类单位每季度不少于一次，二类单位每半年不少于一次。",
        sources=["库/知识库/原文/检查办法.md"], links=["专项检查方式"],
    )
    check("card_created", r1["action"] == "created", str(r1["action"]))
    check("card_file_exists", Path(r1["path"]).is_file(), r1["path"])

    # 同标题再写：应改为更新，不新增文件
    r2 = store.create_card(
        "专项检查频次", "concept", ["监督", "频次"],
        "一类单位每季度不少于一次（二类每半年）。",
        "专项检查对一类单位每季度不少于一次，二类单位每半年不少于一次，年度可合并。",
        sources=["库/知识库/原文/检查办法.md"],
    )
    check("card_dedupe_same_title", r2["action"] == "updated", str(r2["action"]))
    check("card_dedupe_keeps_one_file",
          len(list((root / ws.KB_DIR / "概念").glob("*.md"))) == 1,
          str(sorted(p.name for p in (root / ws.KB_DIR / "概念").glob("*.md"))))
    index = store.read_index()
    check("index_single_row_after_dedupe", len(index) == 1, str(len(index)))
    check("index_row_updated_summary", "二类每半年" in str(index[0]["summary"]), str(index[0]["summary"]))

    # 同摘要不同标题：也算重复
    r3 = store.create_card(
        "检查频次（二）", "concept", ["监督"],
        "一类单位每季度不少于一次（二类每半年）。",
        "重复内容。",
    )
    check("card_dedupe_same_summary", r3["action"] == "updated", str(r3["action"]))
    check("card_count_still_one", len(list((root / ws.KB_DIR / "概念").glob("*.md"))) == 1, "")

    # 新概念：新增卡并同步索引
    store.create_card(
        "专项检查方式", "method", ["专项检查", "方式"],
        "听汇报、查资料、个别谈话三结合。",
        "专项检查采取听取汇报、查阅资料、个别谈话相结合的方式。",
        links=["专项检查频次"],
    )
    index = store.read_index()
    check("index_grew_with_new_card", len(index) == 2, str(len(index)))
    check("index_has_both_kinds", {str(r["rel"]).split("/")[0] for r in index} == {"概念", "方法"},
          str([r["rel"] for r in index]))
    method_card = root / ws.KB_DIR / "方法" / "专项检查方式.md"
    check("card_has_wikilinks", "[[专项检查频次]]" in method_card.read_text(encoding="utf-8"), "")
    ok, detail = store.index_matches_cards()
    check("index_matches_cards", ok, detail)
    check("index_one_line_per_card",
          all(str(r["rel"]).count("/") == 1 for r in index) and len(index) == 2, "")

    # 卡片正文与格式约束
    body = Path(r1["path"]).read_text(encoding="utf-8")
    check("card_has_frontmatter", body.startswith("---") and "tags:" in body and "date:" in body, "")
    check("card_has_sources", "sources:" in body, "")
    check("card_body_within_300",
          len(ws.render_card("x", ["a"], [], dt.date(2026, 1, 1), "短", [])) < 300, "")

    # 超长正文必须被拒绝（一卡一概念）
    try:
        store.create_card("过长卡", "concept", ["测试"], "摘要", "字" * 301)
        check("card_rejects_over_300", False, "超 300 字未被拦截")
    except ws.WorkLogError as exc:
        check("card_rejects_over_300", "300" in str(exc), str(exc))

    # 摘要过长必须被拒绝（索引行不能撑大成本）
    try:
        store.create_card("长摘要卡", "concept", ["测试"], "摘" * 41, "正文")
        check("card_rejects_long_summary", False, "摘要超 40 字未被拦截")
    except ws.WorkLogError as exc:
        check("card_rejects_long_summary", "40" in str(exc), str(exc))

    # 索引遗漏检测真的会报错
    (root / ws.KB_DIR / "概念" / "未登记卡.md").write_text("x", encoding="utf-8")
    ok2, detail2 = store.index_matches_cards()
    check("index_detects_orphan_card", ok2 is False, detail2)
    (root / ws.KB_DIR / "概念" / "未登记卡.md").unlink()
    ok3, _ = store.index_matches_cards()
    check("index_clean_after_orphan_removed", ok3, "")


# ---------------------------------------------------------------------------
# 7. 两跳检索：命中 + 无整目录读取
# ---------------------------------------------------------------------------


def check_two_hop(root: Path) -> None:
    store = ws.WorkLogStore(root, today=dt.date(2026, 10, 25))
    store.ensure_skeleton()
    store.create_card("专项检查频次", "concept", ["监督", "专项检查", "频次"],
                      "一类单位每季度不少于一次。", "一类单位每季度一次。")
    store.create_card("专项检查方式", "method", ["专项检查", "方式"],
                      "听汇报、查资料、个别谈话。", "三种方式结合。")
    store.create_card("线索处置时限", "method", ["线索", "处置", "时限"],
                      "受理后 15 日内提出处置意见。", "15 日内提出处置意见。")
    # 干扰卡：名字相近但无关，确保检索不是靠数量碰运气
    for i in range(8):
        store.create_card(f"无关方法{i}", "method", ["杂项"], f"与提问无关的第 {i} 项。", "无关正文。")

    res = store.search("专项检查多久开展一次？")
    titles = [h["title"] for h in res["hits"]]
    check("search_hits_expected_card", "专项检查频次" in titles, str(titles))
    check("search_max_two_cards", len(res["hits"]) <= 2, str(len(res["hits"])))
    check("search_first_read_is_index", Path(str(res["reads"][0])) == Path(ws.INDEX_FILE),
          str(res["reads"]))
    check("search_read_count_equals_hits_plus_index",
          res["read_count"] == len(res["hits"]) + 1, str(res["read_count"]))
    check("search_no_dir_scan", res["dir_scans"] == [], str(res["dir_scans"]))
    check("search_loads_body", "每季度" in res["hits"][0]["body"], "")

    # 未收录：仍不扫描目录，返回空
    res2 = store.search("完全无关的天气查询")
    check("search_miss_returns_empty", res2["hits"] == [], str(res2["hits"]))
    check("search_miss_no_dir_scan", res2["dir_scans"] == [], str(res2["dir_scans"]))
    check("search_miss_only_read_index", res2["read_count"] == 1, str(res2["reads"]))

    # 用标签能直接定位
    res3 = store.search("线索处置要多久")
    check("search_by_tag_or_title", any(h["title"] == "线索处置时限" for h in res3["hits"]),
          str([h["title"] for h in res3["hits"]]))

    # 守卫本身有效：伪造一次目录枚举必须被抓到
    bad = ws.RecordingFS()
    bad.read_text(root / ws.KB_DIR / ws.INDEX_FILE)
    bad.list_dir(root / ws.KB_DIR / "概念")
    try:
        bad.assert_two_hop(root)
        check("two_hop_guard_catches_scan", False, "目录扫描未被判定为违规")
    except ws.WorkLogError as exc:
        check("two_hop_guard_catches_scan", "整库扫描" in str(exc), str(exc))

    # 读索引以外的库外文件也要被拦
    bad2 = ws.RecordingFS()
    bad2.read_text(root / ws.KB_DIR / ws.INDEX_FILE)
    bad2.read_text(root / ws.RECORD_DIR / "2026-W43.md") if (root / ws.RECORD_DIR / "2026-W43.md").is_file() else None
    (root / ws.RECORD_DIR / "x.md").write_text("x", encoding="utf-8")
    bad2.read_text(root / ws.RECORD_DIR / "x.md")
    try:
        bad2.assert_two_hop(root)
        check("two_hop_guard_catches_out_of_scope_read", False, "库外读取未被拦截")
    except ws.WorkLogError:
        check("two_hop_guard_catches_out_of_scope_read", True, "")


# ---------------------------------------------------------------------------
# 8. Obsidian：无 CLI 降级 + 有 CLI
# ---------------------------------------------------------------------------


def check_obsidian(root_files: Path, root_cli: Path) -> None:
    # --- 降级路径：探测失败不是错误
    adapter = ws.ObsidianAdapter(root_files, cli=None)
    check("obsidian_fallback_backend", adapter.backend == "files", adapter.backend)
    card = root_files / ws.KB_DIR / "概念" / "降级建卡.md"
    root_files.joinpath(ws.KB_DIR, "概念").mkdir(parents=True, exist_ok=True)
    root_files.joinpath(ws.KB_DIR, ws.INDEX_FILE).write_text(
        "# 知识库索引\n\n## 概念\n- [降级建卡](概念/降级建卡.md) | 标签：测试 | 降级验证。\n\n## 方法\n",
        encoding="utf-8")
    used = adapter.create_note(card, "---\ntags: [测试]\ndate: 2026-10-25\n---\n\n正文 [[降级建卡]]\n")
    check("obsidian_fallback_creates_card", card.is_file() and used == "files", used)
    moved = adapter.move_note(card, root_files / ws.INBOX_DIR / "降级建卡.md")
    check("obsidian_fallback_moves_card",
          (root_files / ws.INBOX_DIR / "降级建卡.md").is_file(), moved)
    links = adapter.check_links(root_files / ws.INBOX_DIR / "降级建卡.md")
    check("obsidian_fallback_link_check_degrades", "未做 CLI 级双链校验" in links["detail"]
          or links["ok"], str(links))
    check("obsidian_fallback_lists_inbox",
          "降级建卡.md" in adapter.list_inbox(), str(adapter.list_inbox()))

    # --- CLI 路径：用受控假 CLI 证明命令形状与行为
    fake = Path(tempfile.mkdtemp(prefix="od085-fakecli-")) / "obsidian"
    fake.write_text(ws.FAKE_CLI, encoding="utf-8")
    fake.chmod(0o755)
    log = fake.parent / "cli.log"
    import os
    os.environ["OD085_CLI_LOG"] = str(log)
    try:
        cli_adapter = ws.ObsidianAdapter.detect(root_cli, which=lambda name: str(fake) if name == "obsidian" else None)
        check("obsidian_cli_detected", cli_adapter.backend == "cli", cli_adapter.backend)
        card2 = root_cli / ws.KB_DIR / "概念" / "CLI建卡.md"
        root_cli.joinpath(ws.KB_DIR, "概念").mkdir(parents=True, exist_ok=True)
        root_cli.joinpath(ws.KB_DIR, ws.INDEX_FILE).write_text(
            "# 知识库索引\n\n## 概念\n- [CLI建卡](概念/CLI建卡.md) | 标签：测试 | CLI 路径。\n\n## 方法\n",
            encoding="utf-8")
        used2 = cli_adapter.create_note(card2, "---\ntags: [测试]\n---\n\n正文 [[CLI建卡]]\n")
        check("obsidian_cli_creates_card", used2 == "cli" and card2.is_file(), used2)
        lk = cli_adapter.check_links(card2)
        check("obsidian_cli_checks_links", lk["backend"] == "cli" and lk["ok"], str(lk))
        cli_adapter.move_note(card2, root_cli / ws.INBOX_DIR / "CLI建卡.md")
        check("obsidian_cli_moves_card", (root_cli / ws.INBOX_DIR / "CLI建卡.md").is_file(), "")
        check("obsidian_cli_lists_inbox", "CLI建卡.md" in cli_adapter.list_inbox(), "")
        check("obsidian_cli_invoked", log.is_file() and "create" in log.read_text(encoding="utf-8"), "")
    finally:
        os.environ.pop("OD085_CLI_LOG", None)

    # --- 真实环境探测：本机有无 Obsidian 都要给出确定结论
    real = ws.ObsidianAdapter.detect(root_files)
    check("obsidian_probe_reports_backend", real.backend in ("cli", "files"), real.backend)


# ---------------------------------------------------------------------------
# 9. 目录骨架：只补建，不覆盖既有
# ---------------------------------------------------------------------------


def check_skeleton(root: Path) -> None:
    store = ws.WorkLogStore(root, today=dt.date(2026, 10, 25))
    first = store.ensure_skeleton()
    check("skeleton_created_all", all(v == "本次新建" for v in first.values()), str(first))
    check("skeleton_index_is_empty_head",
          "一行一卡" in (root / ws.KB_DIR / ws.INDEX_FILE).read_text(encoding="utf-8")
          and not ws.parse_index((root / ws.KB_DIR / ws.INDEX_FILE).read_text(encoding="utf-8")), "")
    # 用户已有内容必须原样保留
    existing = root / ws.RECORD_DIR / "2026-W40.md"
    existing.write_text("## 2026-09-28\n\n- 用户既有记录\n", encoding="utf-8")
    digest = sha(existing)
    second = store.ensure_skeleton()
    check("skeleton_is_idempotent", all(v == "已存在，未改动" for v in second.values()), str(second))
    check("skeleton_preserves_user_file", sha(existing) == digest, "")
    check("skeleton_no_migration", sorted(p.name for p in (root / ws.RECORD_DIR).iterdir()) == ["2026-W40.md"],
          str(sorted(p.name for p in (root / ws.RECORD_DIR).iterdir())))


# ---------------------------------------------------------------------------
# 10. 仓库边界：本次改动不得触碰禁改目录
# ---------------------------------------------------------------------------


def check_repo_scope() -> dict[str, object]:
    def dirty(path: str) -> list[str]:
        res = subprocess.run(["git", "status", "--porcelain", "--", path],
                             cwd=str(REPO), capture_output=True, text=True)
        return [l for l in res.stdout.splitlines() if l.strip()]

    runtime_lines = dirty("runtime/")
    ui_lines = [l[3:].strip() for l in dirty("desktop/src/")]
    # OD-ARCH-086 获批修改 src-tauri/（白名单 + 库根）；OD-ARCH-088 获批修改
    # engine/omnidesk_engine/ 的适配层与 RPC 入口，engine/ 其余文件仍属冻结范围。
    frozen_lines = [l for l in dirty("engine/") if l[3:].strip() not in ENGINE_ALLOWED]
    check("runtime_zero_diff", runtime_lines == [], str(runtime_lines))
    check("frozen_engine_zero_diff", frozen_lines == [], str(frozen_lines))
    # desktop/src 的改动 = 084 遗留 + 086 库根入口 + 088 提问卡，须与登记清单逐项相同。
    check("desktop_src_matches_registered_baseline",
          sorted(ui_lines) == sorted(FROZEN_BASELINE["desktop/src"]),
          f"current={sorted(ui_lines)} baseline={sorted(FROZEN_BASELINE['desktop/src'])}")
    return {"runtime": runtime_lines, "frozen_backend": frozen_lines, "desktop_src": sorted(ui_lines)}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--no-write", action="store_true")
    args = ap.parse_args()

    roots: list[Path] = []
    try:
        check_skill_text()

        r = fresh_root("one"); roots.append(r); check_one_liner(r)
        r = fresh_root("append"); roots.append(r); check_append_only(r)
        r = fresh_root("disc"); roots.append(r); check_discussion(r)
        r = fresh_root("long"); roots.append(r); check_long_text(r)
        r = fresh_root("report"); roots.append(r); check_report(r)
        r = fresh_root("card"); roots.append(r); check_cards_and_index(r)
        r = fresh_root("twohop"); roots.append(r); check_two_hop(r)
        r = fresh_root("skel"); roots.append(r); check_skeleton(r)

        r_files = fresh_root("obs-files"); roots.append(r_files)
        r_cli = fresh_root("obs-cli"); roots.append(r_cli)
        check_obsidian(r_files, r_cli)

        scope = check_repo_scope()
    finally:
        for r in roots:
            shutil.rmtree(r, ignore_errors=True)

    failed = [c for c in CASES if not c["ok"]]
    summary = {
        "task": "OD-ARCH-085",
        "generated_at": dt.datetime.now().isoformat(timespec="seconds"),
        "isolation": "全部数据根为 /tmp 一次性副本，未读写 ~/OmniDesk 真实数据",
        "cases_total": len(CASES),
        "cases_passed": len(CASES) - len(failed),
        "cases_failed": len(failed),
        "result": "pass" if not failed else "fail",
        "repo_scope": scope,
        "cases": CASES,
    }
    text = json.dumps(summary, ensure_ascii=False, indent=2)
    print(f"cases={summary['cases_total']} passed={summary['cases_passed']} failed={summary['cases_failed']}")
    for c in failed:
        print("  [FAIL] %s — %s" % (c["name"], c["detail"][:200]))
    if not args.no_write:
        EVIDENCE.mkdir(parents=True, exist_ok=True)
        (EVIDENCE / "facts.json").write_text(text + "\n", encoding="utf-8")
        print("evidence:", EVIDENCE / "facts.json")
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main())


