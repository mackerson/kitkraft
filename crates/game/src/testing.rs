//! Fakes for tests, here and in the engine: a local HTTP server standing in for the Console
//! API's `/console` endpoint, and a host whose processes are a list. Nothing in them talks
//! to a game; they exist so every rule of the controller is tested without one.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::console::parse_response_request;
use crate::error::{GameError, Result};
use crate::host::{Host, Launched};

type Hook = Box<dyn Fn(&str) + Send + Sync>;

#[derive(Default)]
struct FakeState {
    answers: Mutex<Vec<(String, String)>>,
    delays: Mutex<Vec<(String, Duration)>>,
    received: Mutex<Vec<String>>,
    down: AtomicBool,
    hooks: Mutex<Vec<Hook>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A local stand-in for the `/console` endpoint: answers each `POST` with the text set for
/// the longest matching command prefix (ASCII case ignored), else an empty body.
pub struct FakeConsole {
    addr: SocketAddr,
    state: Arc<FakeState>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for FakeConsole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeConsole")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl FakeConsole {
    /// Starts the server on a free loopback port.
    pub fn start() -> std::io::Result<FakeConsole> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let state = Arc::new(FakeState::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (s, st) = (Arc::clone(&state), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let s = Arc::clone(&s);
                        std::thread::spawn(move || serve(stream, &s));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Ok(FakeConsole {
            addr,
            state,
            stop,
            thread: Some(thread),
        })
    }

    /// `http://127.0.0.1:<port>/console`.
    pub fn url(&self) -> String {
        format!("http://{}/console", self.addr)
    }

    /// Answers commands starting with `prefix` with `text`.
    pub fn answer(&self, prefix: &str, text: &str) {
        lock(&self.state.answers).push((prefix.to_ascii_lowercase(), text.to_string()));
    }

    /// Waits `delay` before answering commands starting with `prefix`.
    pub fn delay(&self, prefix: &str, delay: Duration) {
        lock(&self.state.delays).push((prefix.to_ascii_lowercase(), delay));
    }

    /// Runs `hook` with every command received (before answering).
    pub fn on_command(&self, hook: impl Fn(&str) + Send + Sync + 'static) {
        lock(&self.state.hooks).push(Box::new(hook));
    }

    /// While down, connections are closed without an answer.
    pub fn set_down(&self, down: bool) {
        self.state.down.store(down, Ordering::SeqCst);
    }

    /// Every command received, in order.
    pub fn received(&self) -> Vec<String> {
        lock(&self.state.received).clone()
    }
}

impl Drop for FakeConsole {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn best<'a, T: Clone>(list: &'a [(String, T)], command: &str) -> Option<&'a T> {
    let c = command.to_ascii_lowercase();
    list.iter()
        .filter(|(p, _)| c.starts_with(p.as_str()))
        .max_by_key(|(p, _)| p.len())
        .map(|(_, v)| v)
}

fn serve(mut stream: TcpStream, state: &FakeState) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let command = loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(chunk.get(..n).unwrap_or_default()),
        }
        if let Some(body) = parse_response_request(&buf) {
            break String::from_utf8_lossy(&body).into_owned();
        }
        if buf.len() > 1_048_576 {
            return;
        }
    };
    lock(&state.received).push(command.clone());
    if state.down.load(Ordering::SeqCst) {
        return;
    }
    for hook in lock(&state.hooks).iter() {
        hook(&command);
    }
    let delay = best(&lock(&state.delays), &command).copied();
    if let Some(d) = delay {
        std::thread::sleep(d);
    }
    let text = best(&lock(&state.answers), &command)
        .cloned()
        .unwrap_or_default();
    let reply = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
    let _ = stream.write_all(reply.as_bytes());
}

/// A host whose processes are a list.
#[derive(Default)]
pub struct FakeHost {
    running: Mutex<Vec<String>>,
    starts: Mutex<Vec<String>>,
    launched: Mutex<Vec<(PathBuf, Vec<String>)>>,
    terminated: Mutex<Vec<String>>,
    no_screenshots: AtomicBool,
    hooks: Mutex<Vec<Box<dyn Fn() + Send + Sync>>>,
}

impl std::fmt::Debug for FakeHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeHost").finish_non_exhaustive()
    }
}

impl FakeHost {
    /// A host running `running`, where a launch starts `starts`.
    pub fn new(running: &[&str], starts: &[&str]) -> FakeHost {
        let host = FakeHost::default();
        *lock(&host.running) = running.iter().map(|s| s.to_string()).collect();
        *lock(&host.starts) = starts.iter().map(|s| s.to_string()).collect();
        host
    }

    /// Screenshots fail from now on.
    pub fn fail_screenshots(&self) {
        self.no_screenshots.store(true, Ordering::SeqCst);
    }

    /// Runs `hook` when the session launches (to play the game changing files).
    pub fn on_launch(&self, hook: impl Fn() + Send + Sync + 'static) {
        lock(&self.hooks).push(Box::new(hook));
    }

    /// What was launched.
    pub fn launched(&self) -> Vec<(PathBuf, Vec<String>)> {
        lock(&self.launched).clone()
    }

    /// What was ended.
    pub fn terminated(&self) -> Vec<String> {
        lock(&self.terminated).clone()
    }

    /// What runs now.
    pub fn now_running(&self) -> Vec<String> {
        lock(&self.running).clone()
    }
}

struct FakeLaunched;

impl Launched for FakeLaunched {
    fn id(&self) -> u32 {
        4242
    }
    fn kill(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Host for FakeHost {
    fn running(&self) -> Result<Vec<String>> {
        Ok(lock(&self.running).clone())
    }

    fn launch(&self, program: &Path, args: &[String], _cwd: &Path) -> Result<Box<dyn Launched>> {
        lock(&self.launched).push((program.to_path_buf(), args.to_vec()));
        let starts = lock(&self.starts).clone();
        lock(&self.running).extend(starts);
        for hook in lock(&self.hooks).iter() {
            hook();
        }
        Ok(Box::new(FakeLaunched))
    }

    fn terminate(&self, images: &[String]) -> Result<Vec<String>> {
        let mut running = lock(&self.running);
        let ended: Vec<String> = images
            .iter()
            .filter(|i| running.iter().any(|r| r.eq_ignore_ascii_case(i)))
            .cloned()
            .collect();
        running.retain(|r| !images.iter().any(|i| i.eq_ignore_ascii_case(r)));
        lock(&self.terminated).extend(ended.iter().cloned());
        Ok(ended)
    }

    fn screenshot(&self, dest: &Path) -> Result<()> {
        if self.no_screenshots.load(Ordering::SeqCst) {
            return Err(GameError::Process("fake screenshot failure".into()));
        }
        std::fs::write(dest, b"\x89PNG fake").map_err(|e| GameError::io(dest, e))
    }
}

/// A fixture MO2 instance with a dev profile ready for a session, a dev-deployed plugin,
/// the console plugin as an enabled mod, a base save, an observe root and a checkout with
/// a ledger. Every path is under `root`.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The folder holding everything.
    pub root: PathBuf,
    /// The MO2 instance root.
    pub instance: PathBuf,
    /// The dev profile's name (`Kitkraft Dev`, as the owner's folder is spelled).
    pub profile: String,
    /// The dev profile folder.
    pub profile_dir: PathBuf,
    /// The base save file.
    pub base_save: PathBuf,
    /// The plugin under test, as deployed.
    pub plugin: PathBuf,
    /// `<root>/checkout/observe`.
    pub observe_root: PathBuf,
    /// `<root>/checkout/docs/game-observations.md`.
    pub ledger: PathBuf,
}

/// The base save's bytes in [`mo2_fixture`].
pub const FIXTURE_SAVE: &[u8] = b"fixture base save";

fn put(path: &Path, text: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| GameError::io(p, e))?;
    }
    std::fs::write(path, text).map_err(|e| GameError::io(path, e))
}

/// Writes [`Fixture`] under `root` (removing what was there).
pub fn mo2_fixture(root: &Path) -> Result<Fixture> {
    let _ = std::fs::remove_dir_all(root);
    let instance = root.join("MO2");
    let profile = "Kitkraft Dev".to_string();
    let profile_dir = instance.join("profiles").join(&profile);
    put(
        &instance.join("ModOrganizer.ini"),
        b"[General]\r\ngameName=Starfield\r\nselected_profile=@ByteArray(Kitkraft Dev)\r\n",
    )?;
    put(&instance.join("ModOrganizer.exe"), b"not a program")?;
    put(
        &instance
            .join("profiles")
            .join("Default")
            .join("modlist.txt"),
        b"+Owner Mod\r\n",
    )?;
    put(
        &profile_dir.join("modlist.txt"),
        b"# This file was automatically generated by Mod Organizer.\r\n+KitKraft Dev - X\r\n+Console API\r\n",
    )?;
    put(&profile_dir.join("plugins.txt"), b"# header\r\n*X.esm\r\n")?;
    put(
        &profile_dir.join("loadorder.txt"),
        b"Starfield.esm\r\nX.esm\r\n",
    )?;
    put(&profile_dir.join("lockedorder.txt"), b"# locked\r\n")?;
    put(
        &profile_dir.join("StarfieldCustom.ini"),
        b"[General]\r\nbAlwaysActive=1\r\n[Papyrus]\r\nbEnableLogging=1\r\nbEnableTrace=1\r\nbLoadDebugInformation=1\r\n",
    )?;
    put(
        &profile_dir.join("StarfieldPrefs.ini"),
        b"[Display]\r\nbFull Screen=0\r\n",
    )?;
    put(
        &profile_dir.join("settings.ini"),
        b"[General]\r\nLocalSaves=true\r\nLocalSettings=true\r\n",
    )?;
    let base_save = profile_dir.join("saves").join("KitKraftBase.sfs");
    put(&base_save, FIXTURE_SAVE)?;
    put(
        &instance.join("mods").join("Console API").join("meta.ini"),
        b"[General]\r\nversion=2.17.0\r\n",
    )?;
    let dev = instance.join("mods").join("KitKraft Dev - X");
    let plugin = dev.join("X.esm");
    put(&plugin, b"TES4 fixture")?;
    put(
        &dev.join("kitkraft-deploy.json"),
        b"{\"kind\": \"kitkraft-dev-mod\", \"plugin\": \"X.esm\"}",
    )?;
    let checkout = root.join("checkout");
    put(&checkout.join("AGENTS.md"), b"# AGENTS.md\n")?;
    let ledger = checkout.join("docs").join("game-observations.md");
    put(&ledger, b"# Game observations (fixture)\n\n## Ledger\n")?;
    let observe_root = checkout.join("observe");
    Ok(Fixture {
        root: root.to_path_buf(),
        instance,
        profile,
        profile_dir,
        base_save,
        plugin,
        observe_root,
        ledger,
    })
}

/// Every file under `dir` with its bytes, sorted (to show a tree is unchanged).
pub fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(b) = std::fs::read(&p) {
                out.push((p, b));
            }
        }
    }
    out.sort();
    out
}
