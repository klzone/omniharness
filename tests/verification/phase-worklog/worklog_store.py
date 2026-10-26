#!/usr/bin/env python3
"""OD-ARCH-085 工作记录场景：参考实现与硬约束守卫。

SKILL.md 写的是「模型该怎么做」，本模块把其中最容易被稀释的规则做成可执行契约，
供合成评测逐条断言：

* 工作记录只追加（append-only），既有字节不得改动；
* 每条必须有「去向」，四选一；
* 一句话记录不硬扩（正文必须等于原句）；
* 索引.md 一行一卡，建卡/改卡后索引与卡片同步，建卡前查重；
* 两跳检索：只读索引 + 至多两张命中卡，目录扫描由读取日志判定为违规；
* 周报未经用户确认不落盘，确认后按写死规则命名、不覆盖同名；
* Obsidian CLI 可用走 CLI，不可用降级纯文件读写，两条路径都实现。

证据等级：本模块为 static（源码事实）；verify_worklog_085.py 为 script（脚本运行）。
所有数据均为合成样例，不读取任何真实工作记录。
"""
from __future__ import annotations

import datetime as _dt
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

# ---------------------------------------------------------------------------
# 常量：与 SKILL.md 写死的规则一一对应
# ---------------------------------------------------------------------------

DESTINATIONS = ("本周报引用", "知识卡", "inbox", "归档")
RECORD_DIR = "工作记录"
REPORT_DIR = "周报"
KB_DIR = "知识库"
INDEX_FILE = "索引.md"
INBOX_DIR = "inbox"
CARD_KINDS = {"concept": "概念", "method": "方法"}
CARD_MAX_CHARS = 300
SUMMARY_MAX_CHARS = 40
REPORT_SECTIONS = ("本周概述", "事项进展", "问题与风险", "下步计划", "待确认事项")


class WorkLogError(RuntimeError):
    """违反了 SKILL.md 写死的规则。"""


def iso_week(d: _dt.date) -> str:
    y, w, _ = d.isocalendar()
    return f"{y}-W{w:02d}"


def record_name(d: _dt.date) -> str:
    return f"{iso_week(d)}.md"


def report_name(d: _dt.date) -> str:
    return f"{iso_week(d)}-周报.md"


def normalize(text: str) -> str:
    return re.sub(r"[\s\u3000·,，。、；;:：!！?？\"'“”‘’()（）\-_/]+", "", text).lower()


# ---------------------------------------------------------------------------
# 读取日志：两跳检索的「未扫描目录」由它作证
# ---------------------------------------------------------------------------


class RecordingFS:
    """记录每一次文件读取与目录枚举，用于断言检索没有整目录扫描。"""

    def __init__(self) -> None:
        self.ops: list[tuple[str, str]] = []

    def read_text(self, path: Path) -> str:
        self.ops.append(("read", str(path)))
        return Path(path).read_text(encoding="utf-8")

    def list_dir(self, path: Path) -> list[str]:
        self.ops.append(("list", str(path)))
        return sorted(os.listdir(path))

    def reset(self) -> None:
        self.ops.clear()

    def reads(self) -> list[str]:
        return [p for kind, p in self.ops if kind == "read"]

    def scanned_dirs(self) -> list[str]:
        return [p for kind, p in self.ops if kind == "list"]

    def assert_two_hop(self, root: Path) -> None:
        """只允许读索引.md 与命中卡片；任何目录枚举都算整库扫描。"""
        kb = Path(root) / KB_DIR
        allowed_index = kb / INDEX_FILE
        for path in self.reads():
            p = Path(path)
            if p == allowed_index:
                continue
            if p.parent == kb / "概念" or p.parent == kb / "方法":
                continue
            raise WorkLogError(f"检索读到了索引与卡片之外的路径：{p}")
        scans = self.scanned_dirs()
        if scans:
            raise WorkLogError("检索发生了目录枚举（整库扫描）：" + ", ".join(scans))


# ---------------------------------------------------------------------------
# 存储与守卫
# ---------------------------------------------------------------------------


class WorkLogStore:
    def __init__(self, root: Path, fs: RecordingFS | None = None, today: _dt.date | None = None) -> None:
        self.root = Path(root)
        self.fs = fs or RecordingFS()
        self.today = today or _dt.date(2026, 10, 25)
        self.writes: list[str] = []

    # --- 目录骨架 ---------------------------------------------------------

    def ensure_skeleton(self) -> dict[str, str]:
        """只补建缺失目录与空索引；已有文件一律不动。"""
        plan = {
            RECORD_DIR: self.root / RECORD_DIR,
            REPORT_DIR: self.root / REPORT_DIR,
            KB_DIR: self.root / KB_DIR,
            "概念": self.root / KB_DIR / "概念",
            "方法": self.root / KB_DIR / "方法",
            INBOX_DIR: self.root / INBOX_DIR,
        }
        report: dict[str, str] = {}
        for label, path in plan.items():
            if path.is_dir():
                report[label] = "已存在，未改动"
            else:
                path.mkdir(parents=True, exist_ok=True)
                report[label] = "本次新建"
        index = self.root / KB_DIR / INDEX_FILE
        if index.is_file():
            report[INDEX_FILE] = "已存在，未改动"
        else:
            index.write_text(
                "# 知识库索引\n\n"
                "> 一行一卡：标题 + 标签 + 一句话摘要。正文在卡片里，按需加载。\n\n"
                "## 概念\n\n## 方法\n",
                encoding="utf-8",
            )
            report[INDEX_FILE] = "本次新建"
        return report

    # --- 工作记录：只追加 -------------------------------------------------

    def record_path(self, week: str) -> Path:
        return self.root / RECORD_DIR / f"{week}.md"

    def append_record(
        self,
        week: str,
        date: _dt.date,
        entries: list[dict[str, str]],
        mode: str = "note",
    ) -> Path:
        """追加一个日期节的条目块。

        entries: [{"text": ..., "destination": ..., "question": ..., "conclusion": ...,
                   "source_sentence": ...}]
        只以追加模式打开文件；写完断言旧字节未变。
        """
        if not entries:
            raise WorkLogError("没有可记录的条目")
        path = self.record_path(week)
        before = path.read_bytes() if path.is_file() else b""
        block = self.render_block(date, entries, mode, already_has_date=before.count(
            ("## " + date.isoformat()).encode("utf-8")
        ) > 0)
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "ab") as fh:  # 追加模式：不 seek、不截断
            fh.write(block.encode("utf-8"))
            fh.flush()
            os.fsync(fh.fileno())
        after = path.read_bytes()
        if not after.startswith(before):
            raise WorkLogError("工作记录被重写：追加后旧字节发生变化")
        if len(after) != len(before) + len(block.encode("utf-8")):
            raise WorkLogError("工作记录写入长度不符，疑似覆盖")
        self.writes.append(str(path))
        return path

    def render_block(
        self,
        date: _dt.date,
        entries: list[dict[str, str]],
        mode: str,
        already_has_date: bool,
    ) -> str:
        for e in entries:
            self.validate_entry(e, mode)
        lines: list[str] = []
        if not already_has_date:
            lines.append("## " + date.isoformat())
        else:
            lines.append("## " + date.isoformat() + "（续）")
        lines.append("")
        for e in entries:
            if mode == "discussion":
                lines.append("- **问题**：" + e["question"].strip())
                lines.append("  **结论**：" + e["conclusion"].strip())
            else:
                lines.append("- **事**：" + e["text"].strip())
            lines.append("  **去向**：" + e["destination"].strip())
            note = e.get("note")
            if note:
                lines.append("  **待确认**：" + note.strip())
        lines.append("")
        return "\n".join(lines) + "\n"

    def validate_entry(self, entry: dict[str, str], mode: str) -> None:
        dest = entry.get("destination", "").strip()
        if dest not in DESTINATIONS:
            raise WorkLogError(f"条目缺少合法去向（{'/'.join(DESTINATIONS)}）：{dest!r}")
        if mode == "note":
            text = entry.get("text", "").strip()
            if not text:
                raise WorkLogError("一句话记录缺少正文")
            src = entry.get("source_sentence")
            if src is not None and normalize(text) != normalize(src):
                raise WorkLogError(
                    "一句话记录被扩写：'%s' 与原始句子不一致" % text
                )
            if "\n" in text.strip():
                raise WorkLogError("一句话记录不应包含多行铺陈")
        elif mode == "discussion":
            if not entry.get("question", "").strip() or not entry.get("conclusion", "").strip():
                raise WorkLogError("多轮讨论必须提炼「问题 → 结论」两项")
        elif mode == "long":
            text = entry.get("text", "").strip()
            if not text:
                raise WorkLogError("长文要点为空")
            bullets = [b for b in re.split(r"[；;\n]", text) if b.strip()]
            if len(bullets) > 6:
                raise WorkLogError("长文只记要点，条目超过 6 条要点")
            for b in bullets:
                if len(b.strip()) > 60:
                    raise WorkLogError("长文要点过长（>60 字），疑似复述原文：" + b.strip()[:20])
        else:
            raise WorkLogError("未知记录模式：" + mode)

    # --- 索引与卡片 -------------------------------------------------------

    @property
    def index_path(self) -> Path:
        return self.root / KB_DIR / INDEX_FILE

    def read_index(self) -> list[dict[str, object]]:
        text = self.fs.read_text(self.index_path) if self.index_path.is_file() else ""
        return parse_index(text)

    def create_card(
        self,
        title: str,
        kind: str,
        tags: list[str],
        summary: str,
        body: str,
        sources: list[str] | None = None,
        date: _dt.date | None = None,
        links: list[str] | None = None,
    ) -> dict[str, object]:
        """写卡：先查索引防重复，再写文件，最后同步索引。"""
        if kind not in CARD_KINDS:
            raise WorkLogError("卡片类型必须是 concept 或 method")
        if len(summary.strip()) > SUMMARY_MAX_CHARS:
            raise WorkLogError("索引摘要超过 40 字，会撑大第一步读取成本")
        if len(body.strip()) > CARD_MAX_CHARS:
            raise WorkLogError(
                "卡片正文 %d 字超过 300 字上限，说明一张卡装了两个概念" % len(body.strip())
            )
        if not tags:
            raise WorkLogError("卡片至少要有一个标签")

        index = self.read_index()
        existing = None
        ntitle = normalize(title)
        for row in index:
            if normalize(str(row["title"])) == ntitle:
                existing = row
                break
            if normalize(str(row["summary"])) == normalize(summary):
                existing = row
                break

        day = date or self.today
        if existing is not None:
            # 已有同概念卡：改那张，不新建文件、不新增索引行
            rel = str(existing["rel"])
            folder, _, filename = rel.partition("/")
            title = str(existing["title"])
            tags = list(existing["tags"]) if existing["tags"] else tags
            path = self.root / KB_DIR / folder / filename
        else:
            folder = CARD_KINDS[kind]
            rel = f"{folder}/{title}.md"
            path = self.root / KB_DIR / folder / f"{title}.md"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(render_card(title, tags, sources or [], day, body, links or []), encoding="utf-8")
        self.writes.append(str(path))
        self.upsert_index(kind, title, rel, tags, summary)
        return {
            "action": "updated" if existing else "created",
            "path": str(path),
            "rel": rel,
            "title": title,
        }

    def upsert_index(self, kind: str, title: str, rel: str, tags: list[str], summary: str) -> None:
        """索引一行一卡：存在则原地替换该行，不存在则插入对应分组末尾。"""
        text = self.index_path.read_text(encoding="utf-8") if self.index_path.is_file() else (
            "# 知识库索引\n\n> 一行一卡：标题 + 标签 + 一句话摘要。正文在卡片里，按需加载。\n\n## 概念\n\n## 方法\n"
        )
        heading = "## " + CARD_KINDS[kind]
        line = f"- [{title}]({rel}) | 标签：{', '.join(tags)} | {summary.strip()}"
        rows = text.splitlines()
        inserted = False
        replaced = False
        for i, row in enumerate(rows):
            m = re.match(r"^- \[(.*?)\]\((.*?)\)", row)
            if m and (m.group(2) == rel or normalize(m.group(1)) == normalize(title)):
                rows[i] = line
                replaced = True
                break
        if not replaced:
            if heading not in rows:
                rows.extend(["", heading, ""])
            start = rows.index(heading)
            insert_at = len(rows)
            for j in range(start + 1, len(rows)):
                if rows[j].startswith("## "):
                    insert_at = j
                    break
            else:
                insert_at = len(rows)
            while insert_at > start + 1 and rows[insert_at - 1].strip() == "":
                insert_at -= 1
            rows.insert(insert_at, line)
            inserted = True
        new_text = "\n".join(rows)
        if not new_text.endswith("\n"):
            new_text += "\n"
        self.index_path.write_text(new_text, encoding="utf-8")
        self.writes.append(str(self.index_path))
        if not (inserted or replaced):
            raise WorkLogError("索引未更新")

    def index_matches_cards(self) -> tuple[bool, str]:
        """索引条目与实际卡片文件必须一一对应。"""
        index = self.read_index()
        listed = {str(r["rel"]) for r in index}
        on_disk: set[str] = set()
        for folder in CARD_KINDS.values():
            d = self.root / KB_DIR / folder
            if not d.is_dir():
                continue
            for p in d.glob("*.md"):
                on_disk.add(f"{folder}/{p.name}")
        if listed != on_disk:
            return False, f"索引多出 {sorted(listed - on_disk)}，文件多出 {sorted(on_disk - listed)}"
        return True, f"{len(listed)} 张卡与索引一致"

    # --- 两跳检索 ---------------------------------------------------------

    def search(self, query: str, limit: int = 2) -> dict[str, object]:
        """第一跳读索引，第二跳只加载命中卡。绝不枚举卡片目录。"""
        self.fs.reset()
        rows = self.read_index()
        scored: list[tuple[float, dict[str, object]]] = []
        q = normalize(query)
        for row in rows:
            score = 0.0
            for tag in row["tags"]:
                if normalize(str(tag)) and normalize(str(tag)) in q:
                    score += 3
            title = normalize(str(row["title"]))
            if title and (title in q or q in title):
                score += 2
            shingles = {q[i:i + 2] for i in range(len(q) - 1)}
            target = title + normalize(str(row["summary"]))
            score += sum(0.4 for s in shingles if s and s in target)
            # 只有标签/标题命中，或字符重合达到阈值，才算命中。
            # 否则两字重合就会被当成结果，检索会变成「总能翻出点什么」。
            if score >= 2.0:
                scored.append((score, row))
        scored.sort(key=lambda x: (-x[0], str(x[1]["title"])))
        hits = []
        for score, row in scored[:limit]:
            path = self.root / KB_DIR / str(row["rel"])
            body = self.fs.read_text(path)
            hits.append(
                {
                    "title": row["title"],
                    "tags": row["tags"],
                    "summary": row["summary"],
                    "rel": row["rel"],
                    "body": body,
                    "score": round(score, 2),
                }
            )
        self.fs.assert_two_hop(self.root)
        return {
            "query": query,
            "hits": hits,
            "reads": [str(Path(p).name) for p in self.fs.reads()],
            "read_count": len(self.fs.reads()),
            "dir_scans": self.fs.scanned_dirs(),
        }

    # --- 周报：确认后落盘 ------------------------------------------------

    def draft_report(self, week: str) -> dict[str, object]:
        """只读当周记录并起草；此方法不写任何文件。"""
        path = self.record_path(week)
        if not path.is_file():
            raise WorkLogError(f"当周记录不存在：{path.name}")
        text = self.fs.read_text(path)
        # 逐条取「事/问题」行与去向行配对
        items: list[str] = []
        pending: str | None = None
        for ln in text.splitlines():
            stripped = ln.strip()
            if stripped.startswith("- **事**："):
                pending = stripped[len("- **事**："):]
            elif stripped.startswith("- **问题**："):
                pending = stripped[len("- **问题**："):]
            elif stripped.startswith("**结论**："):
                if pending is not None:
                    pending = pending + " → " + stripped[len("**结论**："):]
            elif stripped.startswith("**去向**："):
                dest = stripped[len("**去向**："):].strip()
                if dest == "本周报引用" and pending:
                    items.append(pending)
                pending = None
        lines = [f"# {week} 周报", ""]
        for i, section in enumerate(REPORT_SECTIONS):
            lines.append("## " + section)
            lines.append("")
            if i == 0:
                lines.append(f"本周记录 {len(items)} 项（仅含标记为「本周报引用」的条目）。")
            elif i == 1:
                if items:
                    lines.extend("- " + it for it in items)
                else:
                    lines.append("（本周无标记为周报引用的事项）")
            elif i == 4:
                lines.append("（无）" if items else "（本周记录不足以成稿，待补充）")
            else:
                lines.append("（待用户补充）")
            lines.append("")
        return {
            "week": week,
            "text": "\n".join(lines),
            "item_count": len(items),
            "writes": list(self.writes),
        }

    def confirm_report(self, week: str, text: str, approved: bool) -> dict[str, object]:
        """approved=False 时一个字节都不写。"""
        if not approved:
            return {"written": False, "reason": "用户未确认，未落盘", "path": None}
        if not text.strip():
            raise WorkLogError("周报正文为空，拒绝落盘")
        base = report_name(self.today if week == iso_week(self.today) else _dt.date.fromisocalendar(
            int(week.split("-W")[0]), int(week.split("-W")[1]), 1
        ))
        target = self.root / REPORT_DIR / base
        target.parent.mkdir(parents=True, exist_ok=True)
        n = 2
        while target.exists():  # 不覆盖同名
            target = self.root / REPORT_DIR / base.replace("-周报.md", f"-周报-{n}.md")
            n += 1
        with open(target, "x", encoding="utf-8") as fh:  # create_new：拒绝覆盖
            fh.write(text if text.endswith("\n") else text + "\n")
            fh.flush()
            os.fsync(fh.fileno())
        self.writes.append(str(target))
        return {"written": True, "path": str(target), "name": target.name}


# ---------------------------------------------------------------------------
# 渲染与解析
# ---------------------------------------------------------------------------


def render_card(
    title: str,
    tags: list[str],
    sources: list[str],
    date: _dt.date,
    body: str,
    links: list[str],
) -> str:
    out = ["---", f"tags: [{', '.join(tags)}]",
           f"sources: [{', '.join(sources)}]" if sources else "sources: []",
           f"date: {date.isoformat()}", "---", "", title, ""]
    out.append(body.strip())
    if links:
        out.extend(["", "## 相关", ""])
        out.extend(f"- [[{l}]]" for l in links)
    return "\n".join(out) + "\n"


INDEX_LINE = re.compile(r"^- \[(?P<title>.*?)\]\((?P<rel>.*?)\) \| 标签：(?P<tags>.*?) \| (?P<summary>.*)$")


def parse_index(text: str) -> list[dict[str, object]]:
    rows = []
    for line in text.splitlines():
        m = INDEX_LINE.match(line.strip())
        if not m:
            continue
        rows.append({
            "title": m.group("title"),
            "rel": m.group("rel"),
            "tags": [t.strip() for t in m.group("tags").split(",") if t.strip()],
            "summary": m.group("summary").strip(),
        })
    return rows


# ---------------------------------------------------------------------------
# Obsidian CLI 适配：两条路径都实现
# ---------------------------------------------------------------------------

FAKE_CLI = r'''#!/usr/bin/env python3
import json, os, sys
args = sys.argv[1:]
log = os.environ.get("OD085_CLI_LOG")
if log:
    with open(log, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(args, ensure_ascii=False) + "\n")
cmd = args[0] if args else ""
if cmd == "create":
    p = args[1]
    open(p, "w", encoding="utf-8").write(args[2] if len(args) > 2 else "")
    print("created " + p)
elif cmd == "move":
    os.makedirs(os.path.dirname(args[2]) or ".", exist_ok=True)
    os.replace(args[1], args[2])
    print("moved")
elif cmd == "links":
    p = args[1]
    txt = open(p, encoding="utf-8").read()
    print("ok" if "[[" in txt else "no-links")
elif cmd == "list":
    for name in sorted(os.listdir(args[1])):
        print(name)
else:
    print("unknown")
    sys.exit(1)
'''


class ObsidianAdapter:
    """有 CLI 就调 CLI；没有就退化成普通文件读写。功能集合相同。"""

    def __init__(self, root: Path, cli: str | None = None) -> None:
        self.root = Path(root)
        self.cli = cli
        self.backend = "cli" if cli else "files"

    @classmethod
    def detect(cls, root: Path, which=None) -> "ObsidianAdapter":
        which = which or shutil.which
        found = which("obsidian")
        return cls(root, found if found and os.access(found, os.X_OK) else None)

    def create_note(self, path: Path, text: str) -> str:
        path.parent.mkdir(parents=True, exist_ok=True)
        if self.backend == "cli":
            res = subprocess.run([self.cli, "create", str(path), text], capture_output=True, text=True)
            if res.returncode != 0:
                raise WorkLogError("Obsidian CLI 建卡失败：" + res.stderr.strip())
            return "cli"
        path.write_text(text, encoding="utf-8")
        return "files"

    def move_note(self, src: Path, dst: Path) -> str:
        dst.parent.mkdir(parents=True, exist_ok=True)
        if self.backend == "cli":
            res = subprocess.run([self.cli, "move", str(src), str(dst)], capture_output=True, text=True)
            if res.returncode != 0:
                raise WorkLogError("Obsidian CLI 移动失败：" + res.stderr.strip())
            return "cli"
        os.replace(src, dst)
        return "files"

    def check_links(self, path: Path) -> dict[str, object]:
        """双链校验：CLI 可做；降级时只能做「索引标题 ↔ 卡片链接」比对。"""
        if self.backend == "cli":
            res = subprocess.run([self.cli, "links", str(path)], capture_output=True, text=True)
            return {"backend": "cli", "ok": res.returncode == 0 and "ok" in res.stdout, "detail": res.stdout.strip()}
        text = path.read_text(encoding="utf-8")
        index_titles = {str(r["title"]) for r in parse_index((self.root / KB_DIR / INDEX_FILE).read_text(encoding="utf-8"))}
        links = re.findall(r"\[\[(.*?)\]\]", text)
        missing = [l for l in links if l not in index_titles]
        return {
            "backend": "files",
            "ok": not missing,
            "detail": "未做 CLI 级双链校验" if not links else f"缺失链接目标：{missing}",
            "links": links,
        }

    def list_inbox(self) -> list[str]:
        inbox = self.root / INBOX_DIR
        if not inbox.is_dir():
            return []
        if self.backend == "cli":
            res = subprocess.run([self.cli, "list", str(inbox)], capture_output=True, text=True)
            if res.returncode == 0:
                return [l for l in res.stdout.splitlines() if l.strip()]
        return sorted(os.listdir(inbox))


