use assert_cmd::cargo;
use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;

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
