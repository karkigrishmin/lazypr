#![allow(dead_code)]

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;

use super::store::store_path;
use crate::core::ReviewNote;

/// Sanitize a branch name for use in a filename by replacing `/` with `__`.
fn sanitize_branch(branch: &str) -> String {
    branch.replace('/', "__")
}

/// Return the path to the notes JSON file for a given branch.
pub fn notes_path(repo_root: &Path, branch: &str) -> PathBuf {
    store_path(repo_root)
        .join("reviews")
        .join(format!("{}_notes.json", sanitize_branch(branch)))
}

/// Generate a unique note ID.
///
/// Format: `{unix_millis}_{line_or_0}_{vec_len}` where `vec_len` is used to
/// disambiguate notes created within the same millisecond.
pub fn generate_note_id(line: Option<u32>, disambiguator: usize) -> String {
    let ts = Utc::now().timestamp_millis();
    let line_part = line.unwrap_or(0);
    format!("{}_{}_{}", ts, line_part, disambiguator)
}

/// Load notes for a branch. Returns an empty vec if the file does not exist.
pub fn load_notes(repo_root: &Path, branch: &str) -> Result<Vec<ReviewNote>> {
    let path = notes_path(repo_root, branch);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let notes: Vec<ReviewNote> =
        serde_json::from_str(&contents).with_context(|| format!("parsing {}", path.display()))?;
    Ok(notes)
}

/// Save all notes for a branch to disk. Creates the parent directory if needed.
pub fn save_notes(repo_root: &Path, branch: &str, notes: &[ReviewNote]) -> Result<()> {
    let path = notes_path(repo_root, branch);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(notes).context("serialising notes to JSON")?;
    std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Add a note in memory. Does NOT touch the filesystem.
///
/// Returns a clone of the newly created note.
pub fn add_note(
    notes: &mut Vec<ReviewNote>,
    file: &str,
    line: Option<u32>,
    content: &str,
) -> ReviewNote {
    let id = generate_note_id(line, notes.len());
    let note = ReviewNote {
        id,
        file: file.to_owned(),
        line,
        content: content.to_owned(),
        created_at: Utc::now(),
    };
    notes.push(note.clone());
    note
}

/// Delete a note by ID in memory. Returns `true` if a note was removed.
///
/// Does NOT touch the filesystem.
pub fn delete_note(notes: &mut Vec<ReviewNote>, id: &str) -> bool {
    let len_before = notes.len();
    notes.retain(|n| n.id != id);
    notes.len() < len_before
}

/// Return references to all notes attached to the given file.
pub fn notes_for_file<'a>(notes: &'a [ReviewNote], file: &str) -> Vec<&'a ReviewNote> {
    notes.iter().filter(|n| n.file == file).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_returns_empty_when_no_file() {
        let tmp = TempDir::new().unwrap();
        crate::state::store::init_store(tmp.path()).unwrap();
        let notes = load_notes(tmp.path(), "main").unwrap();
        assert!(notes.is_empty());
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        crate::state::store::init_store(tmp.path()).unwrap();
        let mut notes = Vec::new();
        add_note(&mut notes, "src/main.rs", Some(42), "check this logic");
        save_notes(tmp.path(), "main", &notes).unwrap();
        let loaded = load_notes(tmp.path(), "main").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].file, "src/main.rs");
        assert_eq!(loaded[0].line, Some(42));
        assert_eq!(loaded[0].content, "check this logic");
    }

    #[test]
    fn add_note_generates_unique_ids() {
        let mut notes = Vec::new();
        let n1 = add_note(&mut notes, "a.rs", Some(1), "first");
        let n2 = add_note(&mut notes, "a.rs", Some(2), "second");
        assert_ne!(n1.id, n2.id);
    }

    #[test]
    fn delete_note_removes_by_id() {
        let mut notes = Vec::new();
        let n1 = add_note(&mut notes, "a.rs", Some(1), "first");
        add_note(&mut notes, "a.rs", Some(2), "second");
        assert!(delete_note(&mut notes, &n1.id));
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].content, "second");
    }

    #[test]
    fn delete_note_returns_false_for_missing() {
        let mut notes = Vec::new();
        assert!(!delete_note(&mut notes, "nonexistent"));
    }

    #[test]
    fn notes_for_file_filters_correctly() {
        let mut notes = Vec::new();
        add_note(&mut notes, "a.rs", Some(1), "note on a");
        add_note(&mut notes, "b.rs", Some(2), "note on b");
        add_note(&mut notes, "a.rs", Some(3), "another on a");
        let a_notes = notes_for_file(&notes, "a.rs");
        assert_eq!(a_notes.len(), 2);
    }

    #[test]
    fn notes_path_sanitizes_branch_slashes() {
        let path = notes_path(Path::new("/repo"), "feature/my-branch");
        assert!(path
            .to_str()
            .unwrap()
            .contains("feature__my-branch_notes.json"));
    }

    #[test]
    fn generate_note_id_includes_line() {
        let id = generate_note_id(Some(42), 0);
        assert!(id.contains("_42_"));
    }

    #[test]
    fn generate_note_id_uses_zero_for_none() {
        let id = generate_note_id(None, 0);
        assert!(id.contains("_0_"));
    }
}
