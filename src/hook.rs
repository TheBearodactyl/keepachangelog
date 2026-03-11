use {
    miette::{IntoDiagnostic, Result, bail},
    std::{fs, path::PathBuf},
};

pub fn reopen_tty() {
    #[cfg(unix)]
    unix::reopen_tty();

    #[cfg(windows)]
    windows::reopen_tty();
}

#[cfg(unix)]
mod unix {
    use std::fs;

    pub fn reopen_tty() {
        use std::os::unix::io::IntoRawFd;

        if let Ok(tty) = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
        {
            let tty_fd = tty.into_raw_fd();
            unsafe {
                libc::dup2(tty_fd, 0);
                libc::close(tty_fd);
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    pub fn reopen_tty() {
        use windows_sys::Win32::{
            Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
            Storage::FileSystem::{
                CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE,
                OPEN_EXISTING,
            },
            System::Console::{STD_INPUT_HANDLE, SetStdHandle},
        };

        let conin: Vec<u16> = "CONIN$\0".encode_utf16().collect();

        unsafe {
            let handle = CreateFileW(
                conin.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );

            if handle != INVALID_HANDLE_VALUE {
                SetStdHandle(STD_INPUT_HANDLE, handle);
            }
        }
    }
}

pub fn install(changelog_file: &str) -> Result<()> {
    let hooks_dir = find_git_hooks_dir()?;
    let hook_path = hooks_dir.join("pre-commit");

    if hook_path.exists() {
        eprintln!(
            "A pre-commit hook already exists at {}",
            hook_path.display()
        );
        eprintln!("Rename or remove it first, then re-run --setup-hook.");
        bail!("pre-commit hook already exists");
    }

    let bin_raw = std::env::current_exe()
        .into_diagnostic()?
        .display()
        .to_string();
    let bin = bin_raw.replace('\\', "/");

    let script = format!(
        "#!/usr/bin/env sh\n\
         \"{bin}\" --file \"{changelog_file}\"\n"
    );

    fs::write(&hook_path, &script).into_diagnostic()?;
    make_executable(&hook_path)?;

    println!("Installed pre-commit hook at {}", hook_path.display());
    println!("Runs: {} --file {changelog_file}", bin);
    println!("Skip: git commit --no-verify");

    Ok(())
}

fn make_executable(path: &PathBuf) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).into_diagnostic()?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).into_diagnostic()?;
    }

    #[cfg(not(unix))]
    let _ = path;

    Ok(())
}

fn find_git_hooks_dir() -> Result<PathBuf> {
    let mut dir = std::env::current_dir().into_diagnostic()?;
    loop {
        let candidate = dir.join(".git").join("hooks");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            bail!("No .git directory found — are you inside a git repository?");
        }
    }
}
