//! Runs the external programs DevSweep relies on (docker, wsl, diskpart) without a console
//! window and with a time limit, so a hung daemon cannot freeze a scan.

use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// A command that opens no console window on Windows.
pub fn new(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// A command for a program that ships with Windows (`diskpart.exe`, `reg.exe`, `wsl.exe`),
/// started by its full path in the system folder. Looked up by name, Windows would try
/// DevSweep's own folder first, which the per-user install leaves writable; a program
/// planted there would then run with administrator rights whenever DevSweep has them.
/// The folder comes from Windows itself, not from `%SystemRoot%`, which the user can set.
#[cfg(windows)]
pub fn system(program: &str) -> io::Result<Command> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut buf = [0u16; 260];
    // SAFETY: the buffer and its length match, and the call writes at most that many units.
    let len = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if len == 0 || len >= buf.len() {
        return Err(io::Error::last_os_error());
    }
    let dir = std::path::PathBuf::from(std::ffi::OsString::from_wide(&buf[..len]));
    let mut cmd = new(&dir.join(program).to_string_lossy());
    cmd.current_dir(&dir);
    Ok(cmd)
}

/// Runs `cmd` to the end and collects its output, or kills it once `limit` has passed.
pub fn output(mut cmd: Command, limit: Duration) -> io::Result<Output> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    // Read both pipes while waiting, so a chatty program never blocks on a full pipe.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("gave no answer within {} seconds", limit.as_secs()),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    Ok(Output { status, stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default() })
}

/// What a failed program said: its error output, or its normal output when that is empty.
pub fn message(out: &Output) -> String {
    let err = String::from_utf8_lossy(&out.stderr);
    let text = if err.trim().is_empty() { String::from_utf8_lossy(&out.stdout) } else { err };
    text.trim().to_string()
}
