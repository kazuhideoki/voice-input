//! JSON ファイル版 DictRepository 実装
#[cfg(test)]
use crate::domain::dict::EntryStatus;
use crate::domain::dict::{DictRepository, WordEntry};
use crate::infrastructure::config::AppConfig;
use serde_json::{from_reader, to_writer_pretty};
use std::{fs, io::Result, path::PathBuf};

pub struct JsonFileDictRepo {
    path: PathBuf,
}

impl JsonFileDictRepo {
    pub fn new() -> Self {
        let cfg = AppConfig::load();
        let path = cfg.dict_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create data dir");
        }
        Self { path }
    }
}

impl DictRepository for JsonFileDictRepo {
    fn load(&self) -> Result<Vec<WordEntry>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        let f = fs::File::open(&self.path)?;
        Ok(from_reader::<_, Vec<WordEntry>>(f)?)
    }

    fn save(&self, all: &[WordEntry]) -> Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        {
            let f = fs::File::create(&tmp)?;
            to_writer_pretty(f, all)?;
        }
        fs::rename(tmp, &self.path)?;
        Ok(())
    }
}

// === Unit tests ==========================================================
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn repo_in_tmp() -> (JsonFileDictRepo, TempDir) {
        let tmp = TempDir::new().expect("create tempdir");
        let repo = JsonFileDictRepo {
            path: tmp.path().join("dictionary.json"),
        };
        (repo, tmp)
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let (repo, _tmp) = repo_in_tmp();
        let entries = repo.load().expect("load");
        assert!(entries.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (repo, _tmp) = repo_in_tmp();
        let list = vec![WordEntry {
            surface: "foo".into(),
            replacement: "bar".into(),
            hit: 1,
            status: EntryStatus::Active,
        }];
        repo.save(&list).expect("save");
        let loaded = repo.load().expect("load");
        assert_eq!(loaded.len(), list.len());
        assert_eq!(loaded[0].surface, list[0].surface);
        assert_eq!(loaded[0].replacement, list[0].replacement);
        assert_eq!(loaded[0].hit, list[0].hit);
    }

    #[test]
    fn upsert_adds_and_updates() {
        let (repo, _tmp) = repo_in_tmp();
        repo.upsert(WordEntry {
            surface: "foo".into(),
            replacement: "bar".into(),
            hit: 0,
            status: EntryStatus::Active,
        })
        .expect("upsert add");

        repo.upsert(WordEntry {
            surface: "foo".into(),
            replacement: "baz".into(),
            hit: 2,
            status: EntryStatus::Active,
        })
        .expect("upsert update");

        let loaded = repo.load().expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].surface, "foo");
        assert_eq!(loaded[0].replacement, "baz");
        assert_eq!(loaded[0].hit, 2);
    }

    #[test]
    fn delete_removes_entry() {
        let (repo, _tmp) = repo_in_tmp();
        repo.upsert(WordEntry {
            surface: "foo".into(),
            replacement: "bar".into(),
            hit: 0,
            status: EntryStatus::Active,
        })
        .expect("upsert");
        assert!(repo.delete("foo").expect("delete existing"));
        assert!(!repo.delete("foo").expect("delete missing"));
        let loaded = repo.load().expect("load");
        assert!(loaded.is_empty());
    }
}
