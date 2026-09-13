//! Publishing schema files as records: work out what differs, then write
//! only that (plan §4.5, "a publish step in the build").

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::schema::{SchemaError, SchemaFile, SCHEMA_COLLECTION};
use crate::identity::Did;
use crate::repo::write::{AppPasswordSession, WriteReceipt};
use crate::repo::{RepoClient, RepoError};

/// What publishing would do to one schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// The published record already equals the file.
    Unchanged,
    /// No record exists yet.
    Create,
    /// A record exists and differs; `current` is what is published now.
    Update { current: Value },
}

/// One schema and its planned action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub schema: SchemaFile,
    pub action: Action,
}

impl Entry {
    /// Whether applying the plan would write this entry.
    pub fn needs_write(&self) -> bool {
        !matches!(self.action, Action::Unchanged)
    }
}

/// Why schema files could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Schema {
        path: PathBuf,
        #[source]
        source: SchemaError,
    },
    #[error("no schema files in {0}")]
    Empty(PathBuf),
}

/// Load every `*.json` directly inside `dir` (subdirectories such as
/// `upstream/` are ignored), sorted by NSID. The file stem is the expected
/// NSID.
pub fn load_dir(dir: &Path) -> Result<Vec<SchemaFile>, LoadError> {
    let io = |path: &Path, source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut schemas = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let entry = entry.map_err(|e| io(dir, e))?;
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let expected = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        let text = std::fs::read_to_string(&path).map_err(|e| io(&path, e))?;
        let schema = SchemaFile::parse(&text, &expected).map_err(|source| LoadError::Schema {
            path: path.clone(),
            source,
        })?;
        schemas.push(schema);
    }
    if schemas.is_empty() {
        return Err(LoadError::Empty(dir.to_path_buf()));
    }
    schemas.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(schemas)
}

/// Compare each schema with what `did`'s repo currently publishes.
pub async fn plan(
    reader: &RepoClient,
    did: &Did,
    schemas: &[SchemaFile],
) -> Result<Vec<Entry>, RepoError> {
    let mut entries = Vec::with_capacity(schemas.len());
    for schema in schemas {
        let action = match reader
            .get_record::<Value>(did, SCHEMA_COLLECTION, &schema.id)
            .await
        {
            Ok(record) => {
                let mut current = record.value;
                if let Some(object) = current.as_object_mut() {
                    object.remove("$type");
                }
                if current == schema.value() {
                    Action::Unchanged
                } else {
                    Action::Update { current }
                }
            }
            Err(RepoError::RecordNotFound) => Action::Create,
            Err(err) => return Err(err),
        };
        entries.push(Entry {
            schema: schema.clone(),
            action,
        });
    }
    Ok(entries)
}

/// Write every entry that needs it. Returns what was written.
pub async fn apply(
    session: &AppPasswordSession,
    entries: &[Entry],
) -> Result<Vec<(String, WriteReceipt)>, RepoError> {
    let mut written = Vec::new();
    for entry in entries.iter().filter(|e| e.needs_write()) {
        let receipt = session
            .put_record(
                SCHEMA_COLLECTION,
                &entry.schema.id,
                &entry.schema.record_value(),
            )
            .await?;
        written.push((entry.schema.id.clone(), receipt));
    }
    Ok(written)
}

/// A unified-style line diff of two JSON values, pretty-printed. Small
/// and quadratic, which is fine for schema documents.
pub fn diff(current: &Value, desired: &Value) -> String {
    let a: Vec<String> = pretty(current).lines().map(str::to_owned).collect();
    let b: Vec<String> = pretty(desired).lines().map(str::to_owned).collect();
    // Longest common subsequence table.
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = String::new();
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            out.push_str("  ");
            out.push_str(&a[i]);
            i += 1;
            j += 1;
        } else if j < b.len() && (i >= a.len() || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push_str("+ ");
            out.push_str(&b[j]);
            j += 1;
        } else {
            out.push_str("- ");
            out.push_str(&a[i]);
            i += 1;
        }
        out.push('\n');
    }
    out
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_marks_changed_lines_only() {
        let a =
            serde_json::json!({"id": "x", "defs": {"main": {"type": "object", "required": ["a"]}}});
        let b = serde_json::json!({"id": "x", "defs": {"main": {"type": "object", "required": ["a", "b"]}}});
        let d = diff(&a, &b);
        let added: Vec<&str> = d
            .lines()
            .filter(|l| l.starts_with("+ "))
            .map(|l| l[2..].trim())
            .collect();
        assert_eq!(added, vec!["\"a\",", "\"b\""], "{d}");
        assert!(
            d.lines()
                .any(|l| l.starts_with("  ") && l.contains("\"id\": \"x\"")),
            "{d}"
        );
        assert!(
            !d.lines()
                .any(|l| l.starts_with("- ") && l.contains("\"id\"")),
            "{d}"
        );
        assert_eq!(
            diff(&a, &a)
                .lines()
                .filter(|l| !l.starts_with("  "))
                .count(),
            0
        );
    }

    #[test]
    fn loads_the_project_lexicons_and_skips_upstream() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lexicons");
        let schemas = load_dir(&dir).unwrap();
        let ids: Vec<&str> = schemas.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["at.eaten.preferences", "at.eaten.subject"]);
        assert!(matches!(
            load_dir(&dir.join("nope")),
            Err(LoadError::Io { .. })
        ));
    }
}
