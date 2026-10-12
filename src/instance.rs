use crate::{AppWindow, launcher};
use interprocess::local_socket::{Listener, ListenerOptions, Name, Stream, prelude::*};
use single_instance::SingleInstance;
use std::sync::Arc;
use std::{
    error::Error,
    io::{self, BufRead, BufReader, Write},
    thread,
    time::Duration,
};

const APP_ID: &str = "hover-flow-launcher-4fc9e1b8";

pub struct Instance {
    // The OS lock is acquired before binding IPC, including concurrent starts.
    _lock: SingleInstance,
    listener: Arc<Listener>,
}

fn socket_name() -> io::Result<Name<'static>> {
    #[cfg(windows)]
    {
        APP_ID.to_ns_name::<interprocess::local_socket::GenericNamespaced>()
    }
    #[cfg(not(windows))]
    {
        std::env::temp_dir()
            .join(format!("{APP_ID}.sock"))
            .to_fs_name::<interprocess::local_socket::GenericFilePath>()
    }
}

/// None means an existing instance was successfully told to show its window.
pub fn acquire() -> Result<Option<Instance>, Box<dyn Error>> {
    #[cfg(windows)]
    let lock_name = APP_ID.to_string();
    #[cfg(not(windows))]
    let lock_name = std::env::temp_dir()
        .join(format!("{APP_ID}.lock"))
        .to_string_lossy()
        .into_owned();
    let lock = SingleInstance::new(&lock_name)?;
    if !lock.is_single() {
        activate_existing()?;
        return Ok(None);
    }
    #[cfg(not(windows))]
    {
        // Only the lock owner may reclaim a socket left by a crashed process.
        let path = std::env::temp_dir().join(format!("{APP_ID}.sock"));
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let listener = ListenerOptions::new().name(socket_name()?).create_sync()?;
    Ok(Some(Instance {
        _lock: lock,
        listener: Arc::new(listener),
    }))
}

fn activate_existing() -> io::Result<()> {
    // Another simultaneous launch may own the lock before its IPC is ready.
    let mut last_error = io::Error::other("Existing launcher did not respond");
    for _ in 0..20 {
        match Stream::connect(socket_name()?) {
            Ok(mut stream) => {
                let mut greeting = String::new();
                BufReader::new(&mut stream).read_line(&mut greeting)?;
                let pid: u32 = greeting
                    .trim()
                    .strip_prefix("FLOW_V1 ")
                    .ok_or_else(|| io::Error::other("Invalid launcher IPC greeting"))?
                    .parse()
                    .map_err(io::Error::other)?;
                #[cfg(windows)]
                {
                    #[link(name = "user32")]
                    unsafe extern "system" {
                        fn AllowSetForegroundWindow(pid: u32) -> i32;
                    }
                    // Allow only the verified primary process to take focus.
                    unsafe {
                        AllowSetForegroundWindow(pid);
                    }
                }
                #[cfg(not(windows))]
                let _ = pid;
                stream.write_all(b"show\n")?;
                eprintln!("Activated existing launcher (PID {pid})");
                return Ok(());
            }
            Err(error) => last_error = error,
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(last_error)
}

impl Instance {
    pub fn listen(&self, weak: slint::Weak<AppWindow>) -> io::Result<()> {
        let listener = self.listener.clone();
        thread::Builder::new()
            .name("launcher-activation".into())
            .spawn(move || {
                for mut stream in listener.incoming().flatten() {
                    if writeln!(stream, "FLOW_V1 {}", std::process::id()).is_err() {
                        continue;
                    }
                    let mut command = String::new();
                    if BufReader::new(&mut stream).read_line(&mut command).is_ok()
                        && command.trim() == "show"
                    {
                        let _ = weak.upgrade_in_event_loop(|ui| {
                            eprintln!("Existing instance activation received");
                            if let Err(error) = launcher::show(&ui) {
                                eprintln!("Cannot activate launcher: {error}");
                            }
                        });
                    }
                }
            })?;
        Ok(())
    }
}
