use crate::{Project, FORMAT_VERSION};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const MAX_PROJECT_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_ASSET_BYTES: u64 = 32 * 1024 * 1024;

fn unique_suffix() -> String {
    format!(
        "{}-{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_PROJECT_BYTES {
        return Err("Файл слишком большой (максимум 8 МБ).".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Не удалось прочитать файл: {e}"))
}

pub fn read_project(path: &Path) -> Result<Project, String> {
    let project: Project = read_json(path)?;
    if project.format_version != FORMAT_VERSION {
        return Err("Эта версия формата пока не поддерживается.".into());
    }
    if project.scenes.is_empty() {
        return Err("В проекте нет сцен.".into());
    }
    let mut ids = BTreeSet::new();
    if project
        .scenes
        .iter()
        .any(|scene| scene.id.is_empty() || !ids.insert(&scene.id))
    {
        return Err("Идентификаторы сцен должны быть непустыми и уникальными.".into());
    }
    // Incomplete text and routes are valid drafts. Playback/export validates them later.
    Ok(project)
}

/// Writes beside the destination, syncs, then renames. A failed write never truncates
/// the user's current project. Existing content is also kept in `<file>.bak`.
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Проект превышает ограничение 8 МБ.".into());
    }
    let tmp = PathBuf::from(format!("{}.tmp-{}", path.display(), unique_suffix()));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            fs::copy(path, format!("{}.bak", path.display()))?;
        }
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("Не удалось сохранить {}: {e}", path.display()))
}

/// Uses a portable, unambiguous slash-separated relative path on every host OS.
pub fn is_safe_asset_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['\\', ':'])
        && !value.chars().any(char::is_control)
        && value.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.ends_with(['.', ' '])
        })
}

pub fn resolve_asset(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if !is_safe_asset_path(relative) {
        return Err("Изображение должно находиться внутри папки проекта.".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let resolved = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("Не найдено изображение {relative}: {e}"))?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return Err("Изображение находится за пределами проекта.".into());
    }
    if fs::metadata(&resolved).map_err(|e| e.to_string())?.len() > MAX_ASSET_BYTES {
        return Err("Изображение слишком большое (максимум 32 МБ).".into());
    }
    Ok(resolved)
}

pub fn assets(project: &Project) -> BTreeSet<&str> {
    project
        .scenes
        .iter()
        .flat_map(|scene| [&scene.background, &scene.portrait])
        .flatten()
        .map(String::as_str)
        .collect()
}

pub fn import_asset(root: &Path, source: &Path) -> Result<String, String> {
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
        return Err("Выберите PNG или JPEG.".into());
    }
    if fs::metadata(source).map_err(|e| e.to_string())?.len() > MAX_ASSET_BYTES {
        return Err("Изображение слишком большое (максимум 32 МБ).".into());
    }
    let folder = root.join("assets");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    if !folder
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&canonical_root)
    {
        return Err("Папка assets находится за пределами проекта.".into());
    }
    let relative = format!("assets/image-{}.{}", unique_suffix(), ext);
    let mut target = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(&relative))
        .map_err(|e| e.to_string())?;
    let mut source = fs::File::open(source).map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut target).map_err(|e| e.to_string())?;
    target.sync_all().map_err(|e| e.to_string())?;
    Ok(relative)
}

pub fn copy_assets(
    project: &Project,
    source_root: &Path,
    target_root: &Path,
) -> Result<(), String> {
    let target_root = target_root.canonicalize().map_err(|e| e.to_string())?;
    for relative in assets(project) {
        let source = resolve_asset(source_root, relative)?;
        let target = target_root.join(relative);
        let parent = target
            .parent()
            .ok_or("Не удалось определить папку изображения.")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        if !parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&target_root)
        {
            return Err("Папка назначения находится вне проекта.".into());
        }
        if target.exists() {
            let resolved = resolve_asset(&target_root, relative)?;
            if fs::read(&source).map_err(|e| e.to_string())?
                != fs::read(&resolved).map_err(|e| e.to_string())?
            {
                return Err(format!(
                    "В папке уже есть другое изображение {relative}. Выберите новую папку."
                ));
            }
        } else {
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| e.to_string())?;
            let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
            std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Creates a new, portable directory. The desktop app supplies its executable.
/// An existing directory is never overwritten or removed.
pub fn export_bundle(
    project: &Project,
    root: &Path,
    destination: &Path,
    executable: &Path,
) -> Result<(), String> {
    project.check()?;
    if destination.exists() {
        return Err("Папка сборки уже существует. Выберите новое имя.".into());
    }
    let staging = PathBuf::from(format!("{}.tmp-{}", destination.display(), unique_suffix()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        copy_assets(project, root, &staging)?;
        write_json(&staging.join("story.rinva"), project)?;
        fs::copy(executable, staging.join("RinvaPlayer.exe")).map_err(|e| e.to_string())?;
        fs::write(
            staging.join("LICENSE.txt"),
            include_str!("../../../LICENSE"),
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            staging.join("THIRD_PARTY_NOTICES.txt"),
            concat!(
                include_str!("../../../docs/THIRD_PARTY_NOTICES.txt"),
                "\nZune-core and zune-jpeg (https://github.com/etemesi254/zune-image)\n",
                include_str!("../../../docs/ZUNE_LICENSE_MIT.txt")
            ),
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            staging.join("README.txt"),
            "Запустите RinvaPlayer.exe. Передавайте всю папку целиком.\r\n",
        )
        .map_err(|e| e.to_string())?;
        fs::rename(&staging, destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(staging);
    }
    result
}
