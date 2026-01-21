use std::{fs, path::PathBuf};

pub fn workspace_dir(mut path: PathBuf) -> PathBuf {
    loop {
        if let Ok(mut entries) = fs::read_dir(&path)
            && entries.any(|e| {
                e.map_or(false, |e| {
                    e.file_name() == ".git" && e.file_type().map_or(false, |t| t.is_dir())
                })
            })
        {
            break path;
        } else if !path.pop() {
            break std::env::current_dir().expect("No cwd");
        }
    }
}
