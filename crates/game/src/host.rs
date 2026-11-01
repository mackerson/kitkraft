//! The process side of a session: which programs run, starting the mod manager, ending
//! what the session started, and screenshots. Behind the [`Host`] trait so every rule is
//! tested against [`crate::testing::FakeHost`]; [`SystemHost`] is the real one.
//!
//! [`SystemHost`] uses only `std::process` and Windows' own tools (`tasklist`, `taskkill`,
//! `netsh` for the excluded TCP port ranges, PowerShell for a screen capture and for
//! whether the game has a main window): no process crate (ADR 0010 Amendment 1 (c)). It
//! never attaches to, reads or injects into a process (ADR 0001, ADR 0010 Decision 2): it
//! lists image names, top-level windows and excluded port ranges, starts one program, and
//! ends programs by image name.

use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::error::{GameError, Result};

/// A program the session started.
pub trait Launched: Send {
    /// Its process id.
    fn id(&self) -> u32;
    /// Ends it if it still runs.
    fn kill(&mut self) -> Result<()>;
}

/// What a session needs from the operating system.
pub trait Host: Send + Sync {
    /// Image names of the running processes (`Starfield.exe`).
    fn running(&self) -> Result<Vec<String>>;
    /// Starts `program` with `args` in `cwd`.
    fn launch(&self, program: &Path, args: &[String], cwd: &Path) -> Result<Box<dyn Launched>>;
    /// Ends every running process whose image name is in `images`; returns the names ended.
    fn terminate(&self, images: &[String]) -> Result<Vec<String>>;
    /// Writes a PNG of the screen to `dest`.
    fn screenshot(&self, dest: &Path) -> Result<()>;
    /// Whether a running process with the image name `image` (`Starfield.exe`) has a main
    /// window. The controller sends no command before it does (game-20261101-T1/b'). An
    /// error means unknown, not no.
    fn has_main_window(&self, image: &str) -> Result<bool>;
    /// The TCP port ranges (inclusive) the operating system excludes from binding (on
    /// Windows, Hyper-V/WSL reservations: `netsh int ipv4 show excludedportrange
    /// protocol=tcp`). An error means unknown, not none.
    fn excluded_ports(&self) -> Result<Vec<(u16, u16)>>;
}

/// The `(start, end)` ranges in `netsh int ipv4 show excludedportrange` output. Only lines
/// of two integers count (an administered range carries a trailing `*`), so localized
/// headers and footers are skipped.
pub fn parse_excluded_ports(text: &str) -> Vec<(u16, u16)> {
    text.lines()
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            let start = words.next()?.parse::<u16>().ok()?;
            let end = words.next()?.parse::<u16>().ok()?;
            match words.next() {
                None | Some("*") if words.next().is_none() => {
                    Some((start.min(end), start.max(end)))
                }
                _ => None,
            }
        })
        .collect()
}

/// The answer of [`WINDOWS`]: the number of processes with a main window, as text.
pub fn parse_window_count(text: &str) -> Result<bool> {
    text.trim().parse::<u32>().map(|n| n > 0).map_err(|_| {
        GameError::Process(format!(
            "main window: not a count: `{}`",
            text.trim().chars().take(80).collect::<String>()
        ))
    })
}

/// PowerShell that prints how many processes named by its first argument (the image
/// name without `.exe`) have a main window: the same test as the M4.T1 script
/// (`MainWindowHandle` is not 0), read from the window list, not from the process.
pub const WINDOWS: &str = "& { param($n) \
@(Get-Process -Name $n -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 }).Count }";

/// Whether `name` is among `running` (ASCII case ignored).
pub fn is_running(running: &[String], name: &str) -> bool {
    running.iter().any(|r| r.eq_ignore_ascii_case(name))
}

/// The real host (Windows).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemHost;

struct ChildProcess(Child);

impl Launched for ChildProcess {
    fn id(&self) -> u32 {
        self.0.id()
    }

    fn kill(&mut self) -> Result<()> {
        match self.0.try_wait() {
            Ok(Some(_)) => Ok(()),
            _ => {
                let _ = self.0.kill();
                let _ = self.0.wait();
                Ok(())
            }
        }
    }
}

/// The image names in `tasklist /FO CSV /NH` output.
pub fn parse_tasklist(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix('"')?;
            let end = rest.find('"')?;
            rest.get(..end).map(str::to_string)
        })
        .collect()
}

fn output(cmd: &mut Command, what: &str) -> Result<String> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .map_err(|e| GameError::Process(format!("{what}: {e}")))?;
    if !out.status.success() {
        return Err(GameError::Process(format!(
            "{what}: exit {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// PowerShell that captures the virtual screen into the PNG named by its first argument.
const CAPTURE: &str = "& { param($p) Add-Type -AssemblyName System.Windows.Forms, System.Drawing; \
$b = [System.Windows.Forms.SystemInformation]::VirtualScreen; \
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height; \
$g = [System.Drawing.Graphics]::FromImage($bmp); \
$g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size); \
$bmp.Save($p, [System.Drawing.Imaging.ImageFormat]::Png); $g.Dispose(); $bmp.Dispose() }";

impl Host for SystemHost {
    fn running(&self) -> Result<Vec<String>> {
        if !cfg!(windows) {
            return Err(GameError::Process(
                "listing processes is implemented for Windows only".into(),
            ));
        }
        let text = output(
            Command::new("tasklist").args(["/FO", "CSV", "/NH"]),
            "tasklist",
        )?;
        Ok(parse_tasklist(&text))
    }

    fn launch(&self, program: &Path, args: &[String], cwd: &Path) -> Result<Box<dyn Launched>> {
        let child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| GameError::Process(format!("{}: {e}", program.display())))?;
        Ok(Box::new(ChildProcess(child)))
    }

    fn terminate(&self, images: &[String]) -> Result<Vec<String>> {
        let running = self.running()?;
        let mut ended = Vec::new();
        for image in images {
            if is_running(&running, image) {
                output(
                    Command::new("taskkill").args(["/F", "/T", "/IM", image]),
                    "taskkill",
                )?;
                ended.push(image.clone());
            }
        }
        Ok(ended)
    }

    fn excluded_ports(&self) -> Result<Vec<(u16, u16)>> {
        if !cfg!(windows) {
            return Err(GameError::Process(
                "excluded port ranges are read on Windows only".into(),
            ));
        }
        let text = output(
            Command::new("netsh").args([
                "int",
                "ipv4",
                "show",
                "excludedportrange",
                "protocol=tcp",
            ]),
            "netsh",
        )?;
        Ok(parse_excluded_ports(&text))
    }

    fn has_main_window(&self, image: &str) -> Result<bool> {
        if !cfg!(windows) {
            return Err(GameError::Process(
                "window lists are read on Windows only".into(),
            ));
        }
        let stem = Path::new(image)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(image);
        let text = output(
            Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", WINDOWS])
                .arg(stem),
            "main window",
        )?;
        parse_window_count(&text)
    }

    fn screenshot(&self, dest: &Path) -> Result<()> {
        output(
            Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", CAPTURE])
                .arg(dest),
            "screenshot",
        )?;
        if dest.is_file() {
            Ok(())
        } else {
            Err(GameError::Process(format!(
                "screenshot: {} was not written",
                dest.display()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasklist_csv_gives_image_names() {
        let text = "\"System Idle Process\",\"0\",\"Services\",\"0\",\"8 K\"\r\n\"ModOrganizer.exe\",\"8784\",\"Console\",\"1\",\"201,688 K\"\r\nINFO: junk\r\n";
        let names = parse_tasklist(text);
        assert_eq!(names, ["System Idle Process", "ModOrganizer.exe"]);
        assert!(is_running(&names, "modorganizer.EXE"));
        assert!(!is_running(&names, "Starfield.exe"));
    }

    #[test]
    fn a_window_count_is_a_number() {
        assert!(parse_window_count("1\r\n").unwrap());
        assert!(parse_window_count(" 2 ").unwrap());
        assert!(!parse_window_count("0\r\n").unwrap());
        assert!(parse_window_count("").is_err());
        assert!(parse_window_count("Get-Process : error").is_err());
    }

    #[test]
    fn netsh_excluded_ranges_are_lines_of_two_integers() {
        // As the dev box printed it on 2026-11-01, and a localized header.
        let text = "\r\nProtocol tcp Port Exclusion Ranges\r\n\r\nStart Port    End Port      \r\n----------    --------      \r\n      5357        5357      \r\n     50000       50059     *\r\n     55477       55576      \r\n\r\n* - Administered port exclusions.\r\n";
        assert_eq!(
            parse_excluded_ports(text),
            [(5357, 5357), (50000, 50059), (55477, 55576)]
        );
        let localized = "Protokoll tcp Portausschlussbereiche\r\n\r\nStartport    Endport\r\n---------    -------\r\n     55477       55576\r\n 1 2 3\r\n 70000 70001\r\n 12 x\r\n";
        assert_eq!(parse_excluded_ports(localized), [(55477, 55576)]);
        assert!(parse_excluded_ports("").is_empty());
    }
}
