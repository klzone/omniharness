#!/usr/bin/env bash
# 把 VERSION 写进 desktop/src-tauri/tauri.conf.json（OD-ARCH-134）。
#
# 发布包与 NSIS 安装器的文件名取自 tauri.conf.json 的 version，Release tag 取自
# VERSION。两处各写一遍迟早会不一致（0.2.5 发布时 conf 还停在 0.2.3 就是这样），
# 发布前统一同步一次。内容不变时不写文件，避免制造无意义的差异。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
CONF="$ROOT/desktop/src-tauri/tauri.conf.json"
VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION")"

if [ -z "$VERSION" ]; then
  printf 'VERSION 为空，无法同步版本号。\n' >&2
  exit 1
fi
if [ ! -f "$CONF" ]; then
  printf '找不到 %s\n' "$CONF" >&2
  exit 1
fi

node - "$CONF" "$VERSION" <<'NODE'
const fs = require("fs");
const [conf, version] = process.argv.slice(2);
const json = JSON.parse(fs.readFileSync(conf, "utf8"));
if (json.version === version) {
  console.log(`tauri.conf.json 已是 ${version}`);
  process.exit(0);
}
json.version = version;
fs.writeFileSync(conf, JSON.stringify(json, null, 2) + "\n");
console.log(`tauri.conf.json → ${version}`);
NODE
