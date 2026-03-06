use std::fs;
#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
const PS_SETUP_START: &str = "# >>> agf setup >>>";
#[cfg(windows)]
const PS_SETUP_END: &str = "# <<< agf setup <<<";

/// Detect user's shell and append the init line to the appropriate rc file.
pub fn setup() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        setup_windows()
    }

    #[cfg(not(windows))]
    {
        setup_unix()
    }
}

#[cfg(not(windows))]
fn setup_unix() -> anyhow::Result<()> {
    let shell_path = std::env::var("SHELL").unwrap_or_default();
    let shell_name = shell_path.rsplit('/').next().unwrap_or("");

    let (rc_file, init_line) = match shell_name {
        "zsh" => (
            dirs::home_dir().unwrap_or_default().join(".zshrc"),
            r#"eval "$(agf init zsh)""#,
        ),
        "bash" => {
            let home = dirs::home_dir().unwrap_or_default();
            // Prefer .bashrc, fall back to .bash_profile on macOS
            let rc = if home.join(".bashrc").exists() {
                home.join(".bashrc")
            } else {
                home.join(".bash_profile")
            };
            (rc, r#"eval "$(agf init bash)""#)
        }
        "fish" => (
            dirs::config_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
                .join("fish")
                .join("config.fish"),
            "agf init fish | source",
        ),
        _ => {
            eprintln!("Unsupported shell: {shell_name}");
            eprintln!("Manually add to your shell config:");
            eprintln!("  eval \"$(agf init zsh)\"   # for zsh");
            eprintln!("  eval \"$(agf init bash)\"  # for bash");
            eprintln!("  agf init fish | source    # for fish");
            return Ok(());
        }
    };

    // Check if already configured
    if rc_file.exists() {
        let content = fs::read_to_string(&rc_file)?;
        if content.contains("agf init") {
            eprintln!("Already configured in {}", rc_file.display());
            eprintln!("Restart your shell or run: source {}", rc_file.display());
            return Ok(());
        }
    }

    // Ensure parent directory exists (for fish)
    if let Some(parent) = rc_file.parent() {
        fs::create_dir_all(parent)?;
    }

    // Append the init line
    let mut content = if rc_file.exists() {
        fs::read_to_string(&rc_file)?
    } else {
        String::new()
    };

    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content.push_str(&format!("\n# agf - AI Agent Session Finder\n{init_line}\n"));
    fs::write(&rc_file, content)?;

    eprintln!("Added to {}", rc_file.display());
    eprintln!("Restart your shell or run: source {}", rc_file.display());
    Ok(())
}

#[cfg(windows)]
fn setup_windows() -> anyhow::Result<()> {
    let exe_path = resolve_setup_executable_path();
    let setup_block = build_powershell_setup_block(&exe_path);

    let mut changed = Vec::new();
    let mut unchanged = Vec::new();

    for profile_file in powershell_profile_paths() {
        if let Some(parent) = profile_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = if profile_file.exists() {
            fs::read_to_string(&profile_file)?
        } else {
            String::new()
        };

        let updated = upsert_powershell_setup_block(&content, &setup_block);
        if updated == content {
            unchanged.push(profile_file);
        } else {
            fs::write(&profile_file, updated)?;
            changed.push(profile_file);
        }
    }

    if changed.is_empty() {
        if let Some(first) = unchanged.first() {
            eprintln!("Already configured in {}", first.display());
        } else {
            eprintln!("Already configured.");
        }
    } else {
        for file in &changed {
            eprintln!("Added to {}", file.display());
        }
    }

    eprintln!("Reload current profile: . \"$PROFILE\"");
    eprintln!("Then run 'agf' (shell function), not '.\\agf.exe'.");
    Ok(())
}

#[cfg(windows)]
fn resolve_setup_executable_path() -> PathBuf {
    let current = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("agf.exe"));

    // If setup is invoked from cargo's unstable target/*/deps binary,
    // prefer sibling target/*/agf.exe so profile doesn't pin a transient path.
    let Some(parent) = current.parent() else {
        return current;
    };
    let is_deps_dir = parent
        .file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("deps"))
        .unwrap_or(false);
    if !is_deps_dir {
        return current;
    }

    if let Some(bin_dir) = parent.parent() {
        let stable = bin_dir.join("agf.exe");
        if stable.is_file() {
            return stable;
        }
    }

    current
}

#[cfg(windows)]
fn powershell_profile_paths() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let documents = dirs::document_dir().unwrap_or_else(|| home.join("Documents"));
    let preferred = documents
        .join("PowerShell")
        .join("Microsoft.PowerShell_profile.ps1");
    let legacy = documents
        .join("WindowsPowerShell")
        .join("Microsoft.PowerShell_profile.ps1");

    if preferred == legacy {
        vec![preferred]
    } else {
        vec![preferred, legacy]
    }
}

#[cfg(windows)]
fn build_powershell_setup_block(exe_path: &std::path::Path) -> String {
    let exe = powershell_single_quote_escape(&exe_path.to_string_lossy());
    format!(
        "{PS_SETUP_START}\n$env:AGF_EXE = '{exe}'\n$agfInit = (& '{exe}' init powershell | Out-String)\nInvoke-Expression $agfInit\n{PS_SETUP_END}\n"
    )
}

#[cfg(windows)]
fn upsert_powershell_setup_block(content: &str, block: &str) -> String {
    let content = remove_legacy_powershell_init_lines(content);

    if let Some(start) = content.find(PS_SETUP_START) {
        if let Some(end_rel) = content[start..].find(PS_SETUP_END) {
            let end = start + end_rel + PS_SETUP_END.len();
            let mut out = String::new();
            out.push_str(&content[..start]);
            if !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            out.push_str(block);

            let mut rest = &content[end..];
            if let Some(stripped) = rest.strip_prefix("\r\n") {
                rest = stripped;
            } else if let Some(stripped) = rest.strip_prefix('\n') {
                rest = stripped;
            }
            out.push_str(rest);
            if !out.ends_with('\n') {
                out.push('\n');
            }
            return out;
        }
    }

    let mut out = content;
    if !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(block);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(windows)]
fn remove_legacy_powershell_init_lines(content: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();

    for line in content.lines() {
        let legacy_init =
            line.contains("Invoke-Expression") && line.contains("agf init powershell");
        if legacy_init {
            if kept
                .last()
                .map(|l| l.trim() == "# agf - AI Agent Session Finder")
                .unwrap_or(false)
            {
                kept.pop();
            }
            continue;
        }
        kept.push(line);
    }

    let mut out = kept.join("\n");
    if content.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(windows)]
fn powershell_single_quote_escape(value: &str) -> String {
    value.replace('\'', "''")
}

pub fn shell_init(shell: &str) -> String {
    match shell {
        "zsh" => ZSH_WRAPPER.to_string(),
        "bash" => BASH_WRAPPER.to_string(),
        "fish" => FISH_WRAPPER.to_string(),
        "powershell" | "pwsh" => POWERSHELL_WRAPPER.to_string(),
        other => {
            format!("echo \"Unsupported shell: {other}. Use zsh, bash, fish, or powershell.\"")
        }
    }
}

const ZSH_WRAPPER: &str = r#"function agf() {
    local result
    result="$(command agf "$@")"
    if [ $? -eq 0 ] && [ -n "$result" ]; then
        eval "$result"
    fi
}"#;

const BASH_WRAPPER: &str = r#"function agf() {
    local result
    result="$(command agf "$@")"
    if [ $? -eq 0 ] && [ -n "$result" ]; then
        eval "$result"
    fi
}"#;

const FISH_WRAPPER: &str = r#"function agf
    set -l result (command agf $argv)
    if test $status -eq 0; and test -n "$result"
        eval $result
    end
end"#;

const POWERSHELL_WRAPPER: &str = r#"function agf {
    $agfExe = $env:AGF_EXE
    if ($agfExe -and -not (Test-Path $agfExe)) {
        $agfExe = $null
    }

    if (-not $agfExe) {
        $agfCmd = Get-Command agf -CommandType Application -ErrorAction SilentlyContinue
        if (-not $agfCmd) {
            $agfCmd = Get-Command agf.exe -CommandType Application -ErrorAction SilentlyContinue
        }

        if ($agfCmd) {
            $agfExe = $agfCmd.Source
        }
    }

    if (-not $agfExe) {
        Write-Error "agf executable not found. Run: agf setup"
        return
    }

    $cmdFile = [System.IO.Path]::GetTempFileName()
    $exitCode = 1
    try {
        $env:AGF_WRAPPED = "1"
        $env:AGF_SHELL = "powershell"
        $env:AGF_COMMAND_FILE = $cmdFile

        & $agfExe @args
        $exitCode = $LASTEXITCODE
    }
    finally {
        Remove-Item Env:\AGF_COMMAND_FILE -ErrorAction SilentlyContinue
        Remove-Item Env:\AGF_SHELL -ErrorAction SilentlyContinue
        Remove-Item Env:\AGF_WRAPPED -ErrorAction SilentlyContinue
    }

    try {
        if ($exitCode -eq 0) {
            $cmdLine = (Get-Content -Path $cmdFile -Raw -ErrorAction SilentlyContinue).Trim()
            if ($cmdLine) {
                Invoke-Expression $cmdLine
            }
        }
    }
    finally {
        Remove-Item $cmdFile -ErrorAction SilentlyContinue
    }
}"#;
