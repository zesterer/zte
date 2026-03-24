use std::{fs, path::Path};

pub fn workspace_dir(mut dir: &Path) -> &Path {
    for p in dir.ancestors() {
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
    dir
}
