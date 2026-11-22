//! Running processes via `sysinfo`.

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use super::{ProcessSnapshot, ProcessSource};

/// The real process list.
pub struct OsProcesses;

impl ProcessSource for OsProcesses {
    fn snapshot(&self) -> Option<ProcessSnapshot> {
        let mut sys = System::new().ok()?;
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
        );
        let me = std::process::id();
        let mut snap = ProcessSnapshot::default();
        for (pid, proc_) in sys.processes() {
            let pid = pid.as_u32();
            snap.alive.insert(pid);
            if pid == me {
                continue;
            }
            let mut args: Vec<String> = proc_
                .cmd()
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            if args.is_empty() {
                args.push(proc_.name().to_string_lossy().into_owned());
            }
            snap.processes.push((pid, args));
        }
        Some(snap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_excludes_self_but_knows_it_is_alive() {
        let snap = OsProcesses.snapshot().unwrap();
        let me = std::process::id();
        assert!(snap.alive.contains(&me));
        assert!(snap.processes.iter().all(|(pid, _)| *pid != me));
    }
}
