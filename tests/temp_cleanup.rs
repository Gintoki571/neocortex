//! The shared `TempDir` guard removes its directory on drop, also on panic.

mod common;
use common::TempDir;

#[test]
fn temp_dir_guard_removes_its_directory() {
    let dir = TempDir::new("guard-check");
    let path = dir.to_path_buf();
    std::fs::write(dir.join("file.txt"), b"x").unwrap();
    assert!(path.join("file.txt").exists());
    drop(dir);
    assert!(!path.exists(), "{} survived its guard", path.display());

    let panicked = std::panic::catch_unwind(|| {
        let dir = TempDir::new("guard-panic");
        std::fs::write(dir.join("file.txt"), b"x").unwrap();
        let path = dir.to_path_buf();
        assert!(path.exists());
        // Hand the path out through the panic payload so the check runs after unwinding.
        std::panic::panic_any(path);
    });
    let path = *panicked
        .expect_err("closure must panic")
        .downcast::<std::path::PathBuf>()
        .unwrap();
    assert!(!path.exists(), "{} survived a panic", path.display());
}
