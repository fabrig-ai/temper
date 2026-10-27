//! Offline download figures for the shipped web-profile playground (playground.md, 8 and 9).
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) fn bundle() -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os("TEMPER_TEST_PLAYGROUND")
            .expect("run scripts/browser.sh to build the playground and set TEMPER_TEST_PLAYGROUND"),
    );
    let path = if path.is_absolute() { path } else { skein_scratch::paths::workspace().join(path) };
    assert!(path.join("index.html").is_file(), "playground bundle holds no index.html: {}", path.display());
    path.canonicalize().expect("absolute playground bundle")
}

/// Print each file and the totals; gzip is per file, level nine, with no filename or timestamp.
pub(crate) fn download(root: &Path) -> (u64, u64) {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("playground directory") {
            let entry = entry.expect("playground entry");
            let kind = entry.file_type().expect("playground file type");
            if kind.is_dir() {
                pending.push(entry.path());
            } else {
                assert!(kind.is_file(), "playground entry must be a regular file");
                // Hosting configuration is shipped beside the page, but never fetched by it.
                if entry.path() == root.join("headers") {
                    continue;
                }
                files.push(entry.path());
            }
        }
    }
    files.sort();
    let mut raw = 0_u64;
    let mut gzip = 0_u64;
    for path in &files {
        let bytes = fs::metadata(path).expect("playground file").len();
        let compressed = Command::new("gzip").args(["-9", "-n", "-c"]).arg(path).output().expect("gzip is required");
        assert!(compressed.status.success(), "gzip: {}", String::from_utf8_lossy(&compressed.stderr));
        let compressed = u64::try_from(compressed.stdout.len()).expect("gzip size");
        println!(
            "playground download {}: raw {bytes} bytes, gzip -9 {compressed} bytes",
            path.strip_prefix(root).expect("bundle file").display()
        );
        raw = raw.checked_add(bytes).expect("bundle size");
        gzip = gzip.checked_add(compressed).expect("compressed bundle size");
    }
    let count = u64::try_from(files.len()).expect("bundle file count");
    println!("playground download total: {count} files, raw {raw} bytes, gzip -9 {gzip} bytes");
    (count, raw)
}
