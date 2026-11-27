//! Runs the external programs DevSweep relies on (docker, wsl, diskpart) without a console
//! window and with a time limit, so a hung daemon cannot freeze a scan.

use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::sync::Mutex;
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

/// Runs `cmd` to the end and collects its output, or stops it and everything it started
/// once `limit` has passed. The program ends with DevSweep: closing the window half way
/// through a `pnpm store prune` must not leave pnpm running on its own.
pub fn output(cmd: Command, limit: Duration) -> io::Result<Output> {
    run(cmd, limit, true)
}

/// `output` for a program that must not be cut off half way, even when DevSweep exits.
/// diskpart stopped while compacting leaves the disk attached and locked until a reboot.
/// Only Windows has diskpart.
#[cfg(windows)]
pub fn output_to_end(cmd: Command, limit: Duration) -> io::Result<Output> {
    run(cmd, limit, false)
}

fn run(mut cmd: Command, limit: Duration, ends_with_app: bool) -> io::Result<Output> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(target_os = "linux")]
    if ends_with_app {
        use std::os::unix::process::CommandExt;
        // SAFETY: prctl is safe to call between fork and exec. The signal goes out when the
        // thread that started the program ends, and this thread waits for the program below.
        unsafe {
            cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
    }
    let mut child = cmd.spawn()?;
    let _running = ends_with_app.then(|| Running::start(&child));
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
            // `pnpm.cmd` runs under cmd.exe, so killing the child alone would leave pnpm.
            let procs = crate::procs::snapshot();
            let _ = crate::procs::stop(&[child.id()], &procs);
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

/// Programs started by `output` that have not ended yet.
static RUNNING: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Holds a program's place in `RUNNING` while it runs.
struct Running(u32);

impl Running {
    fn start(child: &Child) -> Self {
        #[cfg(windows)]
        job::adopt(child);
        RUNNING.lock().unwrap().push(child.id());
        Running(child.id())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.lock().unwrap().retain(|p| *p != self.0);
    }
}

/// Stops every program `output` started that is still running, and what those started.
/// Called when DevSweep exits. On Windows the job below also covers a crash, and on Linux
/// the parent-death signal does; this is what ends them on macOS.
pub fn end_all() {
    let pids = std::mem::take(&mut *RUNNING.lock().unwrap());
    if !pids.is_empty() {
        let _ = crate::procs::stop(&pids, &crate::procs::snapshot());
    }
}

/// One job object for the whole app, set to end every process in it when its last handle
/// closes. Windows closes that handle when DevSweep exits for any reason, crashes included,
/// and the programs a child starts join the job too, so pnpm under cmd.exe goes as well.
#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    struct Job(HANDLE);
    // SAFETY: a job handle may be used from any thread.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn job() -> Option<HANDLE> {
        JOB.get_or_init(|| {
            // SAFETY: plain Win32 calls on a handle this function owns; the info struct is
            // zeroed, then the one flag is set, and its size is passed along with it.
            unsafe {
                let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if h.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let set = SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if set == 0 {
                    CloseHandle(h);
                    return None;
                }
                Some(Job(h))
            }
        })
        .as_ref()
        .map(|j| j.0)
    }

    /// Puts `child` in the job. A program it starts in the moment before this runs stays
    /// outside; `end_all` still stops that one on a normal exit.
    pub fn adopt(child: &Child) {
        if let Some(job) = job() {
            // SAFETY: both handles are valid for the duration of the call.
            unsafe {
                AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE);
            }
        }
    }
}

/// What a failed program said: its error output, or its normal output when that is empty.
pub fn message(out: &Output) -> String {
    let err = String::from_utf8_lossy(&out.stderr);
    let text = if err.trim().is_empty() { String::from_utf8_lossy(&out.stdout) } else { err };
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program that runs for a minute, found again by `n` in its command line.
    fn direct(n: u32) -> (Command, String) {
        if cfg!(windows) {
            let mut c = new("ping");
            c.args(["-n", "60", &format!("127.0.0.{n}")]);
            (c, format!("127.0.0.{n}"))
        } else {
            let mut c = new("sleep");
            c.arg(format!("6{n}"));
            (c, format!("sleep 6{n}"))
        }
    }

    /// The same program under a shell, so the program is a grandchild, as pnpm is under
    /// cmd.exe when it is installed as `pnpm.cmd`.
    fn wrapped(n: u32) -> (Command, String) {
        if cfg!(windows) {
            let mut c = new("cmd");
            c.args(["/c", &format!("ping -n 60 127.0.0.{n} >nul")]);
            (c, format!("127.0.0.{n}"))
        } else {
            let mut c = new("sh");
            // `; true` keeps sh from replacing itself with sleep.
            c.args(["-c", &format!("sleep 6{n}; true")]);
            (c, format!("sleep 6{n}"))
        }
    }

    /// The program with `marker` in its command line, other than a shell running it.
    fn alive(marker: &str) -> Option<u32> {
        crate::procs::snapshot()
            .into_iter()
            .find(|p| p.cmd.contains(marker) && !matches!(p.name.as_str(), "cmd" | "sh"))
            .map(|p| p.pid)
    }

    fn within(secs: u64, mut done: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(secs) {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    #[test]
    fn a_timeout_stops_the_program_under_the_wrapper_too() {
        let (cmd, marker) = wrapped(1);
        let err = output(cmd, Duration::from_secs(2)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(within(5, || alive(&marker).is_none()), "{marker} still runs");
    }

    #[test]
    fn a_program_is_on_the_exit_list_only_while_it_runs() {
        let (cmd, marker) = direct(2);
        let runner = std::thread::spawn(move || output(cmd, Duration::from_secs(60)));
        let mut pid = None;
        assert!(within(10, || {
            pid = alive(&marker);
            pid.is_some()
        }));
        let pid = pid.unwrap();
        assert!(RUNNING.lock().unwrap().contains(&pid));
        assert!(crate::procs::stop(&[pid], &crate::procs::snapshot()).is_empty());
        let _ = runner.join().unwrap();
        assert!(!RUNNING.lock().unwrap().contains(&pid));
    }

    #[test]
    #[ignore = "started by programs_end_when_devsweep_is_killed"]
    fn helper_runs_a_program_for_a_minute() {
        // Windows ends the whole job, so the wrapped form is covered there. Linux signals
        // only the direct child, which is how DevSweep starts pnpm there.
        let (cmd, _) = if cfg!(windows) { wrapped(3) } else { direct(3) };
        let _ = output(cmd, Duration::from_secs(60));
    }

    /// DevSweep killed outright, as by Task Manager or a crash, so nothing of its own runs.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn programs_end_when_devsweep_is_killed() {
        let marker = if cfg!(windows) { wrapped(3).1 } else { direct(3).1 };
        let mut helper = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "command::tests::helper_runs_a_program_for_a_minute"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        assert!(within(20, || alive(&marker).is_some()), "the helper started {marker}");
        helper.kill().unwrap();
        helper.wait().unwrap();
        assert!(within(5, || alive(&marker).is_none()), "{marker} outlived the helper");
    }
}
