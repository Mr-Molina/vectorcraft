//! Safe saving: a file is written to a temporary file beside it, then renamed over it, so a failed
//! or interrupted write never leaves a damaged file behind (the old one stays as it was).

use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// RAII guard that cleans up the temporary file if dropped while armed.
pub struct TempGuard {
    pub path: PathBuf,
    pub armed: bool,
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Open a unique temporary file beside `target` with a retry loop to prevent `AlreadyExists` collisions.
pub fn open_temp(target: &Path) -> io::Result<(File, TempGuard)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = target.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{}: not a file path", target.display())))?;
    let pid = std::process::id();
    for _ in 0..10_000 {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let tmp = target.with_file_name(format!(".{}.{pid}-{n}.tmp", name.to_string_lossy()));
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => return Ok((file, TempGuard { path: tmp, armed: true })),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "failed to create unique temporary file after multiple attempts"))
}

/// Write `bytes` to `path` atomically (see the module docs).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(path, |f| f.write_all(bytes))
}

/// Write `path` atomically with `fill`, which writes the content into the temporary file. When
/// anything fails (`fill`, flushing, the rename) the temporary file is removed and `path` is left
/// untouched.
pub fn write_atomic_with(path: &Path, fill: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    let target = resolve_target(path)?;
    let (mut f, mut guard) = open_temp(&target)?;
    let filled = fill(&mut f).and_then(|()| f.sync_all());
    drop(f);
    filled?;
    keep_permissions(&target, &guard.path)?;
    std::fs::rename(&guard.path, &target)?;
    guard.armed = false;
    #[cfg(unix)]
    {
        let parent = target.parent().unwrap_or_else(|| Path::new("."));
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// A symbolic link keeps pointing at its file: write the file it names.
/// If `canonicalize` fails (e.g. dangling symlink), fall back to resolving `read_link` relative to `path.parent()`.
pub fn resolve_target(path: &Path) -> io::Result<PathBuf> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => match std::fs::canonicalize(path) {
            Ok(target) => Ok(target),
            Err(_) => {
                let link = std::fs::read_link(path)?;
                if link.is_relative() {
                    let parent = path.parent().unwrap_or_else(|| Path::new(""));
                    Ok(parent.join(link))
                } else {
                    Ok(link)
                }
            }
        },
        _ => Ok(path.to_path_buf()),
    }
}

/// The replacement keeps the replaced file's permissions (Unix; Windows files have none to keep
/// beyond read-only, which makes the save fail as writing in place would).
#[cfg(unix)]
fn keep_permissions(target: &Path, tmp: &Path) -> io::Result<()> {
    match std::fs::metadata(target) {
        Ok(m) => std::fs::set_permissions(tmp, m.permissions()),
        // A new file: the default permissions.
        Err(_) => Ok(()),
    }
}

#[cfg(not(unix))]
fn keep_permissions(_: &Path, _: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vc-atomic-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The other files in `d` besides `keep`.
    fn leftovers(d: &Path, keep: &str) -> Vec<String> {
        std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).filter(|n| n != keep).collect()
    }

    #[test]
    fn the_original_file_survives_a_failed_write() {
        let d = dir("fail");
        let path = d.join("doc.vectorcraft");
        std::fs::write(&path, b"original").unwrap();
        let r = write_atomic_with(&path, |f| {
            f.write_all(b"half a fi")?;
            Err(io::Error::other("disk full"))
        });
        assert!(r.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert!(leftovers(&d, "doc.vectorcraft").is_empty(), "the temporary file is removed");
        write_atomic(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert!(leftovers(&d, "doc.vectorcraft").is_empty());
        // A folder that doesn't exist: an error, nothing written.
        assert!(write_atomic(&d.join("missing").join("x.vectorcraft"), b"x").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn temp_guard_cleans_up_on_drop_when_armed() {
        let d = dir("guard");
        let path = d.join("test.tmp");
        std::fs::write(&path, b"temp").unwrap();
        assert!(path.exists());
        {
            let _guard = TempGuard { path: path.clone(), armed: true };
        }
        assert!(!path.exists(), "armed guard should remove file on drop");

        std::fs::write(&path, b"temp2").unwrap();
        assert!(path.exists());
        {
            let mut guard = TempGuard { path: path.clone(), armed: true };
            guard.armed = false;
        }
        assert!(path.exists(), "disarmed guard should NOT remove file on drop");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn open_temp_retries_on_collision() {
        let d = dir("collision");
        let target = d.join("file.vectorcraft");
        let (f1, g1) = open_temp(&target).unwrap();
        let (f2, g2) = open_temp(&target).unwrap();
        assert_ne!(g1.path, g2.path);
        drop(f1);
        drop(f2);
        drop(g1);
        drop(g2);
        let _ = std::fs::remove_dir_all(&d);
    }
}
