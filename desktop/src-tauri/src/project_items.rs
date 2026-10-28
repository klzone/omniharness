//! 项目事项的本地持久化（OD-ARCH-102 S01）。
//!
//! 定位：**应用数据**，与 threads 同级，落在应用数据根下的 `project-items.v1.json`，
//! 不是用户知识库或成果目录。项目跟进页 v3 的卡片、编辑、删除与时间线都以本文件为准；
//! 首页对话 → `task_project_summary` 的旧链路保留，但不再驱动该页。
//!
//! 边界与 thread_store 一致：
//!   * 路径必须通过本地数据根校验，且真实落点不得在仓库内；
//!   * 只写这一个文件与它的 `.bak`，不触碰其它目录；
//!   * 写入走同目录临时文件 + rename（原子替换），旧文件先备份为 `.bak`；
//!   * 条目校验复用 task_shell 的项目规则（负责人只允许 P+数字、日期合法、
//!     开始日期不晚于期限、状态枚举），非法条目整批拒绝并报「第 N 条」；
//!   * 读到坏主文件时只回退到 `.bak`，两份都坏就如实报错，不猜测、不静默清空。

use std::path::{Path, PathBuf};

use crate::app_paths::{resolve_real_path, AppPaths};
use crate::task_shell::{
    assert_local_root_allowed, assert_outside_repo, validate_project_due, validate_project_owner,
    validate_project_start, validate_project_status,
};

pub const PROJECT_ITEMS_SCHEMA: &str = "omnidesk.project-items.v1";
pub const PROJECT_ITEMS_FILE: &str = "project-items.v1.json";
/// 单个存储文件上限：事项条目都是短文本，异常大小说明不是本流程的文件。
pub const PROJECT_ITEMS_MAX_BYTES: u64 = 1024 * 1024;
/// 条目数上限：个人工作台的清单远小于此，超出即拒绝而不是悄悄截断。
pub const PROJECT_ITEMS_MAX_COUNT: usize = 500;
/// 单字段字符数上限，防止把整段正文塞进事项标题。
const MAX_FIELD_CHARS: usize = 400;

/// 一条项目事项。字段与 `task_shell::ProjectItem` 对齐，另带创建/更新时间。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectItemRecord {
    pub item: String,
    pub owner: String,
    pub due: String,
    #[serde(default)]
    pub start: Option<String>,
    pub status: String,
    pub next_step: String,
    #[serde(default)]
    pub created_at: f64,
    #[serde(default)]
    pub updated_at: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectItemsDoc {
    pub schema: String,
    pub items: Vec<ProjectItemRecord>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProjectItemsListing {
    /// 实际使用的存储文件（绝对路径；供界面显示与排障）。
    pub path: String,
    pub items: Vec<ProjectItemRecord>,
    /// 主文件不存在（首次使用）或损坏后从 `.bak` 读回等情况的说明；正常时为 null。
    pub note: Option<String>,
    /// 本次内容是否来自 `.bak`。
    pub recovered_from_backup: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProjectItemsSaveReceipt {
    pub path: String,
    pub bytes: u64,
    pub items: Vec<ProjectItemRecord>,
}

pub struct ProjectItemsStore {
    path: PathBuf,
}

/// 读取失败分两类：结构损坏可尝试 `.bak`，数据不合规则必须原样报错。
#[derive(Debug)]
enum ReadDocError {
    Structural(String),
    Invalid(String),
}

impl ReadDocError {
    fn message(&self) -> &str {
        match self {
            Self::Structural(message) | Self::Invalid(message) => message,
        }
    }

    fn is_structural(&self) -> bool {
        matches!(self, Self::Structural(_))
    }
}

impl ProjectItemsStore {
    /// 由应用数据根构造：`<数据根>/project-items.v1.json`。
    pub fn resolve(paths: &AppPaths) -> Result<Self, String> {
        let store = Self::for_path(paths.root.join(PROJECT_ITEMS_FILE));
        store.assert_allowed()?;
        Ok(store)
    }

    /// 直接指定文件路径（测试用；不创建目录）。
    pub fn for_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn backup_path(&self) -> PathBuf {
        let mut raw = self.path.clone().into_os_string();
        raw.push(".bak");
        PathBuf::from(raw)
    }

    fn assert_allowed(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| format!("存储路径没有父目录：{}", self.path.display()))?;
        assert_local_root_allowed(parent)?;
        assert_outside_repo(parent)?;
        let resolved = resolve_real_path(parent)?;
        assert_local_root_allowed(&resolved)?;
        assert_outside_repo(&resolved)?;
        Ok(())
    }

    /// 读回事项清单。文件不存在时返回空清单（首次使用，不是错误）。
    pub fn load(&self) -> Result<ProjectItemsListing, String> {
        self.assert_allowed()?;
        let backup = self.backup_path();
        if !self.path.exists() {
            return Ok(ProjectItemsListing {
                path: self.path.display().to_string(),
                items: Vec::new(),
                note: Some("还没有保存过项目事项（首次保存时创建文件）".to_string()),
                recovered_from_backup: false,
            });
        }
        match self.read_doc(&self.path) {
            Ok(doc) => Ok(ProjectItemsListing {
                path: self.path.display().to_string(),
                items: doc.items,
                note: None,
                recovered_from_backup: false,
            }),
            Err(primary_error) => {
                // 主文件结构损坏才回退；条目校验失败属于数据问题，直接报出，
                // 避免拿旧备份悄悄覆盖用户刚看到的内容。
                if !primary_error.is_structural() || !backup.exists() {
                    return Err(primary_error.message().to_string());
                }
                let doc = self.read_doc(&backup).map_err(|backup_error| {
                    format!(
                        "项目事项文件损坏且备份也读不回来。主文件：{}；备份：{}",
                        primary_error.message(),
                        backup_error.message()
                    )
                })?;
                Ok(ProjectItemsListing {
                    path: self.path.display().to_string(),
                    items: doc.items,
                    note: Some(
                        "主文件损坏，已从 .bak 备份读回；下次保存会重写主文件并重新备份。"
                            .to_string(),
                    ),
                    recovered_from_backup: true,
                })
            }
        }
    }

    fn read_doc(&self, path: &Path) -> Result<ProjectItemsDoc, ReadDocError> {
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| {
                ReadDocError::Structural(format!(
                    "读取项目事项文件失败（{}）：{}",
                    path.display(),
                    e
                ))
            })?;
        if !meta.file_type().is_file() {
            return Err(ReadDocError::Structural(format!(
                "项目事项路径不是普通文件，拒绝读取：{}",
                path.display()
            )));
        }
        if meta.len() > PROJECT_ITEMS_MAX_BYTES {
            return Err(ReadDocError::Structural(format!(
                "项目事项文件 {} 字节，超过上限 {} 字节，拒绝读取。",
                meta.len(),
                PROJECT_ITEMS_MAX_BYTES
            )));
        }
        let raw = std::fs::read_to_string(path).map_err(|e| {
            ReadDocError::Structural(format!(
                "读取项目事项文件失败（{}）：{}",
                path.display(),
                e
            ))
        })?;
        let doc: ProjectItemsDoc = serde_json::from_str(&raw).map_err(|e| {
            ReadDocError::Structural(format!(
                "解析项目事项文件失败（{}）：{}",
                path.display(),
                e
            ))
        })?;
        if doc.schema != PROJECT_ITEMS_SCHEMA {
            return Err(ReadDocError::Invalid(format!(
                "项目事项文件的 schema 是「{}」，不是「{}」，拒绝读取。",
                doc.schema, PROJECT_ITEMS_SCHEMA
            )));
        }
        if doc.items.len() > PROJECT_ITEMS_MAX_COUNT {
            return Err(ReadDocError::Invalid(format!(
                "项目事项共有 {} 条，超过上限 {} 条，拒绝读取。",
                doc.items.len(),
                PROJECT_ITEMS_MAX_COUNT
            )));
        }
        validate_items(&doc.items).map_err(ReadDocError::Invalid)?;
        Ok(doc)
    }

    /// 保存整份清单：先整批校验，再备份旧文件、原子写入新文件。
    ///
    /// `now` 由调用方传入（Unix 秒），便于测试与审计；同一份输入 + 同一时刻结果一致。
    /// 已存在条目的 `created_at` 会沿用旧值，其余重新赋值。
    pub fn save(
        &self,
        items: Vec<ProjectItemRecord>,
        now: f64,
    ) -> Result<ProjectItemsSaveReceipt, String> {
        self.assert_allowed()?;
        if items.len() > PROJECT_ITEMS_MAX_COUNT {
            return Err(format!(
                "项目事项共有 {} 条，超过上限 {} 条，拒绝保存。",
                items.len(),
                PROJECT_ITEMS_MAX_COUNT
            ));
        }
        validate_items(&items)?;

        let previous = self.read_previous_for_timestamps();
        let items: Vec<ProjectItemRecord> = items
            .into_iter()
            .map(|mut item| {
                let created = previous
                    .iter()
                    .find(|p| p.item == item.item)
                    .map(|p| p.created_at)
                    .filter(|v| *v > 0.0)
                    .unwrap_or(now);
                item.created_at = created;
                item.updated_at = now;
                item
            })
            .collect();

        let doc = ProjectItemsDoc {
            schema: PROJECT_ITEMS_SCHEMA.to_string(),
            items,
        };
        let encoded = serde_json::to_vec_pretty(&doc)
            .map_err(|e| format!("序列化项目事项失败：{}", e))?;
        if encoded.len() as u64 > PROJECT_ITEMS_MAX_BYTES {
            return Err(format!(
                "项目事项文件 {} 字节，超过上限 {} 字节，拒绝保存。",
                encoded.len(),
                PROJECT_ITEMS_MAX_BYTES
            ));
        }

        // 旧文件先备份；只有真正可解析的旧文件才值得备份，坏文件不覆盖已有 .bak。
        if self.path.exists() && self.read_doc(&self.path).is_ok() {
            let backup = self.backup_path();
            if backup.exists() && !std::fs::symlink_metadata(&backup).map(|m| m.file_type().is_file()).unwrap_or(false)
            {
                return Err(format!(
                    "备份路径不是普通文件，拒绝写入：{}",
                    backup.display()
                ));
            }
            std::fs::copy(&self.path, &backup)
                .map_err(|e| format!("备份项目事项文件失败（{}）：{}", backup.display(), e))?;
        }

        let dir = self
            .path
            .parent()
            .ok_or_else(|| format!("存储路径没有父目录：{}", self.path.display()))?;
        write_atomic(dir, &self.path, &encoded)?;
        Ok(ProjectItemsSaveReceipt {
            path: self.path.display().to_string(),
            bytes: encoded.len() as u64,
            items: doc.items,
        })
    }

    /// 仅用于沿用 `created_at`；读不回来就不沿用（按新条目处理），不阻断保存。
    fn read_previous_for_timestamps(&self) -> Vec<ProjectItemRecord> {
        if !self.path.exists() {
            return Vec::new();
        }
        self.read_doc(&self.path)
            .map(|doc| doc.items)
            .unwrap_or_default()
    }
}

/// 逐条校验；错误带「第 N 条」，整批拒绝。
pub fn validate_items(items: &[ProjectItemRecord]) -> Result<(), String> {
    for (index, item) in items.iter().enumerate() {
        let line_no = index + 1;
        let prefix = format!("第 {} 条", line_no);
        let fail = |message: &str| -> String { format!("{}：{}", prefix, message) };
        for (label, value) in [
            ("事项", item.item.as_str()),
            ("负责人代号", item.owner.as_str()),
            ("下一步", item.next_step.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(fail(&format!("{}不能为空", label)));
            }
            if value.chars().count() > MAX_FIELD_CHARS {
                return Err(fail(&format!("{}超过 {} 字", label, MAX_FIELD_CHARS)));
            }
        }
        // 换行会把一条事项撑成多行，界面与后续导出都不可预期，明确拒绝。
        if item.item.contains(['\n', '\r']) || item.next_step.contains(['\n', '\r']) {
            return Err(fail("事项与下一步不能包含换行"));
        }
        validate_project_owner(line_no, &item.owner)
            .map_err(|_| fail("负责人只填代号（例如 P1），不得填写真实姓名"))?;
        let due = validate_project_due(line_no, &item.due).map_err(|e| as_item_error(line_no, e))?;
        validate_project_start(line_no, item.start.as_deref(), due)
            .map_err(|e| as_item_error(line_no, e))?;
        validate_project_status(line_no, &item.status).map_err(|e| as_item_error(line_no, e))?;
        for (label, value) in [
            ("事项", item.item.as_str()),
            ("下一步", item.next_step.as_str()),
        ] {
            if value != value.trim() {
                return Err(fail(&format!("{}首尾不能有空白", label)));
            }
        }
    }
    for (index, item) in items.iter().enumerate() {
        if let Some(first) = items[..index].iter().position(|p| p.item == item.item) {
            return Err(format!(
                "第 {} 条：事项「{}」与第 {} 条重复（按事项名区分条目）",
                index + 1,
                item.item,
                first + 1
            ));
        }
    }
    Ok(())
}

/// 共用的项目校验函数按「行」报错；存储层按「条」报错，这里统一前缀。
fn as_item_error(line_no: usize, message: String) -> String {
    let line_prefix = format!("第 {} 行：", line_no);
    match message.strip_prefix(&line_prefix) {
        Some(rest) => format!("第 {} 条：{}", line_no, rest),
        None => message,
    }
}

/// 同目录临时文件 + rename，避免半写状态；失败时清理临时文件。
fn write_atomic(dir: &Path, target: &Path, bytes: &[u8]) -> Result<(), String> {
    let stamp = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let temp = dir.join(format!(".project-items-{}.tmp", stamp));
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temp)
            .map_err(|e| format!("写入项目事项临时文件失败（{}）：{}", temp.display(), e))?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                format!("写入项目事项临时文件失败（{}）：{}", temp.display(), e)
            })?;
    }
    std::fs::rename(&temp, target).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("替换项目事项文件失败（{}）：{}", target.display(), e)
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> (ProjectItemsStore, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "omnidesk-project-items-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        (
            ProjectItemsStore::for_path(base.join(PROJECT_ITEMS_FILE)),
            base,
        )
    }

    fn item(name: &str) -> ProjectItemRecord {
        ProjectItemRecord {
            item: name.to_string(),
            owner: "P1".to_string(),
            due: "2026-11-05".to_string(),
            start: None,
            status: "in_progress".to_string(),
            next_step: "先写纯函数".to_string(),
            created_at: 0.0,
            updated_at: 0.0,
        }
    }

    #[test]
    fn first_read_of_missing_file_is_empty_not_error() {
        let (store, base) = temp_store("empty");
        let listing = store.load().unwrap();
        assert!(listing.items.is_empty());
        assert!(listing.note.is_some());
        assert!(!base.join(PROJECT_ITEMS_FILE).exists());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn saves_and_reads_back_in_order() {
        let (store, base) = temp_store("roundtrip");
        let items = vec![item("合成：一"), item("合成：二")];
        let receipt = store.save(items.clone(), 1000.0).unwrap();
        assert_eq!(receipt.items.len(), 2);
        let listing = store.load().unwrap();
        assert_eq!(listing.items[0].item, "合成：一");
        assert_eq!(listing.items[1].item, "合成：二");
        assert_eq!(listing.items[0].created_at, 1000.0);
        assert!(listing.note.is_none());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn invalid_item_rejects_whole_batch() {
        let (store, base) = temp_store("invalid");
        let mut bad = item("合成：坏");
        bad.owner = "张三".to_string();
        let err = store.save(vec![item("合成：好"), bad], 1000.0).unwrap_err();
        assert!(err.contains("第 2 条"), "{}", err);
        assert!(!base.join(PROJECT_ITEMS_FILE).exists());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn duplicate_item_names_are_refused() {
        let (store, base) = temp_store("duplicate");
        let err = store
            .save(vec![item("合成：同"), item("合成：同")], 1000.0)
            .unwrap_err();
        assert!(err.contains("重复"), "{}", err);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn corrupt_main_file_falls_back_to_backup() {
        let (store, base) = temp_store("recover");
        store.save(vec![item("合成：备份里")], 1000.0).unwrap();
        // 再存一次，生成 .bak（内容为上一版）。
        store.save(vec![item("合成：备份里"), item("合成：新的")], 2000.0).unwrap();
        assert!(store.backup_path().exists());
        std::fs::write(store.path(), b"{ not json").unwrap();
        let listing = store.load().unwrap();
        assert!(listing.recovered_from_backup);
        assert_eq!(listing.items.len(), 1);
        assert_eq!(listing.items[0].item, "合成：备份里");
        // 再存一次可恢复：主文件被重写，备份保存上一版可读内容。
        store.save(listing.items.clone(), 3000.0).unwrap();
        assert!(!store.load().unwrap().recovered_from_backup);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn wrong_schema_is_refused() {
        let (store, base) = temp_store("schema");
        std::fs::write(
            store.path(),
            br#"{"schema":"other.v9","items":[]}"#,
        )
        .unwrap();
        let err = store.load().unwrap_err();
        assert!(err.contains("schema"), "{}", err);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn invalid_item_is_refused_even_when_backup_exists() {
        let (store, base) = temp_store("invalid-not-recovered");
        store.save(vec![item("合成：备份里")], 1000.0).unwrap();
        store.save(vec![item("合成：备份里"), item("合成：新的")], 2000.0).unwrap();
        assert!(store.backup_path().exists());
        std::fs::write(
            store.path(),
            r#"{"schema":"omnidesk.project-items.v1","items":[{"item":"合成：坏","owner":"张三","due":"2026-11-05","status":"in_progress","next_step":"先写纯函数"}]}"#
                .as_bytes(),
        )
        .unwrap();
        let err = store.load().unwrap_err();
        assert!(err.contains("第 1 条"), "{}", err);
        assert!(err.contains("负责人"), "{}", err);
        assert!(!err.contains("从 .bak"), "{}", err);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn wrong_schema_is_refused_even_when_backup_exists() {
        let (store, base) = temp_store("schema-not-recovered");
        store.save(vec![item("合成：备份里")], 1000.0).unwrap();
        store.save(vec![item("合成：备份里"), item("合成：新的")], 2000.0).unwrap();
        assert!(store.backup_path().exists());
        std::fs::write(store.path(), br#"{"schema":"other.v9","items":[]}"#).unwrap();
        let err = store.load().unwrap_err();
        assert!(err.contains("schema"), "{}", err);
        assert!(!err.contains("从 .bak"), "{}", err);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn created_at_is_preserved_and_updated_at_moves() {
        let (store, base) = temp_store("timestamps");
        store.save(vec![item("合成：甲")], 1000.0).unwrap();
        let mut renamed = item("合成：甲");
        renamed.status = "done".to_string();
        let receipt = store.save(vec![renamed], 2000.0).unwrap();
        assert_eq!(receipt.items[0].created_at, 1000.0);
        assert_eq!(receipt.items[0].updated_at, 2000.0);
        let _ = std::fs::remove_dir_all(base);
    }
}
