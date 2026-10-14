#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import os, sys, shutil, subprocess, zipfile
from pathlib import Path

SRC_DIR = Path("/mnt/data/Documents/U350恢复/纪委工作/制度梳理").resolve()
DEST_DIR = Path("/mnt/data/Code/MyWorkStation/knowledge/regulations").resolve()

SOFFICE_BIN = "/snap/bin/libreoffice"
if not os.path.exists(SOFFICE_BIN):
    which_lo = shutil.which("libreoffice") or shutil.which("soffice")
    if which_lo:
        SOFFICE_BIN = which_lo

TEMP_STAGING_DIR = Path.home() / ".cache" / "omnidesk_wps_staging"

def is_docx_zip(file_path):
    try:
        with zipfile.ZipFile(file_path, "r") as zf:
            return "word/document.xml" in zf.namelist()
    except Exception:
        return False

def convert_single_doc_to_docx(src_file, target_docx_file):
    TEMP_STAGING_DIR.mkdir(parents=True, exist_ok=True)
    temp_in = TEMP_STAGING_DIR / f"convert_{os.getpid()}_{src_file.name}.doc"
    shutil.copy2(src_file, temp_in)
    cmd = [
        SOFFICE_BIN,
        "--headless",
        "--convert-to", "docx",
        str(temp_in),
        "--outdir", str(TEMP_STAGING_DIR)
    ]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        temp_out = TEMP_STAGING_DIR / f"convert_{os.getpid()}_{src_file.name}.docx"
        if temp_out.exists():
            target_docx_file.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(temp_out), str(target_docx_file))
            if temp_in.exists():
                temp_in.unlink()
            return True, "Converted"
        else:
            if temp_in.exists():
                temp_in.unlink()
            return False, res.stderr.strip() or res.stdout.strip()
    except Exception as e:
        if temp_in.exists():
            temp_in.unlink()
        return False, str(e)

def main():
    divider = "=" * 65
    print(divider)
    print("  OmniDesk 制度文档梳理转换与归类程序")
    print(f"  源目录: {SRC_DIR}")
    print(f"  目标目录: {DEST_DIR}")
    print(divider)
    
    if not SRC_DIR.exists():
        print(f"错误: 源目录不存在: {SRC_DIR}")
        sys.exit(1)
        
    DEST_DIR.mkdir(parents=True, exist_ok=True)
    wps_converted = 0
    wps_copied_as_docx = 0
    wps_encrypted = 0
    wps_failed = 0
    word_copied = 0
    pdf_copied = 0
    skipped_other = 0
    
    for root, dirs, files in os.walk(SRC_DIR):
        rel_path = Path(root).relative_to(SRC_DIR)
        target_sub_dir = DEST_DIR / rel_path
        for file_name in files:
            src_file_path = Path(root) / file_name
            ext = src_file_path.suffix.lower()
            if ext == ".wps":
                target_sub_dir.mkdir(parents=True, exist_ok=True)
                stem = src_file_path.stem
                target_docx_path = target_sub_dir / f"{stem}.docx"
                if is_docx_zip(src_file_path):
                    shutil.copy2(src_file_path, target_docx_path)
                    wps_copied_as_docx += 1
                    print(f"[WPS->DOCX (原生XML)] {rel_path}/{file_name}")
                    continue
                success, msg = convert_single_doc_to_docx(src_file_path, target_docx_path)
                if success:
                    wps_converted += 1
                    print(f"[WPS->DOCX (转换成功)] {rel_path}/{file_name}")
                else:
                    if "could not be loaded" in msg:
                        wps_encrypted += 1
                        print(f"[WPS加密/只读保留原件] {rel_path}/{file_name}")
                        shutil.copy2(src_file_path, target_sub_dir / file_name)
                    else:
                        wps_failed += 1
                        print(f"[WPS转换失败] {rel_path}/{file_name} -> {msg}")
            elif ext in [".doc", ".docx"]:
                target_sub_dir.mkdir(parents=True, exist_ok=True)
                target_file_path = target_sub_dir / file_name
                shutil.copy2(src_file_path, target_file_path)
                word_copied += 1
                print(f"[Word保留复制] {rel_path}/{file_name}")
            elif ext == ".pdf":
                target_sub_dir.mkdir(parents=True, exist_ok=True)
                target_file_path = target_sub_dir / file_name
                shutil.copy2(src_file_path, target_file_path)
                pdf_copied += 1
                print(f"[PDF保留复制] {rel_path}/{file_name}")
            else:
                skipped_other += 1

    if TEMP_STAGING_DIR.exists():
        shutil.rmtree(TEMP_STAGING_DIR, ignore_errors=True)

    print()
    print(divider)
    print("  统计汇总结果:")
    print(f"  - WPS 转 DOCX 成功: {wps_converted + wps_copied_as_docx} 份 (包含 {wps_copied_as_docx} 份直接更名)")
    print(f"  - WPS 加密/只读 (保留原件待解密): {wps_encrypted} 份")
    if wps_failed > 0:
        print(f"  - WPS 转换失败: {wps_failed} 份")
    print(f"  - Word (.doc/.docx) 保留复制: {word_copied} 份")
    print(f"  - PDF (.pdf) 保留复制: {pdf_copied} 份")
    print(f"  - 其他格式跳过未动: {skipped_other} 份")
    print(f"  - 目标制度法规库目录: {DEST_DIR}")
    print(divider)

if __name__ == "__main__":
    main()
