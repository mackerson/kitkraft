//! The game session form (M6.G4, first half; ADR 0007 Amendment 3): toolkit-free, no
//! drawing and no button wiring yet (the wiring is the next ticket).
//!
//! The gate lets the window start a session with `game.session` in its writing form
//! (`start: true`, `dry_run` absent) only as a confirmed request, follow it with
//! `game.status` (allowed) and end it with `game.stop` only as a confirmed request
//! (its preview is `dry_run: true`). This file is the one panel that builds the
//! confirmed `game.session` and `game.stop` requests (the gate's source scan holds it):
//! [`SessionForm::start_request`] for the start, [`SessionForm::stop_request`] for the
//! stop. The window never launches the game itself: the requests go through
//! `gate::ReadOnlyEngine` and the engine runs them.

use serde_json::{json, Map, Value};

use crate::gate::Request;

/// The session form: where the probes and the game come from, which profile runs them,
/// and where the session writes (empty fields stay out of the arguments; the engine
/// fills the rest with its defaults).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionForm {
    /// The MO2 instance root.
    pub mo2: String,
    /// The profile name (the engine's default is `KitKraft Dev`; never `Default`).
    pub profile: String,
    /// The probes file: the session's rows.
    pub probes: String,
    /// The SHA-256 of the base save, as the engine checks it.
    pub base_save_sha256: String,
    /// The game's folder (its launcher and console settings).
    pub game: String,
    /// The folder the session writes its rows, screenshots and logs into.
    pub observe: String,
    /// The ledger file the session appends its entry to.
    pub ledger: String,
}

impl SessionForm {
    /// The form from a request's arguments (absent or non-string members stay empty).
    pub fn from_args(args: &Value) -> Self {
        let get = |k: &str| {
            args.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        Self {
            mo2: get("mo2"),
            profile: get("profile"),
            probes: get("probes"),
            base_save_sha256: get("base_save_sha256"),
            game: get("game"),
            observe: get("observe"),
            ledger: get("ledger"),
        }
    }

    /// The session's arguments: the filled fields, trimmed; empty ones left out.
    fn args(&self) -> Map<String, Value> {
        let mut a = Map::new();
        for (k, v) in [
            ("mo2", &self.mo2),
            ("profile", &self.profile),
            ("probes", &self.probes),
            ("base_save_sha256", &self.base_save_sha256),
            ("game", &self.game),
            ("observe", &self.observe),
            ("ledger", &self.ledger),
        ] {
            let v = v.trim();
            if !v.is_empty() {
                a.insert(k.into(), json!(v));
            }
        }
        a
    }

    /// The preview: `game.session` with `dry_run: true`. It writes nothing and passes
    /// the gate like a read.
    pub fn preview_request(&self) -> Request {
        let mut a = self.args();
        a.insert("dry_run".into(), json!(true));
        Request::new("game.session", Value::Object(a))
    }

    /// The confirmed start request for the arguments a preview ran with: `dry_run`
    /// dropped, `start: true` added. Called only by the confirm box's button, once per
    /// click: the one place this crate builds a confirmed `game.session` (gate test).
    /// The window starts sessions with `start: true` and follows them with
    /// `game.status`; a synchronous session would block the window's one engine worker.
    pub fn start_request(previewed: &Value) -> Request {
        let mut a = match &previewed {
            Value::Object(m) => Value::Object(m.clone()),
            _ => Value::Object(Map::new()),
        };
        if let Some(m) = a.as_object_mut() {
            m.remove("dry_run");
            m.insert("start".into(), json!(true));
        }
        Request::confirmed("game.session", a)
    }

    /// The stop's preview: `game.stop` for `id` with `dry_run: true`. It writes nothing
    /// and passes the gate like a read.
    pub fn stop_preview(id: &str) -> Request {
        Request::new("game.stop", json!({"id": id, "dry_run": true}))
    }

    /// The confirmed stop request for `id`: the one place this crate builds a
    /// confirmed `game.stop` (gate test).
    pub fn stop_request(id: &str) -> Request {
        Request::confirmed("game.stop", json!({"id": id}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate;

    /// The preview is a dry run with no `start`; the confirm turns those arguments into
    /// the writing form: `start: true`, no `dry_run`, confirmed, and it passes the gate.
    #[test]
    fn the_session_preview_is_a_dry_run_and_the_confirm_starts() {
        let form = SessionForm {
            mo2: "M".into(),
            profile: "KitKraft Dev".into(),
            probes: "P".into(),
            base_save_sha256: "abc".into(),
            game: "G".into(),
            observe: "O".into(),
            ledger: "L".into(),
        };
        let preview = form.preview_request();
        assert_eq!(preview.args["dry_run"], json!(true));
        assert!(preview.args.get("start").is_none(), "{}", preview.args);
        assert!(!preview.is_confirmed());
        assert!(gate::check(&preview).is_ok(), "{}", preview.args);
        let start = SessionForm::start_request(&preview.args);
        assert!(start.is_confirmed());
        assert_eq!(start.args["start"], json!(true));
        assert!(start.args.get("dry_run").is_none(), "{}", start.args);
        assert_eq!(start.args["mo2"], json!("M"));
        assert!(gate::check(&start).is_ok(), "{}", start.args);
        // Empty fields stay out of the arguments.
        let thin = SessionForm {
            mo2: "M".into(),
            ..Default::default()
        };
        assert_eq!(
            thin.preview_request().args,
            json!({"mo2": "M", "dry_run": true})
        );
    }
}
