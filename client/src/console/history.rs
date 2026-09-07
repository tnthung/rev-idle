use std::{fs, path::{Path, PathBuf}};

const HISTORY_LIMIT: usize = 500;

pub(super) fn history_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("rev-idle").join("console_history.txt")
}

pub(super) fn load_history(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|contents| {
            contents
                .lines()
                .map(str::to_owned)
                .filter(|line| !line.is_empty() && line.trim() != "exit")
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn save_history(path: &Path, history: &[String]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, history.join("\n"));
}

pub(super) fn push_history(history: &mut Vec<String>, line: &str) -> bool {
    if line.trim().is_empty() || line.trim() == "exit" || history.last().is_some_and(|last| last == line) {
        return false;
    }
    history.push(line.to_owned());
    if history.len() > HISTORY_LIMIT {
        history.remove(0);
    }
    true
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!(
            "rev-idle-console-history-test-{}-{}",
            std::process::id(),
            line!(),
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("console_history.txt");

        assert!(load_history(&path).is_empty());

        let history = vec!["load a.js".to_string(), "reload".to_string()];
        save_history(&path, &history);

        assert_eq!(load_history(&path), history);

        fs::remove_dir_all(&dir).unwrap();
    }
}
