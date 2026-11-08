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
//! A getter's line goes to the one request still waiting for an answer (fewer lines than
//! sends) that can claim it. When two or more can (the same getter sent twice, or
//! `GetInCell`, whose answer names no argument), the line is **ambiguous**: it goes to none,
//! and a step left with no line of its own is `inconclusive (ambiguous reply)` (review H1).
//! A getter with an axis argument (`getpos x`) claims no line naming another axis. A line
//! for a getter that already has all its answers is noted, not graded.
//!
//! A `help` line holding the reply's own term is its own, even when a longer term also
//! names it; the longer term's request gets a copy when it has no line yet (review M1).
//! Between other `help` requests the longest term wins, then the oldest one still without
//! a line (T05's late answers came in the order the requests were sent).
//!
//! Commands of any other kind name nothing: lines no request names stay with the reply
//! they arrived in. So do the console's failure lines ([`crate::probes::console_failure`]),
//! unless the item they quote is a word of another request's command and not of this one;
//! then they are re-queued to every such request (review M2, H3), and none of their rows
//! can keep a `pass`: each becomes `inconclusive (late failure)`.
//!
//! Before that, send order: while a getter sent before the reply's request has no line
//! claimed for it, and no request sent after it has one either, the getter's answer is not
//! known to have arrived. It may simply be unclaimed here: its failure line may have gone
//! elsewhere by the item rule, or its answer may name the function in a form the getter
//! does not claim, or have been lost to echo-only replies. A failure line arriving then
//! goes to every request sent from the one after the newest answered request up to the
//! reply's own (not included), and to every other request whose command spells the item
//! it quotes. Each of their rows is re-opened (`inconclusive (late failure)`), and the
//! reply's own row is inconclusive, not failed: an on-time failure after such a getter is
//! a lost fail, never a false pass (fix review FR2 to FR4). A request whose own failure
//! line stayed with it counts as answered.
//!
//! A getter or `help` whose own failure line was diverted this way has no answer of its
//! own and is re-sent like an echo-only request (R20), so the commands a session sends can
//! differ from review H2's "a failure line is not re-sent". Only those read-only commands
//! are re-sent; the row keeps the diverted failure as an inconclusive part, so a later line
//! for the request cannot make it a pass.
//!
//! The send-order assumption is the owner's ruling of 2026-11-07 (`plan/STATUS.md`, "Owner
//! rulings, 2026-11-07 (midday)", item 2), drawn from `game-20261103-04` and `-08`; no
//! session has yet been designed to test it. The rows it decides are those where a failure
//! line arrives while an older getter is still waiting (FR2, FR3 and FR4 in
//! `tests/session.rs`). A controller session on `KitKraft Dev` is to confirm the ordering;
//! if it contradicts the ruling, the ruling reverts and this rule goes with it.

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
                // An axis argument (`getpos x`) claims no line naming another axis
                // (`GetPos: Z >> ...`, review H1).
                let axes = ["x", "y", "z"];
                let named_axes: Vec<&String> = w
                    .iter()
                    .skip(1)
                    .filter(|x| axes.contains(&x.as_str()))
                    .collect();
                if args.iter().any(|a| {
                    axes.contains(&a.as_str()) && !named_axes.is_empty() && !named_axes.contains(&a)
                }) {
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

/// `(command, line)`: a line, and the command whose reply it came in.
pub type Via = (String, String);

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
    /// A failure line in its own reply stayed with it: it was answered (send order).
    failed_here: bool,
    /// How many times it was sent (each send may bring one answer, late or not).
    pub sent: usize,
    /// The lines that answer it, in the order they arrived.
    pub lines: Vec<String>,
    /// `(command, line)`: each line that came in another request's reply, with that
    /// request's command.
    pub late: Vec<(String, String)>,
    /// `(line, other requests)`: lines it could claim but so could another request still
    /// waiting for an answer; given to none (ruling R28, review H1).
    pub ambiguous: Vec<(String, Vec<usize>)>,
    /// `(command, line)`: console failure lines naming this request's item that came in
    /// another request's reply (review M2).
    pub failures: Vec<(String, String)>,
    /// `(command, line)`: lines naming it that came after it had all its answers, in
    /// another request's reply (noted, never graded; review L5).
    pub surplus: Vec<(String, String)>,
}

impl Request {
    /// Whether its answer lines name it (`help`, a getter).
    pub fn keyed(&self) -> bool {
        self.subject.is_some()
    }

    fn is_getter(&self) -> bool {
        matches!(self.subject, Some(Subject::Getter { .. }))
    }

    /// A getter still waiting for an answer to one of its sends.
    fn waiting(&self) -> bool {
        self.lines.len() < self.sent
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
    /// `(requests, line)`: getter lines more than one waiting request could claim.
    pub ambiguous: Vec<(Vec<usize>, String)>,
    /// `(request, line)`: failure lines naming another request's item, re-queued to it.
    pub failures: Vec<(usize, String)>,
    /// `(requests, chosen, line)`: `help` lines several terms name equally.
    pub ties: Vec<(Vec<usize>, usize, String)>,
    /// `(request, line)`: lines for a getter that already had all its answers.
    pub surplus: Vec<(usize, String)>,
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

/// The item a console failure line quotes (`Item 'X' not found ...`, `Script command "x"
/// not found`), lowercased.
fn failure_item(line: &str) -> Option<String> {
    let l = line.to_ascii_lowercase();
    let quoted = |after: &str, q: char| -> Option<String> {
        let at = l.find(after)?;
        let rest = l.get(at.saturating_add(after.len())..)?;
        let rest = rest.strip_prefix(q)?;
        let end = rest.find(q)?;
        rest.get(..end).map(str::to_string)
    };
    quoted("item ", '\'').or_else(|| quoted("script command ", '"'))
}

/// Whether `command` names `item` (lowercased) as a whole word or as its quoted text.
fn names_item(command: &str, item: &str) -> bool {
    let c = command.to_ascii_lowercase();
    c.split('"').skip(1).step_by(2).any(|q| q.trim() == item)
        || c.split(|ch: char| ch.is_whitespace() || ch == '"')
            .any(|w| w == item)
}

/// Every request of a session, and the lines that answer each.
#[derive(Debug, Clone, Default)]
pub struct Correlator {
    requests: Vec<Request>,
}

impl Correlator {
    /// Opens a request (sent once); returns its id.
    pub fn open(&mut self, row: &str, step: usize, command: &str) -> usize {
        self.requests.push(Request {
            row: row.to_string(),
            step,
            command: command.trim().to_string(),
            subject: subject(command),
            failed_here: false,
            sent: 1,
            lines: Vec::new(),
            late: Vec::new(),
            ambiguous: Vec::new(),
            failures: Vec::new(),
            surplus: Vec::new(),
        });
        self.requests.len().saturating_sub(1)
    }

    /// Request `id` was sent again.
    pub fn resend(&mut self, id: usize) {
        if let Some(r) = self.requests.get_mut(id) {
            r.sent = r.sent.saturating_add(1);
        }
    }

    /// Request `id`.
    pub fn request(&self, id: usize) -> Option<&Request> {
        self.requests.get(id)
    }

    /// Every request, in the order opened.
    pub fn requests(&self) -> &[Request] {
        &self.requests
    }

    /// Takes the failure lines and surplus lines re-queued so far: `(request, failures,
    /// surplus)`.
    pub fn take_notes(&mut self) -> Vec<(usize, Vec<Via>, Vec<Via>)> {
        self.requests
            .iter_mut()
            .enumerate()
            .filter(|(_, r)| !r.failures.is_empty() || !r.surplus.is_empty())
            .map(|(i, r)| {
                (
                    i,
                    std::mem::take(&mut r.failures),
                    std::mem::take(&mut r.surplus),
                )
            })
            .collect()
    }

    /// The requests sent before `id` that a line arriving now may answer, in send order
    /// (owner ruling 2026-11-07): when a getter older than `id` has no line claimed for it
    /// and no later request has one either, its answer is not known to have arrived (a
    /// getter's answer is a line, though the correlator may not have claimed it; a `help`
    /// that finds nothing may print none, so a `help` does not count), and neither is any
    /// answer sent after it. Then every request since the newest one with a line, up to
    /// `id` (not included), may own the line. Empty when no such getter is known.
    fn unanswered_before(&self, id: usize) -> Vec<usize> {
        let from = self
            .requests
            .iter()
            .rposition(|r| !r.lines.is_empty() || r.failed_here)
            .map_or(0, |a| a.saturating_add(1));
        let oldest = (from..id).find(|i| self.requests.get(*i).is_some_and(Request::is_getter));
        // Every request sent since that getter is as unanswered as it is: all of them are
        // candidates (re-opening one loses a pass, never makes one; fix review S1).
        match oldest {
            Some(_) => (from..id).collect(),
            None => Vec::new(),
        }
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

    fn give(&mut self, to: usize, id: usize, command: &str, line: &str, routed: &mut Routed) {
        if let Some(r) = self.requests.get_mut(to) {
            r.lines.push(line.to_string());
            if to == id {
                routed.own.push(line.to_string());
            } else {
                r.late.push((command.to_string(), line.to_string()));
                routed.requeued.push((to, line.to_string()));
            }
        }
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
                // A failure line is the reply's own request's, unless the item it quotes
                // is in another request's command and not in this one (review M2).
                // The item must be a whole word of the command (or its quoted text). Every
                // such request gets it: the text cannot tell which of them failed, so no
                // candidate's row may keep a pass (review H3 step 3; owner review M1).
                // First, send order: while an older getter's answer is not known to have
                // arrived, every request sent since the newest answered one may own the
                // line (fix review FR2 to FR4), and so may every request spelling its item
                // (fix review S1, RP1 to RP3); each candidate is re-opened.
                let item = failure_item(&line);
                let before = self.unanswered_before(id);
                let others: Vec<usize> = match item.as_ref() {
                    _ if !before.is_empty() => {
                        let mut all = before;
                        if let Some(item) = item.as_ref() {
                            for (i, r) in self.requests.iter().enumerate() {
                                if i != id && !all.contains(&i) && names_item(&r.command, item) {
                                    all.push(i);
                                }
                            }
                        }
                        all
                    }
                    Some(item) if !names_item(&command, item) => self
                        .requests
                        .iter()
                        .enumerate()
                        .filter(|(i, r)| *i != id && names_item(&r.command, item))
                        .map(|(i, _)| i)
                        .collect(),
                    _ => Vec::new(),
                };
                if others.is_empty() {
                    // Kept, it is this request's answer (a getter's `Item ... not found`).
                    if let Some(r) = self.requests.get_mut(id) {
                        r.failed_here = true;
                    }
                    routed.unclaimed.push(line);
                    continue;
                }
                for o in others {
                    if let Some(r) = self.requests.get_mut(o) {
                        r.failures.push((command.clone(), line.clone()));
                        routed.failures.push((o, line.clone()));
                    }
                }
                continue;
            }
            let scores: Vec<(usize, usize)> = self
                .requests
                .iter()
                .enumerate()
                .filter_map(|(i, r)| Some((i, r.subject.as_ref()?.claims(&line)?)))
                .collect();
            let Some(top) = scores.iter().map(|(_, s)| *s).max() else {
                routed.unclaimed.push(line);
                continue;
            };
            let tied: Vec<usize> = scores
                .iter()
                .filter(|(_, s)| *s == top)
                .map(|(i, _)| *i)
                .collect();
            let getter = tied
                .first()
                .and_then(|i| self.requests.get(*i))
                .is_some_and(Request::is_getter);
            if getter {
                // A getter's line goes to the one request still waiting that can claim
                // it; with two or more, it is ambiguous and goes to none (review H1); with
                // none, it is a surplus answer, noted only (review L5).
                let waiting: Vec<usize> = tied
                    .iter()
                    .copied()
                    .filter(|i| self.requests.get(*i).is_some_and(Request::waiting))
                    .collect();
                match waiting.as_slice() {
                    [one] => self.give(*one, id, &command, &line, &mut routed),
                    [] => {
                        let to = tied.last().copied().unwrap_or(id);
                        if let Some(r) = self.requests.get_mut(to) {
                            if to != id {
                                r.surplus.push((command.clone(), line.clone()));
                            }
                        }
                        routed.surplus.push((to, line));
                    }
                    many => {
                        for i in many {
                            let others: Vec<usize> =
                                many.iter().copied().filter(|j| j != i).collect();
                            if let Some(r) = self.requests.get_mut(*i) {
                                r.ambiguous.push((line.clone(), others));
                            }
                        }
                        routed.ambiguous.push((many.to_vec(), line));
                    }
                }
                continue;
            }
            // `help`: a line holding the own request's term is its own (`help T` lists
            // every form whose name holds T), even when a longer term also names it; that
            // request, if it has no line yet, gets a copy (review M1).
            let own_claims = scores.iter().any(|(i, _)| *i == id);
            let longest = if tied.contains(&id) {
                id
            } else {
                tied.iter()
                    .copied()
                    .find(|i| self.requests.get(*i).is_some_and(|r| r.lines.is_empty()))
                    .or_else(|| tied.last().copied())
                    .unwrap_or(id)
            };
            if tied.len() > 1 {
                routed.ties.push((tied.clone(), longest, line.clone()));
            }
            if own_claims {
                self.give(id, id, &command, &line, &mut routed);
                if longest != id
                    && self
                        .requests
                        .get(longest)
                        .is_some_and(|r| r.lines.is_empty())
                {
                    self.give(longest, id, &command, &line, &mut routed);
                }
            } else {
                self.give(longest, id, &command, &line, &mut routed);
            }
        }
        routed.empty = !any;
        routed.echo_only = any
            && routed.own.is_empty()
            && routed.unclaimed.is_empty()
            && routed.requeued.is_empty()
            && routed.ambiguous.is_empty()
            && routed.failures.is_empty()
            && routed.surplus.is_empty();
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
        // The TMLM line holds T-07's own term: T-07 keeps it, and T-02 (no line yet) gets a
        // copy (review M1).
        assert_eq!(r.requeued.first().map(|x| x.0), Some(t02));
        assert_eq!(
            r.own,
            vec![
                "TMLM:  (09001001) 'KitKraft Mini Site Menu'".to_string(),
                "LCTN: KitKraftMiniSiteLocation (09001006) 'KitKraft Mini Site'".to_string()
            ]
        );
        assert_eq!(r.ties.len(), 0);
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
