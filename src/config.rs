use std::env;
#[cfg(windows)]
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::AgfError;
use crate::model::Agent;

pub fn home_dir() -> Result<PathBuf, AgfError> {
    dirs::home_dir().ok_or(AgfError::NoHomeDir)
}

pub fn claude_dir() -> Result<PathBuf, AgfError> {
    Ok(home_dir()?.join(".claude"))
}

pub fn codex_dir() -> Result<PathBuf, AgfError> {
    Ok(home_dir()?.join(".codex"))
}

pub fn opencode_data_dir() -> Result<PathBuf, AgfError> {
    Ok(home_dir()?.join(".local/share/opencode"))
}

pub fn pi_sessions_dir() -> Result<PathBuf, AgfError> {
    Ok(home_dir()?.join(".pi/agent/sessions"))
}

pub fn gemini_dir() -> Result<PathBuf, AgfError> {
    Ok(home_dir()?.join(".gemini"))
}

pub fn cursor_dir() -> Result<PathBuf, AgfError> {
    Ok(home_dir()?.join(".cursor"))
}

pub fn kiro_data_dir() -> Result<PathBuf, AgfError> {
    // Kiro CLI stores data via dirs::data_local_dir()
    // macOS: ~/Library/Application Support/kiro-cli/
    // Linux: ~/.local/share/kiro-cli/
    dirs::data_local_dir()
        .map(|d| d.join("kiro-cli"))
        .ok_or(AgfError::NoHomeDir)
}

pub fn is_agent_installed(agent: Agent) -> bool {
    command_exists(agent.cli_name())
}

pub fn installed_agents() -> Vec<Agent> {
    Agent::all()
        .iter()
        .copied()
        .filter(|a| is_agent_installed(*a))
        .collect()
}

fn command_exists(binary: &str) -> bool {
    let candidate = Path::new(binary);
    if candidate.components().count() > 1 {
        return executable_candidates(candidate)
            .iter()
            .any(|path| path.is_file());
    }

    let Some(path_var) = env::var_os("PATH") else {
        return false;
    };

    for dir in env::split_paths(&path_var) {
        let base = dir.join(binary);
        if executable_candidates(&base)
            .iter()
            .any(|path| path.is_file())
        {
            return true;
        }
    }

    false
}

fn executable_candidates(base: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        if base.extension().is_some() {
            return vec![base.to_path_buf()];
        }

        let mut candidates = Vec::new();
        candidates.push(base.to_path_buf());
        for ext in windows_pathext() {
            let mut value = base.as_os_str().to_os_string();
            value.push(ext);
            candidates.push(PathBuf::from(value));
        }
        candidates
    }

    #[cfg(not(windows))]
    {
        vec![base.to_path_buf()]
    }
}

#[cfg(windows)]
fn windows_pathext() -> Vec<OsString> {
    let defaults = vec![
        OsString::from(".EXE"),
        OsString::from(".CMD"),
        OsString::from(".BAT"),
        OsString::from(".COM"),
    ];

    let Some(pathext) = env::var_os("PATHEXT") else {
        return defaults;
    };

    let text = pathext.to_string_lossy();
    let mut exts = Vec::new();
    for raw in text.split(';') {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            exts.push(OsString::from(trimmed));
        }
    }
    if exts.is_empty() {
        defaults
    } else {
        exts
    }
}
