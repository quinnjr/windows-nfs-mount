pub fn to_nfs_components(windows_path: &str) -> Vec<&str> {
    windows_path
        .split(|c| c == '\\' || c == '/')
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn to_nfs_path(components: &[&str]) -> String {
    if components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", components.join("/"))
    }
}

pub fn split_parent_name(windows_path: &str) -> Option<(Vec<&str>, &str)> {
    let components = to_nfs_components(windows_path);
    if components.is_empty() {
        return None;
    }
    let (parent, name) = components.split_at(components.len() - 1);
    Some((parent.to_vec(), name[0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_path() {
        assert_eq!(to_nfs_components("\\"), Vec::<&str>::new());
        assert_eq!(to_nfs_components("/"), Vec::<&str>::new());
    }

    #[test]
    fn single_component() {
        assert_eq!(to_nfs_components("\\folder"), vec!["folder"]);
    }

    #[test]
    fn nested_path() {
        assert_eq!(to_nfs_components("\\folder\\sub\\file.txt"), vec!["folder", "sub", "file.txt"]);
    }

    #[test]
    fn forward_slashes() {
        assert_eq!(to_nfs_components("/folder/file.txt"), vec!["folder", "file.txt"]);
    }

    #[test]
    fn to_nfs_path_root() {
        assert_eq!(to_nfs_path(&[]), "/");
    }

    #[test]
    fn to_nfs_path_nested() {
        assert_eq!(to_nfs_path(&["usr", "local", "bin"]), "/usr/local/bin");
    }

    #[test]
    fn split_parent_name_file() {
        let (parent, name) = split_parent_name("\\folder\\file.txt").unwrap();
        assert_eq!(parent, vec!["folder"]);
        assert_eq!(name, "file.txt");
    }

    #[test]
    fn split_parent_name_root_child() {
        let (parent, name) = split_parent_name("\\folder").unwrap();
        assert!(parent.is_empty());
        assert_eq!(name, "folder");
    }

    #[test]
    fn split_parent_name_root() {
        assert!(split_parent_name("\\").is_none());
    }
}
