use std::{fs, path::PathBuf};

pub fn workspace_dir(path: PathBuf) -> PathBuf {
    let current_dir;
    let mut dir = match path.parent() {
        Some(dir) => dir,
        None => {
            current_dir = std::env::current_dir().expect("No cwd");
            current_dir.as_path()
        }
    };

    for p in path.ancestors() {
        if let Ok(mut entries) = fs::read_dir(&p)
            && entries.any(|e| {
                e.map_or(false, |e| {
                    let is_git_repo =
                        e.file_name() == ".git" && e.file_type().map_or(false, |t| t.is_dir());
                    let is_repo_dir =
                        e.file_name() == ".repo" && e.file_type().map_or(false, |t| t.is_dir());
                    is_git_repo || is_repo_dir
                })
            })
        {
            dir = p;
        }
    }

    dir.to_path_buf()
}
