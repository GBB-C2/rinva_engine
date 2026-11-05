use super::ui::wide;
use rinva_core::recovery::{self, Snapshot};
use std::{
    path::{Path, PathBuf},
    ptr::null,
    thread::JoinHandle,
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};

/// A named kernel object distinguishes a live editor from a crashed session,
/// including when Windows has reused the previous process ID.
pub struct Session {
    pub id: String,
    pub path: PathBuf,
    marker: HANDLE,
    worker: Option<JoinHandle<Result<Snapshot, String>>>,
}

impl Session {
    pub fn new(folder: &Path) -> Result<Self, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = format!("{}-{stamp:x}", std::process::id());
        let marker = unsafe {
            CreateMutexW(
                null(),
                0,
                wide(&format!("Local\\RinvaRecovery-{id}")).as_ptr(),
            )
        };
        if marker.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Self {
            path: folder.join(format!("session-{id}.json")),
            id,
            marker,
            worker: None,
        })
    }
    pub fn is_active(id: &str) -> bool {
        unsafe {
            let handle = OpenMutexW(
                SYNCHRONIZATION_SYNCHRONIZE,
                0,
                wide(&format!("Local\\RinvaRecovery-{id}")).as_ptr(),
            );
            if !handle.is_null() {
                CloseHandle(handle);
                true
            } else {
                GetLastError() == ERROR_ACCESS_DENIED
            }
        }
    }
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }
    pub fn start(&mut self, snapshot: Snapshot) {
        if self.busy() {
            return;
        }
        let path = self.path.clone();
        self.worker = Some(std::thread::spawn(move || {
            std::fs::create_dir_all(path.parent().ok_or("Нет папки для автокопии.")?)
                .map_err(|e| e.to_string())?;
            recovery::write(&path, &snapshot)?;
            Ok(snapshot)
        }));
    }
    pub fn finish(&mut self) -> Option<Result<Snapshot, String>> {
        self.worker.take().map(|job| {
            job.join()
                .unwrap_or_else(|_| Err("Ошибка фонового автосохранения.".into()))
        })
    }
    pub fn poll(&mut self) -> Option<Result<Snapshot, String>> {
        if self.worker.as_ref().is_some_and(JoinHandle::is_finished) {
            self.finish()
        } else {
            None
        }
    }
    pub fn clear(&mut self) -> Result<(), String> {
        // Wait for our own outstanding write before deleting it; it must not
        // recreate a stale snapshot after an explicit Save or Discard.
        let _ = self.finish();
        recovery::remove(&self.path)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.finish();
        unsafe {
            CloseHandle(self.marker);
        }
    }
}

pub fn candidates(folder: &Path, only_source: Option<&Path>) -> Vec<(PathBuf, Snapshot)> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return vec![];
    };
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Ok(snapshot) = recovery::read(&path)
            .or_else(|_| recovery::read(&PathBuf::from(format!("{}.bak", path.display()))))
        else {
            continue;
        };
        if path
            .file_name()
            .is_none_or(|name| name != format!("session-{}.json", snapshot.session_id).as_str())
        {
            continue;
        }
        if Session::is_active(&snapshot.session_id) {
            continue;
        }
        if only_source.is_some_and(|source| snapshot.source_path.as_deref() != Some(source)) {
            continue;
        }
        candidates.push((path, snapshot));
    }
    candidates.sort_by_key(|a| std::cmp::Reverse(a.1.saved_at));
    candidates
}

pub fn smoke(folder: &Path) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let folder = folder.canonicalize().map_err(|e| e.to_string())?;
    let mut first = Session::new(&folder)?;
    let second = Session::new(&folder)?;
    if first.id == second.id || !Session::is_active(&first.id) || !Session::is_active(&second.id) {
        return Err("Recovery session isolation failed".into());
    }
    let snapshot = Snapshot::new(
        first.id.clone(),
        rinva_core::Project::new(),
        None,
        None,
        folder.clone(),
    );
    first.start(snapshot.clone());
    first.finish().ok_or("Recovery worker missing")??;
    first.start(snapshot.clone());
    first.finish().ok_or("Recovery worker missing")??;
    if !candidates(&folder, None).is_empty() {
        return Err("A live editor's recovery was offered".into());
    }
    // Simulate a damaged newest copy; the previous complete snapshot survives.
    std::fs::write(&first.path, b"incomplete").map_err(|e| e.to_string())?;
    let path = first.path.clone();
    let first_id = first.id.clone();
    drop(first);
    let available = candidates(&folder, None);
    if Session::is_active(&first_id)
        || available.len() != 1
        || available[0].1.project != snapshot.project
    {
        return Err("Crashed-session recovery or backup fallback failed".into());
    }
    recovery::remove(&path)?;
    let mut clearing = Session::new(&folder)?;
    let mut snapshot = snapshot;
    snapshot.session_id = clearing.id.clone();
    clearing.start(snapshot);
    clearing.clear()?;
    if clearing.path.exists() {
        return Err("An asynchronous snapshot reappeared after discard".into());
    }
    Ok(())
}
