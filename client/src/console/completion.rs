use std::fs;

pub(super) fn complete(buffer: &str) -> Vec<String> {
    let mut completions = Vec::new();
    let input = buffer.trim_start();
    if let Some(command_end) = input.find(char::is_whitespace) {
        if &input[..command_end] == "load" {
            let argument = input[command_end..].trim_start();
            let path = argument.strip_prefix('"').unwrap_or(argument);
            let component_start = path.rfind(['/', '\\']).map_or(0, |index| index + 1);
            let (directory, prefix) = path.split_at(component_start);
            if let Ok(entries) = fs::read_dir(if directory.is_empty() { "." } else { directory }) {
                for entry in entries.flatten() {
                    let Ok(file_type) = entry.file_type() else { continue };
                    if !file_type.is_dir() && !entry.path().extension().is_some_and(|extension| extension.eq_ignore_ascii_case("js")) {
                        continue;
                    }
                    let name = entry.file_name();
                    let Some(name) = name.to_str() else { continue };
                    if name.get(..prefix.len()).is_some_and(|start| start.eq_ignore_ascii_case(prefix)) {
                        completions.push(format!("{}{}", &name[prefix.len()..], if argument.starts_with('"') && !file_type.is_dir() { "\"" } else { "" }));
                    }
                }
            }
            completions.sort();
        }
    } else {
        completions.extend(["load", "reload", "resume", "capture", "clear", "exit"]
            .into_iter()
            .filter_map(|command| command.strip_prefix(input).map(str::to_owned)));
    }
    completions
}
