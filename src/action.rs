use crate::model::{Action, Agent, Session};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShellKind {
    #[cfg(not(windows))]
    Posix,
    #[cfg(windows)]
    WindowsCmd,
    #[cfg(windows)]
    WindowsPowerShell,
}

pub fn generate_command(
    session: &Session,
    action: Action,
    new_agent: Option<Agent>,
) -> Option<String> {
    let shell = detect_shell_kind();
    let cd_cmd = change_dir_cmd(&session.project_path, shell);

    match action {
        Action::Resume => {
            let cmd = session.agent.resume_cmd(&session.session_id);
            Some(join_commands(&cd_cmd, &cmd, shell))
        }
        Action::NewSession => {
            let agent = new_agent.unwrap_or(session.agent);
            let cmd = agent.new_session_cmd();
            Some(join_commands(&cd_cmd, cmd, shell))
        }
        Action::Cd => Some(cd_cmd),
        Action::Delete | Action::Back => None,
    }
}

pub fn action_preview(session: &Session, action: Action) -> String {
    match action {
        Action::Resume => session.agent.resume_cmd(&session.session_id),
        Action::NewSession => "choose agent CLI...".to_string(),
        Action::Cd => change_dir_cmd(&session.project_path, detect_shell_kind()),
        Action::Delete => "remove session data".to_string(),
        Action::Back => "return to session list".to_string(),
    }
}

pub fn new_session_with_flags(session: &Session, agent: Agent, flags: &str) -> Option<String> {
    let shell = detect_shell_kind();
    let cd_cmd = change_dir_cmd(&session.project_path, shell);
    let base = agent.new_session_cmd();
    Some(join_commands(&cd_cmd, &format!("{base}{flags}"), shell))
}

fn detect_shell_kind() -> ShellKind {
    #[cfg(windows)]
    {
        if let Ok(shell) = std::env::var("AGF_SHELL") {
            let shell = shell.to_ascii_lowercase();
            if shell == "powershell" || shell == "pwsh" {
                return ShellKind::WindowsPowerShell;
            }
            if shell == "cmd" {
                return ShellKind::WindowsCmd;
            }
        }

        // Without wrapper context, default to cmd-safe emission.
        // PowerShell wrapper explicitly sets AGF_SHELL=powershell.
        ShellKind::WindowsCmd
    }

    #[cfg(not(windows))]
    {
        ShellKind::Posix
    }
}

fn change_dir_cmd(path: &str, shell: ShellKind) -> String {
    #[cfg(windows)]
    {
        match shell {
            ShellKind::WindowsPowerShell => {
                format!(
                    "Set-Location -LiteralPath {}",
                    shell_escape_powershell(path)
                )
            }
            ShellKind::WindowsCmd => {
                format!("cd /d {}", shell_escape_cmd(path))
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = shell;
        format!("cd {}", shell_escape_posix(path))
    }
}

fn join_commands(first: &str, second: &str, shell: ShellKind) -> String {
    #[cfg(windows)]
    {
        match shell {
            ShellKind::WindowsPowerShell => {
                format!("{first}; if ($?) {{ {second} }}")
            }
            ShellKind::WindowsCmd => {
                format!("{first} && {second}")
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = shell;
        format!("{first} && {second}")
    }
}

#[cfg(windows)]
fn shell_escape_powershell(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(windows)]
fn shell_escape_cmd(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

#[cfg(not(windows))]
fn shell_escape_posix(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
