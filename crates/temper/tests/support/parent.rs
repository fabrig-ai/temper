//! Cleanup and bounded observations for parent-death process tests.
use std::{
    process::Child,
    time::Duration,
};

#[derive(Debug)]
pub struct Guard {
    pub child: Option<Child>,
    pub members: Vec<Member>,
}
impl Guard {
    #[must_use]
    pub fn new(child: Child) -> Self {
        Self { child: Some(child), members: Vec::new() }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _killed = child.kill();
            let _reaped = child.wait();
        }
        for member in &self.members {
            let _sent = member.signal("KILL");
        }
    }
}
/// A stable identity admitted only while a live ancestry chain reaches its owner.
/// Numeric IDs remain diagnostic; delivery always uses this retained pidfd.
#[derive(Debug)]
pub struct Member {
    pub pid: u32,
    fd: skein_io::kernel::Fd,
}
impl Member {
    fn open(pid: u32) -> Option<Self> {
        if pid <= 1 || pid > i32::MAX.cast_unsigned() {
            return None;
        }
        Some(Self { pid, fd: skein_shell::open_pidfd(pid).ok()? })
    }
    /// The caller retains this direct child without reaping during acquisition.
    pub fn child(child: &Child) -> Self {
        Self::open(child.id()).expect("unreaped child pidfd")
    }
    /// Admit a surveyed descendant of this test, never an unrelated replacement.
    pub fn in_test(pid: u32) -> Option<Self> {
        Self::open(std::process::id())?.descendant(pid)
    }
    pub fn descendant(&self, pid: u32) -> Option<Self> {
        let member = Self::open(pid)?;
        let mut ancestors = Vec::new();
        let mut current = &member;
        while current.pid != self.pid {
            let stat = std::fs::read_to_string(format!("/proc/{}/stat", current.pid)).ok()?;
            let parent = stat.rsplit_once(") ")?.1.split_whitespace().nth(1)?.parse().ok()?;
            if parent == self.pid {
                break;
            }
            if ancestors.iter().any(|ancestor: &Self| ancestor.pid == parent) {
                return None;
            }
            ancestors.push(Self::open(parent)?);
            current = ancestors.last()?;
        }
        // Open before inspecting /proc, then check every opened identity is still
        // alive afterwards. An exited/reused PID cannot validate a different pidfd.
        if self.exited() || member.exited() || ancestors.iter().any(Self::exited) {
            return None;
        }
        Some(member)
    }
    pub fn exited(&self) -> bool {
        skein_shell::pidfd_exited(self.fd).expect("poll retained pidfd")
    }
    pub fn signal(&self, signal: &str) -> bool {
        use skein_io::kernel::{Signal, Target};
        let signal = match signal {
            "KILL" => Signal::Kill,
            "TERM" => Signal::Terminate,
            _ => panic!("unsupported test signal"),
        };
        skein_shell::signal_kept_child(self.fd, signal, Target::Child).is_ok()
    }
}
impl Drop for Member {
    fn drop(&mut self) {
        skein_shell::close_keeper_fd(self.fd);
    }
}
#[must_use]
pub fn live(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .is_ok_and(|stat| stat.rsplit_once(") ").is_some_and(|(_, rest)| !rest.starts_with('Z')))
}
pub fn wait(mut ready: impl FnMut() -> bool) {
    let clock = skein_shell::Clock::new();
    let end = clock.now().now.saturating_add(skein_lib::Duration::from_secs(5));
    loop {
        if ready() {
            return;
        }
        assert!(clock.now().now < end, "condition settled within five seconds");
        std::thread::sleep(Duration::from_millis(10));
    }
}
pub fn descendants(pid: u32, out: &mut Vec<u32>) {
    if let Ok(children) = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) {
        for child in children.split_whitespace().map(|child| child.parse().expect("child pid")) {
            out.push(child);
            descendants(child, out);
        }
    }
}
