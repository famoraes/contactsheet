//! Hot folder: watch a directory and report new scans once the scanner has
//! finished writing them (size unchanged between two polls).

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

const POLL: Duration = Duration::from_millis(1000);
/// A file must also be at least this old, in case the writer pauses.
const SETTLE: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct HotFolderState {
    current: Mutex<Option<Watcher>>,
}

struct Watcher {
    stop: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
struct Timing {
    poll: Duration,
    settle: Duration,
}

const TIMING: Timing = Timing { poll: POLL, settle: SETTLE };

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HotFile {
    path: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HotError {
    message: String,
}

/// Scans the hot folder should pick up: TIFF, DNG or camera RAW files that aren't
/// hidden or our own exports.
fn is_candidate(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let scan = name.ends_with(".tif") || name.ends_with(".tiff") || negative::raw::is_raw(path);
    !name.starts_with('.') && scan && !name.ends_with("_positive.tif") && !name.ends_with("_positive.tiff")
}

fn list(dir: &Path) -> std::io::Result<Vec<(PathBuf, u64, SystemTime)>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !is_candidate(&path) {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                out.push((path, meta.len(), meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
            }
        }
    }
    Ok(out)
}

fn watch(dir: &Path, stop: &AtomicBool, t: Timing, on_file: impl Fn(PathBuf), on_error: impl Fn(String)) {
    // Files already present are not "new".
    let mut seen: HashSet<PathBuf> = list(dir).map(|v| v.into_iter().map(|(p, ..)| p).collect()).unwrap_or_default();
    let mut sizes: HashMap<PathBuf, u64> = HashMap::new();
    let mut failing = false;
    while !stop.load(Ordering::Relaxed) {
        thread::sleep(t.poll);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let files = match list(dir) {
            Ok(f) => {
                failing = false;
                f
            }
            Err(e) => {
                if !failing {
                    on_error(format!("{}: {e}", dir.display()));
                }
                failing = true;
                continue;
            }
        };
        let now = SystemTime::now();
        let mut new_ready = Vec::new();
        for (path, size, modified) in files {
            if seen.contains(&path) {
                continue;
            }
            let settled = now.duration_since(modified).map(|age| age >= t.settle).unwrap_or(false);
            if size > 0 && settled && sizes.get(&path) == Some(&size) {
                new_ready.push(path);
            } else {
                sizes.insert(path, size);
            }
        }
        new_ready.sort();
        for path in new_ready {
            sizes.remove(&path);
            seen.insert(path.clone());
            on_file(path);
        }
    }
}

/// Start watching `dir` for new scans (replacing any previous watch), or stop when `None`.
#[tauri::command]
pub fn watch_folder(app: AppHandle, state: State<'_, HotFolderState>, dir: Option<String>) -> Result<(), String> {
    let mut current = state.current.lock().map_err(|_| "hot folder state poisoned".to_string())?;
    if let Some(w) = current.take() {
        w.stop.store(true, Ordering::Relaxed);
    }
    if let Some(dir) = dir {
        let dir = PathBuf::from(dir);
        if !dir.is_dir() {
            return Err(format!("{} is not a folder", dir.display()));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        thread::Builder::new()
            .name("hot-folder".into())
            .spawn(move || {
                let err_app = app.clone();
                watch(
                    &dir,
                    &s,
                    TIMING,
                    |p| {
                        let _ = app.emit("hot-folder", HotFile { path: p.display().to_string() });
                    },
                    |message| {
                        let _ = err_app.emit("hot-folder-error", HotError { message });
                    },
                );
            })
            .map_err(|e| e.to_string())?;
        *current = Some(Watcher { stop });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_new_files_once_written() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("hotfolder-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("old.tif"), b"existing").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let found = Arc::new(Mutex::new(Vec::<String>::new()));
        let (d, s, f) = (dir.clone(), stop.clone(), found.clone());
        let t = Timing { poll: Duration::from_millis(100), settle: Duration::from_millis(250) };
        let handle = thread::spawn(move || {
            watch(&d, &s, t, |p| f.lock().unwrap().push(p.file_name().unwrap().to_string_lossy().into()), |_| {})
        });
        thread::sleep(Duration::from_millis(150)); // let the watcher snapshot existing files
        // A scan written in chunks: not reported while it is still growing.
        let mut file = std::fs::File::create(dir.join("new.tif")).unwrap();
        for _ in 0..5 {
            file.write_all(&[0u8; 1024]).unwrap();
            file.flush().unwrap();
            thread::sleep(Duration::from_millis(120));
            assert!(found.lock().unwrap().is_empty(), "reported while still being written");
        }
        drop(file);
        std::fs::write(dir.join("new_positive.tif"), b"our export").unwrap();
        thread::sleep(Duration::from_millis(900));
        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();
        assert_eq!(*found.lock().unwrap(), vec!["new.tif".to_string()], "only the new scan, once");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn candidates() {
        for ok in ["/s/a.tif", "/s/B.TIFF", "/s/roll 1.tif", "/s/DSC_0001.NEF", "/s/scan.dng"] {
            assert!(is_candidate(Path::new(ok)), "{ok}");
        }
        for no in ["/s/.a.tif", "/s/a.jpg", "/s/a_positive.tif", "/s/a.tif.part", "/s/roll.contactsheet"] {
            assert!(!is_candidate(Path::new(no)), "{no}");
        }
    }
}
