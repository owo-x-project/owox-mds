use std::fs;
use std::path::{Path, PathBuf};

fn should_skip_fixture_dir(name: &str) -> bool {
    matches!(
        name,
        ".build"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "coverage"
            | ".cache"
            | ".next"
            | ".nuxt"
            | ".pnpm-store"
            | ".svelte-kit"
            | ".turbo"
    ) || name.ends_with("-cache")
}

pub(crate) fn copy_fixture_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let entry_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().unwrap();
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if should_skip_fixture_dir(name.as_ref()) {
                continue;
            }
            copy_fixture_dir(&entry_path, &destination_path);
        } else {
            fs::copy(&entry_path, &destination_path).unwrap();
        }
    }
}

pub(crate) fn copy_example_package_to(
    root: &Path,
    example_name: &str,
    package_name: &str,
) -> PathBuf {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root");
    let source = repo_root.join("examples").join(example_name);
    assert!(
        source.exists(),
        "missing example fixture: {}",
        source.display()
    );

    let destination = root.join(package_name);
    copy_fixture_dir(&source, &destination);
    destination
}

#[cfg(test)]
mod tests {
    use super::copy_fixture_dir;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> PathBuf {
        let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mds-fixture-copy-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            unique,
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn copy_fixture_dir_skips_artifact_directories() {
        let root = temp_dir();
        let source = root.join("source");
        let destination = root.join("destination");

        fs::create_dir_all(source.join("src")).unwrap();
        fs::create_dir_all(source.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(source.join(".build/mds")).unwrap();
        fs::create_dir_all(source.join(".cache/tool")).unwrap();
        fs::create_dir_all(source.join("target/debug")).unwrap();
        fs::create_dir_all(source.join("dist/assets")).unwrap();

        fs::write(source.join("src/keep.ts"), "export const keep = true;\n").unwrap();
        fs::write(
            source.join("node_modules/pkg/index.js"),
            "module.exports = 1;\n",
        )
        .unwrap();
        fs::write(source.join(".build/mds/tmp.ts"), "tmp\n").unwrap();
        fs::write(source.join(".cache/tool/state.json"), "{}\n").unwrap();
        fs::write(source.join("target/debug/app"), "bin\n").unwrap();
        fs::write(source.join("dist/assets/app.js"), "bundle\n").unwrap();

        copy_fixture_dir(&source, &destination);

        assert!(destination.join("src/keep.ts").exists());
        assert!(!destination.join("node_modules").exists());
        assert!(!destination.join(".build").exists());
        assert!(!destination.join(".cache").exists());
        assert!(!destination.join("target").exists());
        assert!(!destination.join("dist").exists());

        let _ = fs::remove_dir_all(root);
    }
}
