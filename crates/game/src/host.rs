//! The process side of a session: which programs run, starting the mod manager, ending
//! what the session started, and screenshots. Behind the [`Host`] trait so every rule is
//! tested against [`crate::testing::FakeHost`]; [`SystemHost`] is the real one.
//!
//! [`SystemHost`] uses only `std::process` and Windows' own tools (`tasklist`, `taskkill`,
//! PowerShell for a screen capture): no process crate (ADR 0010 Amendment 1 (c)). It never
//! attaches to, reads or injects into a process (ADR 0001, ADR 0010 Decision 2): it lists
//! image names, starts one program, and ends programs by image name.

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
}

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
}
