//! Which request a console reply line answers (ruling R28, `game-20261103-03`).
//!
//! In 2026-11-03-T05 the console's answers to `help` arrived one or more requests late:
//! each reply began with the echo of the request it was sent for, then held the result of
//! an earlier request (T-03's reply held T-01's `KYWD`, T-04's held T-02's `TMLM`, T-07's
//! held T-03's `TERM`). Judging a reply by arrival order graded those rows `fail`.
//!
//! So a reply is split into lines, the echoes are dropped (the command, `Command: <cmd>`,
//! or both on one line, for any command sent in the session), and each remaining line goes
//! to the request it names:
//!
//! - `help <term> ...`: a line holding the term (ASCII case ignored); when several terms
//!   are in one line, the longest wins (`KitKraft Mini Site Menu` over `KitKraft Mini Site`).
//! - A getter (`[<ref>.]get<function> ...`): a line whose first word is the function
//!   (`GetPos: X >> -17.00`, `GetInCell >> 1.00`), more strongly when it also names the
//!   getter's arguments (`X`).
//!
//! Commands of any other kind name nothing: lines no request names stay with the reply they
//! arrived in, and so do the console's failure lines ([`crate::probes::console_failure`]),
//! whatever they name. When the reply's own request and another one name a line equally (the same
//! getter sent twice), the line stays with the reply's own request: the text cannot tell
//! them apart. Between other requests, the oldest one still without a line takes it
//! (T05's late answers came in the order the requests were sent).

/// What a request's answer names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// `help`: the search term, lowercased.
    Term(String),
    /// A getter: its function and arguments, lowercased (`getpos`, `["x"]`).
    Getter {
        /// The function.
        function: String,
        /// The arguments.
        args: Vec<String>,
    },
}

/// What `command`'s answer lines name, if anything.
pub fn subject(command: &str) -> Option<Subject> {
    let c = command.trim();
    let first = c.split_whitespace().next()?;
    let verb = first
        .rsplit('.')
        .next()
        .unwrap_or(first)
        .to_ascii_lowercase();
    let rest = c.get(first.len()..).unwrap_or_default().trim_start();
    if verb == "help" {
        let term = match rest.strip_prefix('"') {
            Some(q) => q.split('"').next().unwrap_or_default(),
            None => rest.split_whitespace().next().unwrap_or_default(),
        }
        .trim();
        return (!term.is_empty()).then(|| Subject::Term(term.to_ascii_lowercase()));
    }
    if verb.len() > 3 && verb.starts_with("get") {
        return Some(Subject::Getter {
            function: verb,
            args: rest
                .split_whitespace()
                .map(str::to_ascii_lowercase)
                .collect(),
        });
    }
    None
}

fn words(line: &str) -> Vec<String> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

impl Subject {
    /// How strongly `line` (no echo in it) answers this subject; `None`: not at all.
    pub fn claims(&self, line: &str) -> Option<usize> {
        match self {
            Subject::Term(t) => line
                .to_ascii_lowercase()
                .contains(t.as_str())
                .then_some(t.len()),
            Subject::Getter { function, args } => {
                let w = words(line);
                if w.first() != Some(function) {
                    return None;
                }
                let named = args
                    .iter()
                    .filter(|a| w.iter().skip(1).any(|x| x == *a))
                    .count();
                // Above any term: the first word is the function itself.
                Some(named.saturating_add(10_000))
            }
        }
    }
}

/// `line` without a leading echo of `command` (`<cmd>`, `Command: <cmd>`, or both);
/// `None` when it does not start with one.
fn strip_echo<'a>(line: &'a str, command: &str) -> Option<&'a str> {
    let c = command.trim();
    if c.is_empty() {
        return None;
    }
    let lc = c.to_ascii_lowercase();
    let mut rest = line.trim_start();
    let mut stripped = false;
    loop {
        let l = rest.to_ascii_lowercase();
        // The command, ending the line or followed by a space.
        if l.starts_with(&lc)
            && l.get(lc.len()..)
                .is_some_and(|n| n.is_empty() || n.starts_with(' '))
        {
            rest = rest.get(lc.len()..).unwrap_or_default().trim_start();
            stripped = true;
            continue;
        }
        if let Some(after) = l.strip_prefix("command:") {
            let skipped = l.len().saturating_sub(after.trim_start().len());
            let tail = rest.get(skipped..).unwrap_or_default();
            if let Some(r) = strip_echo(tail, c) {
                rest = r;
                stripped = true;
                continue;
            }
        }
        break;
    }
    stripped.then_some(rest)
}

/// One request: a command sent for a row's step (its retries are the same request).
#[derive(Debug, Clone)]
pub struct Request {
    /// The row.
    pub row: String,
    /// The step's index in the row.
    pub step: usize,
    /// The command.
    pub command: String,
    subject: Option<Subject>,
    /// The lines that answer it, in the order they arrived.
    pub lines: Vec<String>,
    /// `(command, line)`: each line that came in another request's reply, with that
    /// request's command.
    pub late: Vec<(String, String)>,
}

impl Request {
    /// Whether its answer lines name it (`help`, a getter).
    pub fn keyed(&self) -> bool {
        self.subject.is_some()
    }
}

/// Where one reply's lines went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Routed {
    /// Lines naming the reply's own request.
    pub own: Vec<String>,
    /// Lines no request names (kept with the reply).
    pub unclaimed: Vec<String>,
    /// `(request, line)`: lines naming another request, re-queued to it.
    pub requeued: Vec<(usize, String)>,
    /// The reply had text and all of it was echo.
    pub echo_only: bool,
    /// The reply had no text at all.
    pub empty: bool,
}

impl Routed {
    /// The reply's text without its echoes and the lines re-queued elsewhere.
    pub fn kept(&self) -> String {
        self.own
            .iter()
            .chain(&self.unclaimed)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Every request of a session, and the lines that answer each.
#[derive(Debug, Clone, Default)]
pub struct Correlator {
    requests: Vec<Request>,
}

impl Correlator {
    /// Opens a request; returns its id.
    pub fn open(&mut self, row: &str, step: usize, command: &str) -> usize {
        self.requests.push(Request {
            row: row.to_string(),
            step,
            command: command.trim().to_string(),
            subject: subject(command),
            lines: Vec::new(),
            late: Vec::new(),
        });
        self.requests.len().saturating_sub(1)
    }

    /// Request `id`.
    pub fn request(&self, id: usize) -> Option<&Request> {
        self.requests.get(id)
    }

    /// `line` without the echoes: the reply's own first, then any other command's.
    fn content<'a>(&self, id: usize, line: &'a str) -> &'a str {
        let own = self.requests.get(id).map(|r| r.command.as_str());
        let mut rest = line.trim();
        for c in own
            .into_iter()
            .chain(self.requests.iter().map(|r| r.command.as_str()))
        {
            if let Some(r) = strip_echo(rest, c) {
                rest = r;
            }
        }
        if rest.to_ascii_lowercase().starts_with("command:") {
            // The echo of a command this session did not send.
            return "";
        }
        rest
    }

    /// Routes a reply to request `id`.
    pub fn route(&mut self, id: usize, reply: &str) -> Routed {
        let mut routed = Routed::default();
        let mut any = false;
        let command = self
            .requests
            .get(id)
            .map(|r| r.command.clone())
            .unwrap_or_default();
        for raw in reply.lines() {
            if raw.trim().is_empty() {
                continue;
            }
            any = true;
            let line = self.content(id, raw).to_string();
            if line.is_empty() {
                continue;
            }
            if crate::probes::console_failure("", &line).is_some() {
                // A failure line is the reply's own request's (`Item 'X' not found for
                // parameter` after `prid X`, not after a `help X` sent earlier).
                routed.unclaimed.push(line);
                continue;
            }
            let scores: Vec<(usize, usize)> = self
                .requests
                .iter()
                .enumerate()
                .filter_map(|(i, r)| Some((i, r.subject.as_ref()?.claims(&line)?)))
                .collect();
            let top = scores.iter().map(|(_, s)| *s).max();
            let tied: Vec<usize> = scores
                .iter()
                .filter(|(_, s)| Some(*s) == top)
                .map(|(i, _)| *i)
                .collect();
            let owner = if tied.contains(&id) {
                Some(id)
            } else {
                tied.iter()
                    .copied()
                    .find(|i| self.requests.get(*i).is_some_and(|r| r.lines.is_empty()))
                    .or_else(|| tied.last().copied())
            };
            match owner.and_then(|o| self.requests.get_mut(o).map(|r| (o, r))) {
                Some((o, r)) => {
                    r.lines.push(line.clone());
                    if o == id {
                        routed.own.push(line);
                    } else {
                        r.late.push((command.clone(), line.clone()));
                        routed.requeued.push((o, line));
                    }
                }
                None => routed.unclaimed.push(line),
            }
        }
        routed.empty = !any;
        routed.echo_only = any
            && routed.own.is_empty()
            && routed.unclaimed.is_empty()
            && routed.requeued.is_empty();
        routed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subjects_come_from_help_terms_and_getters() {
        assert_eq!(
            subject("help \"KitKraft Mini Site\" 4"),
            Some(Subject::Term("kitkraft mini site".into()))
        );
        assert_eq!(
            subject("help KitKraftLocTypeMiniSite 4"),
            Some(Subject::Term("kitkraftloctypeminisite".into()))
        );
        assert_eq!(
            subject("player.getincell 0021966F"),
            Some(Subject::Getter {
                function: "getincell".into(),
                args: vec!["0021966f".into()]
            })
        );
        assert_eq!(subject("coc CityNewAtlantisApartmentTEMPLATE"), None);
        assert_eq!(subject("prid 06001000"), None);
        assert_eq!(subject("help"), None);
    }

    /// game-20261103-03 (2026-11-03-T05): T-03's reply, with T-03's echo and T-01's result.
    #[test]
    fn a_line_for_an_earlier_request_is_requeued_to_it() {
        let mut c = Correlator::default();
        let t01 = c.open("T-01", 0, "help KitKraftLocTypeMiniSite 4");
        let r = c.route(t01, "help KitKraftLocTypeMiniSite 4");
        assert!(r.echo_only);
        let t03 = c.open("T-03", 0, "help \"KitKraft Site Terminal\" 4");
        let r = c.route(
            t03,
            "help \"KitKraft Site Terminal\" 4\nKYWD: KitKraftLocTypeMiniSite (09001000) ''",
        );
        assert!(r.own.is_empty() && !r.echo_only, "{r:?}");
        assert_eq!(r.requeued.len(), 1);
        assert_eq!(r.requeued.first().map(|x| x.0), Some(t01));
        let a = c.request(t01).unwrap();
        assert_eq!(
            a.lines,
            vec!["KYWD: KitKraftLocTypeMiniSite (09001000) ''".to_string()]
        );
        assert_eq!(
            a.late.first().map(|x| x.0.as_str()),
            Some("help \"KitKraft Site Terminal\" 4")
        );
        assert!(c.request(t03).unwrap().lines.is_empty());
    }

    /// T-04's reply held T-02's `TMLM`; T-07's held T-03's `TERM`, and its own `LCTN` came
    /// next. The longer term takes a line both name.
    #[test]
    fn the_longest_term_takes_a_line_and_answers_in_order_stay() {
        let mut c = Correlator::default();
        let t02 = c.open("T-02", 0, "help \"KitKraft Mini Site Menu\" 4");
        let t03 = c.open("T-03", 0, "help \"KitKraft Site Terminal\" 4");
        let t07 = c.open("T-07", 0, "help \"KitKraft Mini Site\" 4");
        let r = c.route(
            t07,
            "help \"KitKraft Mini Site\" 4\nTERM:  (09001002) 'KitKraft Site Terminal'",
        );
        assert_eq!(r.requeued.first().map(|x| x.0), Some(t03));
        let r = c.route(
            t07,
            "help \"KitKraft Mini Site\" 4\nTMLM:  (09001001) 'KitKraft Mini Site Menu'\nLCTN: KitKraftMiniSiteLocation (09001006) 'KitKraft Mini Site'",
        );
        assert_eq!(r.requeued.first().map(|x| x.0), Some(t02));
        assert_eq!(
            r.own,
            vec!["LCTN: KitKraftMiniSiteLocation (09001006) 'KitKraft Mini Site'".to_string()]
        );
    }

    #[test]
    fn echoes_in_every_observed_form_are_dropped() {
        let mut c = Correlator::default();
        let g = c.open("T-08", 1, "player.getincell 0021966F");
        let r = c.route(
            g,
            "player.getincell 0021966F Command: player.getincell 0021966F",
        );
        assert!(r.echo_only, "{r:?}");
        let r = c.route(
            g,
            "player.getincell 0021966F\nCommand: player.getincell 0021966F\n",
        );
        assert!(r.echo_only, "{r:?}");
        let r = c.route(
            g,
            "player.getincell 0021966F Command: player.getincell 0021966F GetInCell >> 1.00",
        );
        assert_eq!(r.own, vec!["GetInCell >> 1.00".to_string()]);
        assert!(c.route(g, "").empty);
        // A getter's answer in order, and one for an older getter of another function.
        let g2 = c.open("T-09", 1, "player.getincell 0006BB29");
        let p = c.open("T-04", 1, "getpos x");
        let r = c.route(
            p,
            "getpos x\nCommand: getpos x\nGetPos: X >> -17.00\nGetInCell >> 1.00",
        );
        assert_eq!(r.own, vec!["GetPos: X >> -17.00".to_string()]);
        // Two getincells name it equally; the one still without a line takes it.
        assert_eq!(r.requeued.first().map(|x| x.0), Some(g2));
        // A line nothing names stays with its reply.
        let s = c.open("T-05", 0, "load KitKraftSession-20261103-01");
        let r = c.route(s, "load KitKraftSession-20261103-01\nSave file not found.");
        assert_eq!(r.unclaimed, vec!["Save file not found.".to_string()]);
        assert_eq!(r.kept(), "Save file not found.");
        // A failure line naming a `help` term stays with the reply it came in.
        let h = c.open("T-01", 0, "help KitKraftCrate 4");
        let _ = c.route(h, "help KitKraftCrate 4");
        let q = c.open("T-02", 0, "prid KitKraftCrate");
        let r = c.route(
            q,
            "prid KitKraftCrate
Item 'KitKraftCrate' not found for parameter ObjectReferenceID.",
        );
        assert!(r.requeued.is_empty() && r.unclaimed.len() == 1, "{r:?}");
        // An echo of a command the session did not send.
        let r = c.route(s, "Command: GetSFSEVersion");
        assert!(r.echo_only, "{r:?}");
    }
}
