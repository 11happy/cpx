use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Component;
use std::{
    borrow::Cow,
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct ExcludeRules {
    pub absolute_paths: Vec<PathBuf>,
    pub basenames: HashSet<String>,
    pub glob_set: Option<GlobSet>,
}

pub enum ExcludePattern {
    AbsolutePath(PathBuf),
    BaseName(String),
    GlobPattern(String),
}

impl ExcludePattern {
    pub fn from_string(pattern: &str) -> Self {
        let trimmed = pattern.trim();
        if Path::new(trimmed).is_absolute() {
            return ExcludePattern::AbsolutePath(PathBuf::from(trimmed));
        }
        let has_glob_chars = trimmed.contains('*')
            || trimmed.contains('?')
            || trimmed.contains('[')
            || trimmed.contains(']');
        let has_path_sep = trimmed.contains('/') || trimmed.contains('\\');
        if has_glob_chars || has_path_sep {
            ExcludePattern::GlobPattern(trimmed.to_string())
        } else {
            ExcludePattern::BaseName(trimmed.to_string())
        }
    }
}

pub fn parse_exclude_pattern_list(input: &str) -> Result<Vec<ExcludePattern>, String> {
    let mut patterns = Vec::new();

    for raw in input.split(',') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let path = Path::new(trimmed);
        for component in path.components() {
            if matches!(component, Component::ParentDir) {
                return Err(format!(
                    "Invalid exclude pattern '{}': parent directory references (..) are not allowed",
                    trimmed
                ));
            }
        }

        patterns.push(ExcludePattern::from_string(trimmed));
    }

    Ok(patterns)
}

pub fn build_exclude_rules(patterns: Vec<ExcludePattern>) -> Result<Option<ExcludeRules>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut absolute_paths = Vec::new();
    let mut basenames = HashSet::new();
    let mut glob_builder = GlobSetBuilder::new();
    let mut has_globs = false;
    for pattern in patterns {
        match pattern {
            ExcludePattern::AbsolutePath(path) => {
                let canonical = path.canonicalize().unwrap_or(path);
                absolute_paths.push(canonical);
            }
            ExcludePattern::BaseName(name) => {
                basenames.insert(name);
            }
            ExcludePattern::GlobPattern(pattern) => {
                let glob = Glob::new(&pattern)
                    .map_err(|e| format!("Invalid glob '{}': {}", pattern, e))?;
                glob_builder.add(glob);
                has_globs = true;
            }
        }
    }
    absolute_paths.sort_unstable_by_key(|b| std::cmp::Reverse(b.as_os_str().len()));
    let glob_set = if has_globs {
        Some(
            glob_builder
                .build()
                .map_err(|e| format!("Glob build failed: {}", e))?,
        )
    } else {
        None
    };
    Ok(Some(ExcludeRules {
        absolute_paths,
        basenames,
        glob_set,
    }))
}

pub fn should_exclude(path: &Path, source_root: &Path, rules: &ExcludeRules) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str())
        && rules.basenames.contains(name)
    {
        return true;
    }
    if !rules.absolute_paths.is_empty() {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        for excluded in &rules.absolute_paths {
            if canonical == *excluded
                || (canonical.starts_with(excluded)
                    && canonical.components().count() > excluded.components().count())
            {
                return true;
            }
        }
    }
    if let Some(glob_set) = &rules.glob_set {
        let relative = path.strip_prefix(source_root).unwrap_or(path);
        let mut rel_str: Cow<str> = relative.to_string_lossy();
        if rel_str.contains('\\') {
            rel_str = Cow::Owned(rel_str.replace('\\', "/"));
        }
        if glob_set.is_match(&*rel_str) {
            return true;
        }
        if path.is_dir() {
            let mut with_slash = String::with_capacity(rel_str.len() + 1);
            with_slash.push_str(&rel_str);
            with_slash.push('/');
            if glob_set.is_match(&with_slash) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod exclude_tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_file(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = File::create(path).unwrap();
        f.write_all(content).unwrap();
    }

    #[test]
    fn test_exclude_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        create_file(&file_path, b"hello");

        let rules = build_exclude_rules(vec![ExcludePattern::AbsolutePath(file_path.clone())])
            .unwrap()
            .unwrap();

        assert!(should_exclude(&file_path, temp_dir.path(), &rules));
    }

    #[test]
    fn test_exclude_basename() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("node_modules").join("file.js");
        create_file(&file_path, b"console.log('hi')");

        let rules = build_exclude_rules(vec![ExcludePattern::BaseName("node_modules".to_string())])
            .unwrap()
            .unwrap();
        let rules_ref = &rules;
        assert!(should_exclude(
            file_path.parent().unwrap(),
            temp_dir.path(),
            rules_ref
        ));
        assert!(!should_exclude(&file_path, temp_dir.path(), rules_ref)); // only parent folder excluded
    }

    #[test]
    fn test_exclude_glob_pattern_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("temp123.tmp");
        create_file(&file_path, b"data");

        let rules = build_exclude_rules(vec![ExcludePattern::GlobPattern("*.tmp".to_string())])
            .unwrap()
            .unwrap();

        assert!(should_exclude(&file_path, temp_dir.path(), &rules));
    }

    #[test]
    fn test_exclude_glob_pattern_dir() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().join("build");
        fs::create_dir_all(&dir_path).unwrap();

        let rules = build_exclude_rules(vec![ExcludePattern::GlobPattern("build/".to_string())])
            .unwrap()
            .unwrap();

        assert!(should_exclude(&dir_path, temp_dir.path(), &rules));
    }

    #[test]
    fn test_exclude_mixed_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let abs_file = temp_dir.path().join("exclude_me.txt");
        let base_file = temp_dir.path().join("node_modules").join("file.js");
        let glob_file = temp_dir.path().join("temp.tmp");

        create_file(&abs_file, b"abs");
        create_file(&base_file, b"base");
        create_file(&glob_file, b"glob");

        let rules = build_exclude_rules(vec![
            ExcludePattern::AbsolutePath(abs_file.clone()),
            ExcludePattern::BaseName("node_modules".to_string()),
            ExcludePattern::GlobPattern("*.tmp".to_string()),
        ])
        .unwrap()
        .unwrap();
        let rules_ref = &rules;
        assert!(should_exclude(&abs_file, temp_dir.path(), rules_ref));
        assert!(should_exclude(
            base_file.parent().unwrap(),
            temp_dir.path(),
            rules_ref
        ));
        assert!(should_exclude(&glob_file, temp_dir.path(), rules_ref));
    }

    #[test]
    fn test_exclude_relative_paths() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("dir").join("file.txt");
        create_file(&file_path, b"hello");

        let rules = build_exclude_rules(vec![ExcludePattern::GlobPattern(
            "dir/file.txt".to_string(),
        )])
        .unwrap()
        .unwrap();

        assert!(should_exclude(&file_path, temp_dir.path(), &rules));
    }

    #[test]
    fn test_exclude_not_matching() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("keep.txt");
        create_file(&file_path, b"keep");

        let rules = build_exclude_rules(vec![
            ExcludePattern::GlobPattern("*.tmp".to_string()),
            ExcludePattern::BaseName("node_modules".to_string()),
        ])
        .unwrap()
        .unwrap();

        assert!(!should_exclude(&file_path, temp_dir.path(), &rules));
    }

    #[test]
    fn test_exclude_directory_with_slash_glob() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().join("build");
        fs::create_dir_all(&dir_path).unwrap();

        let rules = build_exclude_rules(vec![ExcludePattern::GlobPattern("build/".to_string())])
            .unwrap()
            .unwrap();

        assert!(should_exclude(&dir_path, temp_dir.path(), &rules));
    }
}
