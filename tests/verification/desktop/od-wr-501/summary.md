# OD-WR-501 验证记录

- 基线：HEAD `eec28fd6357b4a8490613fa4694115e6e04c42d4`，开工时 `git status --short` 为空。
- 验收命令：`export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib`
- 结果：退出码 0；`test result: ok. 1120 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`。
- 原始输出：`cargo-test-lib.txt`。
- 新增测试 10 项：
  - worklog_records：legacy_record_without_raw_fields_load_save_is_byte_identical、new_record_omits_raw_fields_until_set、create_writes_raw_input_and_source_to_frontmatter、blank_raw_input_is_normalized_to_none_and_omitted、patch_has_no_raw_input_entry_and_update_preserves_raw、update_appends_revision_with_changed_fields_and_old_body_hash、successive_updates_hash_the_previous_body_each_time、empty_patch_still_records_an_update、truncated_revision_tail_blocks_update_without_touching_record。
  - quicklog：append_records_raw_input_and_local_home_chat_source。
- 编译警告：2 条既有 dead_code 警告（export_engine.rs `stderr`、writing_section.rs 测试夹具 `config`），与本卡无关。
- 改动文件：worklog_records.rs、quicklog.rs；同步字段的既有调用点 worklog_migrate.rs、knowledge_link.rs 测试夹具。
