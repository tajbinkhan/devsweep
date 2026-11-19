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
