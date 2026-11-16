use std::error::Error;

// ANSI terminal colors
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
use owo_colors::OwoColorize;

/// Print the last `limit` commits of the repository at `path`.
pub fn print_history(path: &str, limit: usize) -> Result<Vec<String>, Box<dyn Error>> {
    let repo = gix::discover(path)?;
    let head = repo.head_commit()?;
    let mut history: Vec<String> = Vec::new();

    for info in repo.rev_walk([head.id]).all()?.take(limit) {
        let commit = info?.object()?;
        let author = commit.author()?;
        let summary = commit.message()?.summary();

        history.push(format!("{} {} — {}", commit.id.to_hex_with_len(7), author.name, summary));
    }

    Ok(history)
}

pub fn diff(repo: &gix::Repository) -> Result<String, Box<dyn Error>> {
    let head = repo.head_commit()?;
    let new_tree = head.tree()?;

    // Older tree: the first parent of HEAD (or use rev_parse_single("HEAD~3") etc.)
    let parent_id = head.parent_ids().next().ok_or("HEAD has no parent")?;
    let old_tree = parent_id.object()?.into_commit().tree()?;

    let changes = repo.diff_tree_to_tree(&old_tree, &new_tree, gix::diff::Options::default())?;

    let mut out = String::new();
    for change in changes {
        use gix::object::tree::diff::ChangeDetached as C;
        // Blob ids before/after the change (None = file didn't exist on that side).
        let (kind, old_id, new_id) = match &change {
            C::Addition { id, .. } => ("A", None, Some(*id)),
            C::Deletion { id, .. } => ("D", Some(*id), None),
            C::Modification { previous_id, id, .. } => ("M", Some(*previous_id), Some(*id)),
            C::Rewrite { source_id, id, .. } => ("R", Some(*source_id), Some(*id)),
        };
        out.push_str(&format!("{BOLD}{kind} {}{RESET}\n", change.location()));

        let old = blob_text(&repo, old_id)?;
        let new = blob_text(&repo, new_id)?;
        let text_diff = similar::TextDiff::from_lines(&old, &new);
        for hunk in text_diff.unified_diff().context_radius(3).iter_hunks() {
            out.push_str(&format!("{CYAN}{}{RESET}\n", hunk.header()));
            for line in hunk.iter_changes() {
                let (color, sign) = match line.tag() {
                    similar::ChangeTag::Insert => (GREEN, "+"),
                    similar::ChangeTag::Delete => (RED, "-"),
                    similar::ChangeTag::Equal => ("", " "),
                };
                out.push_str(&format!("{color}{sign}{line}{RESET}"));
                if line.missing_newline() {
                    out.push('\n');
                }
            }
        }
    }
    Ok(out)
}

/// Read a blob as text; empty if missing or not valid UTF-8 (e.g. binary files).
fn blob_text(repo: &gix::Repository, id: Option<gix::ObjectId>) -> Result<String, Box<dyn Error>> {
    let Some(id) = id else { return Ok(String::new()) };
    let blob = repo.find_object(id)?;
    Ok(String::from_utf8(blob.data.clone()).unwrap_or_default())
}
