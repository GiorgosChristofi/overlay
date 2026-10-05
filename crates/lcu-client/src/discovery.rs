//! Finds a running League client: first from the `LeagueClientUx` process
//! arguments, then from the lockfile in known install folders.

use crate::lockfile::LockfileInfo;
use std::path::{Path, PathBuf};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const UX_PROCESS_NAMES: [&str; 2] = ["LeagueClientUx.exe", "LeagueClientUx"];
const DEFAULT_INSTALL_DIRS: [&str; 2] = [
    r"C:\Riot Games\League of Legends",
    "/Applications/League of Legends.app/Contents/LoL",
];

/// Returns connection details if a client is running.
pub fn discover() -> Option<LockfileInfo> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    let args: Option<Vec<String>> = sys
        .processes()
        .values()
        .find(|p| UX_PROCESS_NAMES.iter().any(|n| p.name() == *n))
        .map(|p| {
            p.cmd()
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect()
        });

    if let Some(args) = args {
        if let Some(info) = from_ux_args(&args) {
            return Some(info);
        }
        if let Some(info) = install_dir_from_args(&args).and_then(|dir| read_lockfile(&dir)) {
            return Some(info);
        }
    }
    DEFAULT_INSTALL_DIRS
        .iter()
        .find_map(|dir| read_lockfile(Path::new(dir)))
}

fn arg_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    let prefix = format!("--{key}=");
    args.iter()
        .find_map(|a| a.trim_matches('"').strip_prefix(prefix.as_str()))
}

/// Builds connection details from `--app-port` and `--remoting-auth-token`.
pub fn from_ux_args(args: &[String]) -> Option<LockfileInfo> {
    let port = arg_value(args, "app-port")?.parse().ok()?;
    let password = arg_value(args, "remoting-auth-token")?.to_owned();
    let pid = arg_value(args, "app-pid")
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    Some(LockfileInfo {
        pid,
        port,
        password,
        protocol: "https".into(),
    })
}

fn install_dir_from_args(args: &[String]) -> Option<PathBuf> {
    arg_value(args, "install-directory").map(PathBuf::from)
}

fn read_lockfile(dir: &Path) -> Option<LockfileInfo> {
    let raw = std::fs::read_to_string(dir.join("lockfile")).ok()?;
    match LockfileInfo::parse(&raw) {
        Ok(info) => Some(info),
        Err(e) => {
            tracing::warn!(error = %e, "found a lockfile but could not parse it");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn reads_port_and_token_from_process_args() {
        let a = args(&[
            "LeagueClientUx.exe",
            "--riotclient-auth-token=abc",
            "--app-port=61437",
            "\"--remoting-auth-token=tok-123\"",
            "--app-pid=20584",
        ]);
        let info = from_ux_args(&a).unwrap();
        assert_eq!(info.port, 61437);
        assert_eq!(info.password, "tok-123");
        assert_eq!(info.pid, 20584);
        assert!(info.is_tls());
    }

    #[test]
    fn missing_token_means_no_match() {
        assert!(from_ux_args(&args(&["--app-port=1"])).is_none());
    }

    #[test]
    fn reads_lockfile_from_install_dir() {
        let dir = std::env::temp_dir().join(format!("lcu-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lockfile"), "LeagueClient:1:4242:pw:https").unwrap();
        let a = args(&[&format!("--install-directory={}", dir.display())]);
        let info = install_dir_from_args(&a)
            .and_then(|d| read_lockfile(&d))
            .unwrap();
        assert_eq!(info.port, 4242);
        std::fs::remove_dir_all(dir).ok();
    }
}
