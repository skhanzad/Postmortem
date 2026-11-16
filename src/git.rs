use std::error::Error;

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
