use std::{
    fs,
    io,
    path::{Path, PathBuf},
};

pub(super) fn history_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("rev-idle")
        .join("script_history.txt")
}

pub(super) fn load_history(path: &Path) -> io::Result<Vec<PathBuf>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut history = Vec::new();
    for path in contents.lines().map(PathBuf::from).filter(|path| path.is_file()) {
        record(&mut history, path);
    }
    Ok(history)
}

pub(super) fn save_history(path: &Path, history: &[PathBuf]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        path,
        history
            .iter()
            .map(|path| path.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

pub(super) fn record(history: &mut Vec<PathBuf>, path: PathBuf) {
    history.retain(|entry| entry != &path);
    history.push(path);
    if history.len() > 10 {
        history.remove(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn history_persists_unique_existing_scripts_in_run_order() {
        let dir = std::env::temp_dir().join(format!(
            "rev-idle-script-history-test-{}-{}",
            std::process::id(),
            line!(),
        ));
        fs::create_dir_all(&dir).unwrap();
        let history_path = dir.join("script_history.txt");
        let first = dir.join("first.js");
        let second = dir.join("second.js");
        fs::write(&first, "export default function() {}").unwrap();
        fs::write(&second, "export default function() {}").unwrap();

        let mut history = load_history(&history_path).unwrap();
        record(&mut history, first.clone());
        record(&mut history, second.clone());
        record(&mut history, first.clone());
        save_history(&history_path, &history);

        assert_eq!(load_history(&history_path).unwrap(), vec![second.clone(), first.clone()]);

        fs::remove_file(second).unwrap();
        let history = load_history(&history_path).unwrap();
        assert_eq!(history, vec![first]);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn history_keeps_only_the_latest_ten_scripts() {
        let mut history = Vec::new();
        for index in 0..12 {
            record(&mut history, PathBuf::from(format!("script-{index}.js")));
        }

        assert_eq!(history.len(), 10);
        assert_eq!(history.first(), Some(&PathBuf::from("script-2.js")));
        assert_eq!(history.last(), Some(&PathBuf::from("script-11.js")));
    }

    #[test]
    fn unreadable_history_is_not_treated_as_an_empty_history() {
        let path = std::env::temp_dir().join(format!(
            "rev-idle-invalid-script-history-test-{}-{}",
            std::process::id(),
            line!(),
        ));
        fs::write(&path, [0xff]).unwrap();

        assert!(load_history(&path).is_err());

        fs::remove_file(path).unwrap();
    }
}
