use std::{io, path::PathBuf};

pub fn path() -> io::Result<PathBuf> {
    if let Some(directory) = std::env::var_os("FLOW_CONFIG_DIR") {
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
    Ok(base.join("FlowLauncherPreview").join("hotkey.txt"))
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
