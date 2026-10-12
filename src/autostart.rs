use std::{io, path::Path};

pub fn is_enabled() -> io::Result<bool> {
    platform::is_enabled()
}
pub fn set_enabled(enabled: bool) -> io::Result<()> {
    platform::set_enabled(enabled, &std::env::current_exe()?)
}

#[cfg(any(windows, test))]
fn windows_command(path: &str) -> io::Result<String> {
    if path.contains(['"', '\0', '\n', '\r']) {
        return Err(io::Error::other("程序路径包含无效字符"));
    }
    let command = format!("\"{path}\" --autostart");
    if command.encode_utf16().count() > 260 {
        return Err(io::Error::other("程序路径过长，无法注册登录自启"));
    }
    Ok(command)
}

#[cfg(any(target_os = "macos", test))]
fn launch_agent(path: &str) -> String {
    let escaped = path
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>net.qingso.launcher</string>\n<key>ProgramArguments</key><array><string>{escaped}</string><string>--autostart</string></array>\n<key>RunAtLoad</key><true/>\n<key>LimitLoadToSessionType</key><string>Aqua</string>\n</dict></plist>\n"
    )
}

#[cfg(windows)]
mod platform {
    use super::*;
    use winreg::{
        RegKey,
        enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    };
    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "QingsoLauncher";

    pub fn is_enabled() -> io::Result<bool> {
        read(KEY)
    }
    pub fn set_enabled(enabled: bool, exe: &Path) -> io::Result<()> {
        write(KEY, enabled, exe)
    }

    fn read(path: &str) -> io::Result<bool> {
        let key = match RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(path, KEY_READ) {
            Ok(key) => key,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        match key.get_value::<String, _>(VALUE) {
            Ok(command) => Ok(!command.trim().is_empty()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn write(path: &str, enabled: bool, exe: &Path) -> io::Result<()> {
        let user = RegKey::predef(HKEY_CURRENT_USER);
        if enabled {
            let command = windows_command(
                exe.to_str()
                    .ok_or_else(|| io::Error::other("程序路径无法编码"))?,
            )?;
            let (key, _) = user.create_subkey_with_flags(path, KEY_WRITE)?;
            key.set_value(VALUE, &command)
        } else {
            let key = match user.open_subkey_with_flags(path, KEY_WRITE) {
                Ok(key) => key,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            };
            match key.delete_value(VALUE) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn isolated_registry_round_trip() {
            let path = format!(r"Software\QingsoAutostartTest-{}", std::process::id());
            // Exercise real registry IO without touching the user's Run key.
            assert!(!read(&path).unwrap());
            write(
                &path,
                true,
                Path::new(r"C:\Program Files\轻搜\qingso-launcher.exe"),
            )
            .unwrap();
            assert!(read(&path).unwrap());
            let key = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(&path)
                .unwrap();
            assert_eq!(
                key.get_value::<String, _>(VALUE).unwrap(),
                "\"C:\\Program Files\\轻搜\\qingso-launcher.exe\" --autostart"
            );
            drop(key);
            write(&path, false, Path::new("unused")).unwrap();
            assert!(!read(&path).unwrap());
            write(&path, false, Path::new("unused")).unwrap();
            RegKey::predef(HKEY_CURRENT_USER)
                .delete_subkey(&path)
                .unwrap();
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    fn path() -> io::Result<std::path::PathBuf> {
        Ok(std::path::PathBuf::from(
            std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is unavailable"))?,
        )
        .join("Library/LaunchAgents/net.qingso.launcher.plist"))
    }
    pub fn is_enabled() -> io::Result<bool> {
        match std::fs::metadata(path()?) {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
    pub fn set_enabled(enabled: bool, exe: &Path) -> io::Result<()> {
        let path = path()?;
        if enabled {
            let data = launch_agent(
                exe.to_str()
                    .ok_or_else(|| io::Error::other("程序路径无法编码"))?,
            );
            crate::config::write_atomic(&path, data.as_bytes())
        } else {
            match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::*;
    pub fn is_enabled() -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "当前系统暂不支持开机自启",
        ))
    }
    pub fn set_enabled(_: bool, _: &Path) -> io::Result<()> {
        is_enabled().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_with_spaces_unicode_and_xml_characters_are_preserved() {
        assert_eq!(
            windows_command(r"C:\轻搜 文件夹\qingso.exe").unwrap(),
            "\"C:\\轻搜 文件夹\\qingso.exe\" --autostart"
        );
        assert!(windows_command("bad\"path").is_err());
        assert!(windows_command(&"x".repeat(261)).is_err());
        let plist = launch_agent("/Users/test/轻搜 & Tools/launcher");
        assert!(plist.contains("轻搜 &amp; Tools"));
        assert!(plist.contains("<string>--autostart</string>"));
        assert!(plist.contains("<key>RunAtLoad</key><true/>"));
    }
}
