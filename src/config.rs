use std::{io, path::PathBuf};

pub fn path() -> io::Result<PathBuf> {
    if let Some(directory) =
        std::env::var_os("QINGSO_CONFIG_DIR").or_else(|| std::env::var_os("FLOW_CONFIG_DIR"))
    {
        return Ok(PathBuf::from(directory).join("hotkey.txt"));
    }
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("APPDATA is unavailable"))?;
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("HOME is unavailable"))?
        .join("Library/Application Support");
    #[cfg(not(any(windows, target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| io::Error::other("Configuration directory is unavailable"))?;
    migrate_legacy(&base)
}

fn migrate_legacy(base: &std::path::Path) -> io::Result<PathBuf> {
    let folder = base.join("QingsoLauncher");
    let legacy = base.join("FlowLauncherPreview");
    for name in ["hotkey.txt", "search-tags.json"] {
        let target = folder.join(name);
        if target.exists() {
            continue;
        }
        match std::fs::read(legacy.join(name)) {
            Ok(bytes) => write_atomic(&target, &bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(folder.join("hotkey.txt"))
}

pub fn load() -> io::Result<Option<String>> {
    match std::fs::read_to_string(path()?) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value.trim().into())),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn save(value: &str) -> io::Result<()> {
    let path = path()?;
    write_atomic(&path, format!("{}\n", value.trim()).as_bytes())
}

pub fn write_atomic(path: &std::path::Path, content: &[u8]) -> io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let temporary = path.with_extension("txt.tmp");
    use std::io::Write;
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(content)?;
    file.sync_all()?;
    drop(file);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(source: *const u16, target: *const u16, flags: u32) -> i32;
        }
        let source: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), 1 | 8) } == 0 {
            let error = io::Error::last_os_error();
            let _ = std::fs::remove_file(temporary);
            return Err(error);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_settings_copy_without_overwriting_new_values() {
        let base = std::env::temp_dir().join(format!("qingso-migration-{}", std::process::id()));
        let old = base.join("FlowLauncherPreview");
        let new = base.join("QingsoLauncher");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("hotkey.txt"), "Ctrl+Alt+Space\n").unwrap();
        std::fs::write(old.join("search-tags.json"), "[]").unwrap();
        assert_eq!(migrate_legacy(&base).unwrap(), new.join("hotkey.txt"));
        assert_eq!(
            std::fs::read_to_string(new.join("search-tags.json")).unwrap(),
            "[]"
        );
        std::fs::write(new.join("hotkey.txt"), "Alt+F12\n").unwrap();
        migrate_legacy(&base).unwrap();
        assert_eq!(
            std::fs::read_to_string(new.join("hotkey.txt")).unwrap(),
            "Alt+F12\n"
        );
        assert_eq!(
            std::fs::read_to_string(old.join("hotkey.txt")).unwrap(),
            "Ctrl+Alt+Space\n"
        );
        for folder in [old, new] {
            std::fs::remove_file(folder.join("hotkey.txt")).unwrap();
            std::fs::remove_file(folder.join("search-tags.json")).unwrap();
            std::fs::remove_dir(folder).unwrap();
        }
        std::fs::remove_dir(base).unwrap();
    }
}
