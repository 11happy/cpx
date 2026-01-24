use assert_cmd::cargo;
use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

#[test]
fn test_copy_single_file() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("Hello, World!").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("Hello, World!");
}

#[test]
fn test_copy_single_file_to_directory() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest_dir = temp.child("dest");

    source.write_str("Test content").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source.txt").assert("Test content");
}

#[test]
fn test_copy_multiple_files_traditional() {
    let temp = assert_fs::TempDir::new().unwrap();
    let file1 = temp.child("file1.txt");
    let file2 = temp.child("file2.txt");
    let dest_dir = temp.child("dest");

    file1.write_str("Content 1").unwrap();
    file2.write_str("Content 2").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(file1.path())
        .arg(file2.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("file1.txt").assert("Content 1");
    dest_dir.child("file2.txt").assert("Content 2");
}

#[test]
fn test_copy_with_target_directory_flag() {
    let temp = assert_fs::TempDir::new().unwrap();
    let file1 = temp.child("file1.txt");
    let file2 = temp.child("file2.txt");
    let dest_dir = temp.child("dest");

    file1.write_str("Content 1").unwrap();
    file2.write_str("Content 2").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-t")
        .arg(dest_dir.path())
        .arg(file1.path())
        .arg(file2.path())
        .assert()
        .success();

    dest_dir.child("file1.txt").assert("Content 1");
    dest_dir.child("file2.txt").assert("Content 2");
}

#[test]
fn test_copy_directory_without_recursive_flag() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir.child("file.txt").write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("directory"))
        .stderr(predicate::str::contains("-r"));
}

#[test]
fn test_copy_directory_recursive() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir.child("file1.txt").write_str("content1").unwrap();
    source_dir.child("file2.txt").write_str("content2").unwrap();

    let subdir = source_dir.child("subdir");
    subdir.create_dir_all().unwrap();
    subdir.child("file3.txt").write_str("content3").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source/file1.txt").assert("content1");
    dest_dir.child("source/file2.txt").assert("content2");
    dest_dir.child("source/subdir/file3.txt").assert("content3");
}

#[test]
fn test_copy_with_resume_flag() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest_dir = temp.child("dest");
    let dest = dest_dir.child("source.txt");

    source.write_str("Same content").unwrap();

    dest_dir.create_dir_all().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    dest.write_str("Same content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--resume")
        .arg(source.path())
        .arg(dest_dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skipping"));

    dest.assert("Same content");
}

#[test]
fn test_copy_with_force_flag() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("New content").unwrap();
    dest.write_str("Old content").unwrap();

    #[cfg(unix)]
    {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dest.path()).unwrap().permissions();
        perms.set_mode(0o444); // read-only
        fs::set_permissions(dest.path(), perms).unwrap();
    }

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-f")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("New content");
}

#[test]
fn test_copy_with_concurrency() {
    let temp = assert_fs::TempDir::new().unwrap();
    let dest_dir = temp.child("dest");
    dest_dir.create_dir_all().unwrap();

    let mut files = Vec::new();
    for i in 0..5 {
        let file = temp.child(format!("file{}.txt", i));
        file.write_str(&format!("Content {}", i)).unwrap();
        files.push(file);
    }

    let mut cmd = Command::new(cargo::cargo_bin!("cpx"));
    cmd.arg("-j").arg("2").arg("-t").arg(dest_dir.path());

    for file in &files {
        cmd.arg(file.path());
    }

    cmd.assert().success();

    for i in 0..5 {
        dest_dir
            .child(format!("file{}.txt", i))
            .assert(format!("Content {}", i));
    }
}

#[test]
fn test_invalid_source() {
    let temp = assert_fs::TempDir::new().unwrap();
    let dest = temp.child("dest.txt");

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("/nonexistent/file.txt")
        .arg(dest.path())
        .assert()
        .failure();
}

#[test]
fn test_missing_destination() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    source.write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_target_directory_must_exist() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    source.write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-t")
        .arg("/nonexistent/directory")
        .arg(source.path())
        .assert()
        .failure();
}

#[test]
fn test_copy_preserves_content_integrity() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.bin");
    let dest = temp.child("dest.bin");

    let binary_data: Vec<u8> = (0..=255).cycle().take(10240).collect();
    fs::write(source.path(), &binary_data).unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    let dest_data = fs::read(dest.path()).unwrap();
    assert_eq!(binary_data, dest_data, "Binary content should be preserved");
}

#[test]
fn test_copy_large_file() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("large.txt");
    let dest = temp.child("large_copy.txt");

    let large_content = "x".repeat(5 * 1024 * 1024);
    fs::write(source.path(), &large_content).unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    let dest_size = fs::metadata(dest.path()).unwrap().len();
    assert_eq!(dest_size, 5 * 1024 * 1024);
}

#[test]
#[cfg(unix)]
fn test_symlink_mode_auto_relative() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest_dir = temp.child("dest");

    source.write_str("content").unwrap();
    dest_dir.create_dir_all().unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-s")
        .arg("auto")
        .arg("source.txt")
        .arg("dest")
        .assert()
        .success();

    let symlink_path = temp.child("dest/source.txt");
    assert!(symlink_path.path().symlink_metadata().unwrap().is_symlink());

    let target = fs::read_link(symlink_path.path()).unwrap();
    assert!(!target.is_absolute());

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
#[cfg(unix)]
fn test_symlink_mode_absolute() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest_dir = temp.child("dest");

    source.write_str("content").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-s")
        .arg("absolute")
        .arg(source.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    let symlink_path = dest_dir.child("source.txt");
    let target = fs::read_link(symlink_path.path()).unwrap();
    assert!(target.is_absolute());
}

#[test]
#[cfg(unix)]
fn test_symlink_directory_recursive() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir.child("file1.txt").write_str("content1").unwrap();
    source_dir.child("file2.txt").write_str("content2").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-s")
        .arg("relative")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    assert!(
        dest_dir
            .child("source/file1.txt")
            .path()
            .symlink_metadata()
            .unwrap()
            .is_symlink()
    );
    assert!(
        dest_dir
            .child("source/file2.txt")
            .path()
            .symlink_metadata()
            .unwrap()
            .is_symlink()
    );
}

#[test]
#[cfg(unix)]
fn test_preserve_existing_symlink() {
    use std::os::unix::fs::symlink;

    let temp = assert_fs::TempDir::new().unwrap();
    let actual_file = temp.child("actual.txt");
    let source_link = temp.child("source_link");
    let dest_dir = temp.child("dest");

    actual_file.write_str("actual content").unwrap();
    dest_dir.create_dir_all().unwrap();

    symlink(actual_file.path(), source_link.path()).unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-P") // no-dereference
        .arg(source_link.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    let dest_link = dest_dir.child("source_link");
    assert!(dest_link.path().symlink_metadata().unwrap().is_symlink());

    let original_target = fs::read_link(source_link.path()).unwrap();
    let copied_target = fs::read_link(dest_link.path()).unwrap();
    assert_eq!(original_target, copied_target);
}

#[test]
#[cfg(unix)]
fn test_hardlink_single_file() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-l")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    let source_meta = fs::metadata(source.path()).unwrap();
    let dest_meta = fs::metadata(dest.path()).unwrap();

    assert_eq!(source_meta.ino(), dest_meta.ino());
    assert_eq!(source_meta.nlink(), 2);
}

#[test]
#[cfg(unix)]
fn test_hardlink_multiple_files() {
    let temp = assert_fs::TempDir::new().unwrap();
    let file1 = temp.child("file1.txt");
    let file2 = temp.child("file2.txt");
    let dest_dir = temp.child("dest");

    file1.write_str("content1").unwrap();
    file2.write_str("content2").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-l")
        .arg(file1.path())
        .arg(file2.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    let dest1_meta = fs::metadata(dest_dir.child("file1.txt").path()).unwrap();
    let dest2_meta = fs::metadata(dest_dir.child("file2.txt").path()).unwrap();

    assert_eq!(dest1_meta.nlink(), 2);
    assert_eq!(dest2_meta.nlink(), 2);
}

#[test]
#[cfg(unix)]
fn test_preserve_hardlinks() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();

    let original = source_dir.child("original.txt");
    original.write_str("content").unwrap();

    let hardlink = source_dir.child("hardlink.txt");
    fs::hard_link(original.path(), hardlink.path()).unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-p")
        .arg("links")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    let dest_orig = dest_dir.child("source/original.txt");
    let dest_link = dest_dir.child("source/hardlink.txt");

    let orig_meta = fs::metadata(dest_orig.path()).unwrap();
    let link_meta = fs::metadata(dest_link.path()).unwrap();

    assert_eq!(orig_meta.ino(), link_meta.ino());
    assert_eq!(orig_meta.nlink(), 2);
}

#[test]
fn test_backup_simple() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("new content").unwrap();
    dest.write_str("old content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-b")
        .arg("simple")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("new content");
    temp.child("dest.txt~").assert("old content");
}

#[test]
fn test_backup_numbered() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("version 1").unwrap();
    dest.write_str("version 0").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-b")
        .arg("numbered")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    temp.child("dest.txt.~1~").assert("version 0");

    source.write_str("version 2").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-b")
        .arg("numbered")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    temp.child("dest.txt.~2~").assert("version 1");
}

#[test]
fn test_backup_existing_mode() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("new").unwrap();
    dest.write_str("old").unwrap();

    // First backup with existing mode (no numbered backups exist)
    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-b")
        .arg("existing")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    temp.child("dest.txt~").assert("old");

    // Create a numbered backup manually
    fs::write(temp.child("dest.txt.~1~").path(), "numbered").unwrap();

    source.write_str("newer").unwrap();
    dest.write_str("new").unwrap();

    // Now it should use numbered
    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-b")
        .arg("existing")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    temp.child("dest.txt.~2~").assert("new");
}

#[test]
#[cfg(unix)]
fn test_preserve_mode() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("content").unwrap();

    let mut perms = fs::metadata(source.path()).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(source.path(), perms).unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-p")
        .arg("mode")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    let dest_mode = fs::metadata(dest.path()).unwrap().permissions().mode() & 0o777;
    assert_eq!(dest_mode, 0o755);
}

#[test]
fn test_preserve_timestamps() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("content").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-p")
        .arg("timestamps")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    let src_mtime = fs::metadata(source.path()).unwrap().modified().unwrap();
    let dest_mtime = fs::metadata(dest.path()).unwrap().modified().unwrap();

    let diff = if src_mtime > dest_mtime {
        src_mtime.duration_since(dest_mtime).unwrap()
    } else {
        dest_mtime.duration_since(src_mtime).unwrap()
    };

    assert!(diff.as_secs() < 2);
}

#[test]
fn test_attributes_only() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("source content").unwrap();
    dest.write_str("dest content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--attributes-only")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    // Content should not change
    dest.assert("dest content");
}

#[test]
fn test_exclude_basename() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir.child("file1.txt").write_str("keep").unwrap();

    let node_modules = source_dir.child("node_modules");
    node_modules.create_dir_all().unwrap();
    node_modules.child("lib.js").write_str("exclude").unwrap();

    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-e")
        .arg("node_modules")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source/file1.txt").assert("keep");
    assert!(!dest_dir.child("source/node_modules").path().exists());
}
#[test]
fn test_exclude_glob_pattern() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir.child("file.txt").write_str("keep").unwrap();
    source_dir.child("temp.tmp").write_str("exclude").unwrap();
    source_dir.child("cache.tmp").write_str("exclude").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-e")
        .arg("*.tmp")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source/file.txt").assert("keep");
    assert!(!dest_dir.child("source/temp.tmp").path().exists());
    assert!(!dest_dir.child("source/cache.tmp").path().exists());
}

#[test]
fn test_exclude_multiple_patterns() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir.child("keep.txt").write_str("keep").unwrap();
    source_dir.child("file.tmp").write_str("exclude").unwrap();
    source_dir.child("file.log").write_str("exclude").unwrap();
    source_dir.child(".git").create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-e")
        .arg("*.tmp,*.log,.git")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source/keep.txt").assert("keep");
    assert!(!dest_dir.child("source/file.tmp").path().exists());
    assert!(!dest_dir.child("source/file.log").path().exists());
    assert!(!dest_dir.child("source/.git").path().exists());
}

#[test]
fn test_exclude_relative_path() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir.child("subdir").create_dir_all().unwrap();
    source_dir
        .child("subdir/keep.txt")
        .write_str("keep")
        .unwrap();
    source_dir
        .child("subdir/exclude.txt")
        .write_str("exclude")
        .unwrap();
    source_dir.child("other.txt").write_str("keep").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-e")
        .arg("subdir/exclude.txt")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source/subdir/keep.txt").assert("keep");
    dest_dir.child("source/other.txt").assert("keep");
    assert!(!dest_dir.child("source/subdir/exclude.txt").path().exists());
}

#[test]
fn test_parents_flag() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("a/b/c");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    let source_file = source_dir.child("file.txt");
    source_file.write_str("content").unwrap();
    dest_dir.create_dir_all().unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--parents")
        .arg("a/b/c/file.txt")
        .arg("dest")
        .assert()
        .success();

    temp.child("dest/a/b/c/file.txt").assert("content");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_parents_multiple_files_absolute() {
    let temp = assert_fs::TempDir::new().unwrap();
    let dest_dir = temp.child("dest");
    dest_dir.create_dir_all().unwrap();

    let file1_dir = temp.child("dir1/sub1");
    file1_dir.create_dir_all().unwrap();
    let file1 = file1_dir.child("file1.txt");
    file1.write_str("content1").unwrap();

    let file2_dir = temp.child("dir2/sub2");
    file2_dir.create_dir_all().unwrap();
    let file2 = file2_dir.child("file2.txt");
    file2.write_str("content2").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--parents")
        .arg(file1.path())
        .arg(file2.path())
        .arg(dest_dir.path())
        .assert()
        .success();
    let file1_rel = file1.path().strip_prefix("/").unwrap();
    let file2_rel = file2.path().strip_prefix("/").unwrap();

    dest_dir.child(file1_rel).assert("content1");
    dest_dir.child(file2_rel).assert("content2");
}

#[test]
#[cfg(unix)]
fn test_dereference_command_line() {
    use std::os::unix::fs::symlink;

    let temp = assert_fs::TempDir::new().unwrap();
    let actual_dir = temp.child("actual");
    actual_dir.create_dir_all().unwrap();
    actual_dir.child("file.txt").write_str("content").unwrap();

    let symlink_dir = temp.child("link");
    symlink(actual_dir.path(), symlink_dir.path()).unwrap();

    let dest_dir = temp.child("dest");
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-H")
        .arg(symlink_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    // The symlink is dereferenced, so contents are copied
    dest_dir.child("link/file.txt").assert("content");
}

#[test]
#[cfg(unix)]
fn test_dereference_always() {
    let temp = assert_fs::TempDir::new().unwrap();
    let actual = temp.child("actual.txt");
    actual.write_str("content").unwrap();

    let source_dir = temp.child("source");
    source_dir.create_dir_all().unwrap();

    let link = source_dir.child("link.txt");
    symlink(actual.path(), link.path()).unwrap();

    let dest_dir = temp.child("dest");

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-L")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    let dest_file = dest_dir.child("source/link.txt");
    assert!(!dest_file.path().symlink_metadata().unwrap().is_symlink());
    dest_file.assert("content");
}

#[test]
fn test_symlink_hardlink_conflict() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-s")
        .arg("-l")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("symbolic-link").and(predicate::str::contains("link")));
}

#[test]
fn test_symlink_resume_conflict() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-s")
        .arg("--resume")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("continue"));
}

#[test]
fn test_dereference_flags_conflict() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-P")
        .arg("-L")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("only one"));
}

#[test]
fn test_copy_empty_file() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("empty.txt");
    let dest = temp.child("empty_copy.txt");

    source.write_str("").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    assert_eq!(fs::metadata(dest.path()).unwrap().len(), 0);
}

#[test]
fn test_copy_to_existing_directory() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest_dir = temp.child("dest");

    source.write_str("content").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source.txt").assert("content");
}

#[test]
fn test_copy_directory_to_file_fails() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_file = temp.child("dest.txt");

    source_dir.create_dir_all().unwrap();
    dest_file.write_str("existing").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg(source_dir.path())
        .arg(dest_file.path())
        .assert()
        .failure();
}

#[test]
fn test_copy_special_characters_in_filename() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("file with spaces & special!.txt");
    let dest_dir = temp.child("dest");

    source.write_str("content").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir
        .child("file with spaces & special!.txt")
        .assert("content");
}

#[test]
fn test_copy_nested_directories() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("a/b/c/d/e");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir
        .child("deep.txt")
        .write_str("deep content")
        .unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg(temp.child("a").path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("a/b/c/d/e/deep.txt").assert("deep content");
}

#[test]
fn test_remove_destination_flag() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("new").unwrap();
    dest.write_str("old").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--remove-destination")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("new");
}

#[test]
fn test_copy_very_long_filename() {
    let temp = assert_fs::TempDir::new().unwrap();
    let long_name = "a".repeat(200) + ".txt";
    let source = temp.child(&long_name);
    let dest_dir = temp.child("dest");

    source.write_str("content").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child(&long_name).assert("content");
}

#[test]
fn test_config_init() {
    let temp = assert_fs::TempDir::new().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("config")
        .arg("init")
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join(".config"))
        .assert()
        .success();

    let config_path = temp.path().join(".config/cpx/cpxconfig.toml");
    assert!(config_path.exists());

    let contents = fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("[exclude]"));
    assert!(contents.contains("[copy]"));
    assert!(contents.contains("[preserve]"));
}

#[test]
fn test_config_init_force_overwrite() {
    let temp = assert_fs::TempDir::new().unwrap();
    let config_dir = temp.path().join(".config/cpx");
    fs::create_dir_all(&config_dir).unwrap();

    let config_path = config_dir.join("cpxconfig.toml");
    fs::write(&config_path, "old config").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("config")
        .arg("init")
        .arg("--force")
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join(".config"))
        .assert()
        .success();

    let contents = fs::read_to_string(&config_path).unwrap();
    assert_ne!(contents, "old config");
}

#[test]
fn test_config_show() {
    Command::new(cargo::cargo_bin!("cpx"))
        .arg("config")
        .arg("show")
        .assert()
        .success();
}

#[test]
fn test_config_path() {
    Command::new(cargo::cargo_bin!("cpx"))
        .arg("config")
        .arg("path")
        .assert()
        .success();
}

#[test]
fn test_no_config_flag() {
    let temp = assert_fs::TempDir::new().unwrap();
    let config_dir = temp.path().join(".config/cpx");
    fs::create_dir_all(&config_dir).unwrap();

    let config_path = config_dir.join("cpxconfig.toml");
    fs::write(
        &config_path,
        r#"
[copy]
force = true
"#,
    )
    .unwrap();

    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("new").unwrap();
    dest.write_str("old").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dest.path()).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(dest.path(), perms).unwrap();
    }

    // With --no-config, should fail without force
    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--no-config")
        .arg(source.path())
        .arg(dest.path())
        .env("HOME", temp.path())
        .env("XDG_CONFIG_HOME", temp.path().join(".config"))
        .assert()
        .failure();
}

#[test]
fn test_resume_skips_identical_files() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    dest_dir.create_dir_all().unwrap();

    // Create files that are already copied
    source_dir.child("file1.txt").write_str("content1").unwrap();
    source_dir.child("file2.txt").write_str("content2").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    dest_dir.child("source").create_dir_all().unwrap();
    dest_dir
        .child("source/file1.txt")
        .write_str("content1")
        .unwrap();

    // Create a file that needs updating
    source_dir
        .child("file3.txt")
        .write_str("new content")
        .unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("--resume")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skipping 1"));
}

#[test]
fn test_resume_with_size_mismatch() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest_dir = temp.child("dest");

    source.write_str("new longer content").unwrap();

    dest_dir.create_dir_all().unwrap();
    let dest_file = dest_dir.child("source.txt");
    dest_file.write_str("old").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--resume")
        .arg(source.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_file.assert("new longer content");
}

#[test]
#[cfg(target_os = "linux")]
fn test_reflink_auto() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("reflink content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--reflink")
        .arg("auto")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("reflink content");
}

#[test]
#[cfg(target_os = "linux")]
fn test_reflink_never() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--reflink")
        .arg("never")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("content");
}

#[test]
fn test_copy_multiple_large_files() {
    let temp = assert_fs::TempDir::new().unwrap();
    let dest_dir = temp.child("dest");
    dest_dir.create_dir_all().unwrap();

    let size = 10 * 1024 * 1024; // 10MB each
    let content = vec![0u8; size];

    let mut files = Vec::new();
    for i in 0..3 {
        let file = temp.child(format!("large_{}.bin", i));
        fs::write(file.path(), &content).unwrap();
        files.push(file);
    }

    let mut cmd = Command::new(cargo::cargo_bin!("cpx"));
    cmd.arg("-j").arg("2").arg("-t").arg(dest_dir.path());

    for file in &files {
        cmd.arg(file.path());
    }

    cmd.assert().success();

    for i in 0..3 {
        let dest_file = dest_dir.child(format!("large_{}.bin", i));
        assert_eq!(fs::metadata(dest_file.path()).unwrap().len(), size as u64);
    }
}

#[test]
fn test_copy_file_with_different_buffer_sizes() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Test different file sizes to trigger different buffer sizes
    let test_sizes = vec![
        (500 * 1024, "small"),        // < 1MB
        (5 * 1024 * 1024, "medium"),  // 5MB
        (100 * 1024 * 1024, "large"), // 100MB
    ];

    for (size, name) in test_sizes {
        let source = temp.child(format!("source_{}.bin", name));
        let dest = temp.child(format!("dest_{}.bin", name));

        let content = vec![42u8; size];
        fs::write(source.path(), &content).unwrap();

        Command::new(cargo::cargo_bin!("cpx"))
            .arg(source.path())
            .arg(dest.path())
            .assert()
            .success();

        assert_eq!(fs::metadata(dest.path()).unwrap().len(), size as u64);
    }
}

#[test]
fn test_implicit_copy_command() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("implicit").unwrap();

    // Should work without explicit "copy" subcommand
    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("implicit");
}

#[test]
fn test_explicit_copy_command() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("dest.txt");

    source.write_str("explicit").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("copy")
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("explicit");
}

#[test]
fn test_help_flag() {
    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_version_flag() {
    Command::new(cargo::cargo_bin!("cpx"))
        .arg("--version")
        .assert()
        .success();
}

#[test]
fn test_copy_help() {
    Command::new(cargo::cargo_bin!("cpx"))
        .arg("copy")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("recursive"));
}

#[test]
#[cfg(unix)]
fn test_copy_readonly_source() {
    use std::os::unix::fs::PermissionsExt;

    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("readonly.txt");
    let dest = temp.child("dest.txt");

    source.write_str("readonly content").unwrap();

    let mut perms = fs::metadata(source.path()).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(source.path(), perms).unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .success();

    dest.assert("readonly content");
}

#[test]
fn test_destination_parent_not_exist() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("source.txt");
    let dest = temp.child("nonexistent/dir/dest.txt");

    source.write_str("content").unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest.path())
        .assert()
        .failure();
}

#[test]
fn test_copy_with_multiple_flags() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir.child("file1.txt").write_str("content1").unwrap();
    source_dir.child("file2.log").write_str("log").unwrap();
    source_dir.child("subdir").create_dir_all().unwrap();
    source_dir
        .child("subdir/file3.txt")
        .write_str("content3")
        .unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg("-f")
        .arg("-p")
        .arg("mode,timestamps")
        .arg("-e")
        .arg("*.log")
        .arg("-j")
        .arg("4")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source/file1.txt").assert("content1");
    dest_dir.child("source/subdir/file3.txt").assert("content3");
    assert!(!dest_dir.child("source/file2.log").path().exists());
}

#[test]
fn test_copy_dotfiles() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();
    source_dir
        .child(".hidden")
        .write_str("hidden content")
        .unwrap();
    source_dir.child(".config").create_dir_all().unwrap();
    source_dir
        .child(".config/app.conf")
        .write_str("config")
        .unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("source/.hidden").assert("hidden content");
    dest_dir.child("source/.config/app.conf").assert("config");
}

#[test]
fn test_copy_unicode_filenames() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source = temp.child("файл.txt"); // Cyrillic
    let dest_dir = temp.child("dest");

    source.write_str("unicode content").unwrap();
    dest_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg(source.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    dest_dir.child("файл.txt").assert("unicode content");
}

#[test]
fn test_copy_empty_directory() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("empty_source");
    let dest_dir = temp.child("dest");

    source_dir.create_dir_all().unwrap();

    Command::new(cargo::cargo_bin!("cpx"))
        .arg("-r")
        .arg(source_dir.path())
        .arg(dest_dir.path())
        .assert()
        .success();

    assert!(dest_dir.child("empty_source").path().exists());
    assert!(dest_dir.child("empty_source").path().is_dir());
}
