//! The process-wide set of installed plug-ins.
//!
//! Plug-ins are installed per process, not per document: live effects are drawn from every open
//! document (and from scratch sessions such as previews), and they must find the same plug-ins.

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

#[cfg(not(target_arch = "wasm32"))]
use crate::Error;
use crate::{Limits, Plugin, Result};

static PLUGINS: RwLock<BTreeMap<String, Arc<Plugin>>> = RwLock::new(BTreeMap::new());
static REVISION: AtomicU64 = AtomicU64::new(0);

/// Ensures a manifest entry path does not contain null bytes or directory traversal (SEC-503).
pub fn validate_manifest_entry(manifest: &crate::Manifest) -> Result<()> {
    if let Some(entry) = &manifest.entry
        && (entry.contains('\0')
            || std::path::Path::new(entry)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))))
    {
        return Err(crate::Error::Manifest(format!("manifest entry {entry:?} contains forbidden path components")));
    }
    Ok(())
}

/// Installs `plugin`, replacing one with the same id.
pub fn install(mut plugin: Plugin) -> Arc<Plugin> {
    let _ = validate_manifest_entry(plugin.manifest());
    plugin.set_serial(REVISION.fetch_add(1, Ordering::Relaxed) + 1);
    let p = Arc::new(plugin);
    PLUGINS.write().unwrap_or_else(|e| e.into_inner()).insert(p.id().to_string(), p.clone());
    p
}

/// Loads `bytes` with the default limits and installs the plug-in.
pub fn install_bytes(bytes: &[u8]) -> Result<Arc<Plugin>> {
    let plugin = Plugin::load(bytes, Limits::default())?;
    validate_manifest_entry(plugin.manifest())?;
    Ok(install(plugin))
}

pub fn get(id: &str) -> Option<Arc<Plugin>> {
    PLUGINS.read().unwrap_or_else(|e| e.into_inner()).get(id).cloned()
}

/// Installed plug-ins, sorted by id.
pub fn list() -> Vec<Arc<Plugin>> {
    PLUGINS.read().unwrap_or_else(|e| e.into_inner()).values().cloned().collect()
}

/// Uninstalls `id`; false if it wasn't installed.
pub fn remove(id: &str) -> bool {
    let gone = PLUGINS.write().unwrap_or_else(|e| e.into_inner()).remove(id).is_some();
    if gone {
        REVISION.fetch_add(1, Ordering::Relaxed);
    }
    gone
}

/// Changes whenever a plug-in is installed or removed (for caches such as menus).
pub fn revision() -> u64 {
    REVISION.load(Ordering::Relaxed)
}

/// `s` as a `&'static str`, for catalogues that hold static strings (effect and menu labels). Each
/// distinct string is stored once, so memory stays bounded by the plug-ins ever installed.
pub fn intern(s: &str) -> &'static str {
    static SET: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
    let mut set = SET.lock().unwrap_or_else(|e| e.into_inner());
    let set = set.get_or_insert_with(HashSet::new);
    if let Some(v) = set.get(s) {
        return v;
    }
    let v: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(v);
    v
}

/// Reads a `.wasm` file (refusing anything that isn't a regular file or exceeds the module limit)
/// and loads it.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_file(path: &std::path::Path, limits: Limits) -> Result<Plugin> {
    use std::io::Read;
    let path_str = path.to_string_lossy();
    if path_str.contains('\0') {
        return Err(Error::Io(format!("{}: path contains null byte", path.display())));
    }
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(Error::Io(format!("{}: path traversal is not permitted", path.display())));
    }
    if path_str.starts_with("\\\\") || path_str.starts_with("//") {
        return Err(Error::Io(format!("{}: UNC paths are not permitted", path.display())));
    }
    let meta = std::fs::metadata(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    if !meta.is_file() {
        return Err(Error::Io(format!("{} is not a file", path.display())));
    }
    if meta.len() > limits.max_module_bytes as u64 {
        return Err(Error::Module(format!("{} is {} bytes (limit {})", path.display(), meta.len(), limits.max_module_bytes)));
    }
    let f = std::fs::File::open(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    let mut bytes = Vec::new();
    f.take(limits.max_module_bytes as u64 + 1).read_to_end(&mut bytes).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    let plugin = Plugin::load(&bytes, limits)?.with_source(path.display().to_string());
    validate_manifest_entry(plugin.manifest())?;
    Ok(plugin)
}

/// What [`load_folder`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderReport {
    /// Ids of the plug-ins installed.
    pub loaded: Vec<String>,
    /// `(file, error)` for each `.wasm` that failed to load.
    pub failed: Vec<(String, String)>,
}

/// Installs every `*.wasm` directly in `dir` (not recursive), in file-name order. A bad module is
/// reported and skipped; the others still load.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_folder(dir: &std::path::Path) -> Result<FolderReport> {
    let rd = std::fs::read_dir(dir).map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?;
    let mut files: Vec<std::path::PathBuf> =
        rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wasm"))).collect();
    files.sort();
    files.truncate(1000);
    let mut report = FolderReport::default();
    for f in files {
        match load_file(&f, Limits::default()) {
            Ok(p) => report.loaded.push(install(p).id().to_string()),
            Err(e) => report.failed.push((f.display().to_string(), e.to_string())),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_manifest_entry_rejects_null_and_traversal() {
        let mut m = crate::Manifest::placeholder();
        m.entry = Some("normal/path.wasm".into());
        assert!(validate_manifest_entry(&m).is_ok());

        m.entry = Some("path\0with/null".into());
        assert!(validate_manifest_entry(&m).is_err());

        m.entry = Some("../parent.wasm".into());
        assert!(validate_manifest_entry(&m).is_err());

        m.entry = Some("/absolute/path.wasm".into());
        assert!(validate_manifest_entry(&m).is_err());

        #[cfg(windows)]
        {
            m.entry = Some("C:\\prefix\\path.wasm".into());
            assert!(validate_manifest_entry(&m).is_err());
        }
    }
}
