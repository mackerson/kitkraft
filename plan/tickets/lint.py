#!/usr/bin/env python3
"""Lint KitKraft tickets: the rules of plan/tickets/README.md ("Writing a good ticket").

    python3 plan/tickets/lint.py [--min N] [--all] FILE_OR_DIR...

Checks each ticket TOML (a directory means its `*.toml` files) and exits 1 on any error;
warnings are printed but do not fail. `--min N` also fails when fewer than N tickets were
linted. Merged tickets (plan/tickets/README.md's table says `merged`, or `git log` has a
"Merge ticket/<ID>:" commit) are history: skipped unless `--all`, which lints them too but
does not ask their tests to be new. Drafts (`plan/tickets/drafts/<slug>.toml`, `id =
"DRAFT"`) get a few stricter checks: a brief that carries context and names files that exist.

What it checks:

- the TOML parses; required fields (`id`, `title`, `brief`, non-empty `owns`, at least one
  `[[accept]]` with `name` and `cmd`, names unique); `timeout_min`, `budget_h`, `hold`,
  `gate` sane; `after` names tickets in plan/tickets (no cycles); `base` looks like a ref.
- acceptance commands can fail and cannot pass vacuously: a `cargo test` with a name filter
  checks the count (`grep '^test result: ok. 1 passed'`, a `<name> ... ok` line, or a line
  a `--nocapture` test prints); `--exact` with k names expects k passed; nothing ends in
  `|| true`, `tee` or `echo`; no statement's exit status is dropped by `;` unless its
  output is grepped; nothing reaches the network, the game or a host path. A corpus test
  run with `--ignored` should grep a line it prints (it passes when it skips).
- tests the acceptance names (`--exact`) do not exist in the repo yet, unless the ticket
  continues a branch (`base`) or the brief says the test exists (existing, unchanged, still,
  keep, ignored...); the agent can write each new test: its file is under `owns`.
- owned paths exist or have an existing parent; never plan/STATUS.md (always allowed), the
  ticket files or this lint, build output, a game Data folder or a binary/Bethesda file.
- repo paths the brief names in backticks exist or are owned (warning).
- the KK-002 lesson: if the brief lets a test be `#[ignore]`d, acceptance must not require
  it to pass (`1 passed` with no `1 ignored` outlet), nor require its type elsewhere.

Stdlib only; needs Python 3.11+ (tomllib), so it runs under WSL/Linux and in the grader.
"""
import argparse, fnmatch, os, re, shlex, subprocess, sys

try:
    import tomllib
except ImportError:  # pragma: no cover
    sys.exit("lint.py needs Python 3.11+ (tomllib); run it under WSL/Linux: python3 plan/tickets/lint.py")

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
TICKETS = os.path.join(ROOT, "plan", "tickets")

KNOWN_KEYS = {"id", "title", "timeout_min", "gate", "after", "hold", "budget_h", "base", "owns", "brief", "accept"}
ACCEPT_KEYS = {"name", "cmd"}
ID_RE = re.compile(r"^KK-\d{3}$")
REF_RE = re.compile(r"^(?!-)(?!.*\.\.)(?!.*//)(?!.*@\{)[A-Za-z0-9._/-]+(?<![./])(?<!\.lock)$")
TOP_DIRS = ("crates/", "apps/", "docs/", "xtask/", "plan/", "prototypes/", ".githooks/")
BINARY_EXT = {".esm", ".esp", ".esl", ".ba2", ".bsa", ".pex", ".nif", ".dds", ".wem", ".bk2", ".exe", ".dll",
              ".pdb", ".so", ".dylib", ".lib", ".a", ".o", ".bin", ".zip", ".7z"}
BUILD_DIRS = ("target", ".targets", "observe", ".git", "out", "dist", "kk-cache", ".scratch")
GUARD_WORDS = re.compile(r"\b(exist(s|ing)?|unchanged|still|keeps?|already|regression|must (keep )?pass|"
                         r"checks|asserts|un-?ignore|ignored|#\[ignore)", re.I)
NEW_TEST = re.compile(r"exact names?|these names|new test|tests? in a new file|add (a |the |both )?tests?", re.I)
GUARD_NAME = re.compile(r"unchanged|still|existing|regression|keeps", re.I)
PERMIT = [
    re.compile(r"\b(may|can|could) (also )?be (marked |left )?(`?#\[ignore|ignored)", re.I),
    re.compile(r"\bmark (it|them|that test|only that test|both( tests)?|the tests?|each|\w+ tests?)\s+`?#\[ignore", re.I),
    re.compile(r"\b(may|can) (be )?ignore\b", re.I),
    re.compile(r"\bif\b.{0,120}\b(cannot|can.t|does not|doesn.t|fails? to)\s+pass\b.{0,300}#\[ignore", re.I | re.S),
    re.compile(r"\bignore (it|them|that test|the test|both)\b", re.I),
]
# Options that take a value, before and after `cargo test`'s `--`.
CARGO_VALUE_OPTS = {"-p", "--package", "--test", "--bin", "--example", "--bench", "--features", "-F", "--target",
                    "--target-dir", "-j", "--jobs", "--manifest-path", "--profile", "--color", "--message-format",
                    "-Z", "--config", "--exclude"}
HARNESS_VALUE_OPTS = {"--test-threads", "--skip", "--format", "-Z", "--color", "--logfile", "--report-time"}
NO_FAIL_LAST = {"tee", "echo", "true", ":", "printf", "cat", "sort", "head", "tail", "wc", "uniq"}


class Report:
    def __init__(self):
        self.errors = self.warnings = 0

    def say(self, path, level, msg):
        if level == "error":
            self.errors += 1
        else:
            self.warnings += 1
        print(f"{os.path.relpath(path, ROOT)}: {level}: {msg}")


# ---- the repo: tests, packages, merged tickets ----------------------------------------------

_tests = None


def repo_tests():
    """{test fn name: [files]} for every #[test] fn under crates/, apps/ and xtask/."""
    global _tests
    if _tests is not None:
        return _tests
    _tests = {}
    pat = re.compile(r"#\[(?:tokio::)?test\]\s*(?:#\[[^\n]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)")
    for top in ("crates", "apps", "xtask"):
        for d, dirs, files in os.walk(os.path.join(ROOT, top)):
            dirs[:] = [x for x in dirs if x not in BUILD_DIRS]
            for f in files:
                if f.endswith(".rs"):
                    p = os.path.join(d, f)
                    try:
                        text = open(p, encoding="utf-8", errors="replace").read()
                    except OSError:
                        continue
                    rel = os.path.relpath(p, ROOT).replace(os.sep, "/")
                    for m in pat.finditer(text):
                        _tests.setdefault(m.group(1), []).append(rel)
                        _paths.add(f"{module_path(rel)}::{m.group(1)}".lstrip(":"))
    return _tests


_paths = set()


def module_path(rel):
    """`crates/engine/src/handlers/newform_tests.rs` -> `handlers::newform_tests` (what a cargo
    test filter matches, less any inline `mod tests`)."""
    parts = rel[:-3].split("/")
    if "src" in parts:
        parts = parts[parts.index("src") + 1:]
    elif "tests" in parts:
        return ""   # an integration test's names start at the file's root
    if parts and parts[-1] in ("lib", "main", "mod"):
        parts = parts[:-1]
    return "::".join(parts)


def filter_matches(f):
    """Existing tests a cargo test filter (a substring of the test path) matches, approximately."""
    repo_tests()
    bare = f.split("::")[-1]
    return sorted({p for p in _paths if f in p or (bare in p.rsplit("::", 1)[-1] and "::" in f
                                                    and f.rsplit("::", 1)[0].split("::")[-1] in p)})


_packages = None


def packages():
    """{package name: its directory, repo-relative}."""
    global _packages
    if _packages is not None:
        return _packages
    _packages = {}
    for top in ("crates", "apps"):
        base = os.path.join(ROOT, top)
        for d in sorted(os.listdir(base)) if os.path.isdir(base) else []:
            _read_package(f"{top}/{d}")
    _read_package("xtask")
    return _packages


def _read_package(rel):
    p = os.path.join(ROOT, rel, "Cargo.toml")
    try:
        name = tomllib.load(open(p, "rb")).get("package", {}).get("name")
    except (OSError, tomllib.TOMLDecodeError):
        return
    if name:
        _packages[name] = rel


def merged_ids():
    out = set()
    try:
        for line in open(os.path.join(TICKETS, "README.md"), encoding="utf-8"):
            m = re.match(r"^\|\s*\[(KK-\d+)\]\([^)]*\)\s*\|.*\|\s*([^|]*?)\s*\|\s*$", line)
            if m and m.group(2).startswith("merged"):
                out.add(m.group(1))
    except OSError:
        pass
    if os.path.exists(os.path.join(ROOT, ".git")):
        try:
            log = subprocess.run(["git", "-C", ROOT, "log", "--format=%s", "HEAD"], capture_output=True,
                                 text=True, timeout=30).stdout
            out |= set(re.findall(r"^Merge ticket/(KK-\d+):", log, re.M))
        except (OSError, subprocess.SubprocessError):
            pass
    return out


def all_tickets():
    """{ID: ticket dict} for plan/tickets/KK-*.toml (unparsable ones left out)."""
    out = {}
    for f in sorted(os.listdir(TICKETS)):
        if re.match(r"^KK-\d+\.toml$", f):
            try:
                t = tomllib.load(open(os.path.join(TICKETS, f), "rb"))
            except (OSError, tomllib.TOMLDecodeError):
                t = {}
            out[f[:-5]] = t
    return out


# ---- paths ------------------------------------------------------------------------------------

def literal(pattern):
    return re.split(r"[*?\[]", pattern, maxsplit=1)[0]


def in_scope(path, owns):
    """kk_grade.in_scope: what the scope check lets the agent change."""
    return any(path == p or fnmatch.fnmatch(path, p) or path.startswith(p.rstrip("/") + "/") for p in owns)


def exists(rel):
    return os.path.exists(os.path.join(ROOT, rel))


def check_owns(t, path, rep):
    owns = t.get("owns")
    if not isinstance(owns, list) or not owns or not all(isinstance(p, str) and p.strip() for p in owns):
        rep.say(path, "error", "owns must be a non-empty list of repo paths")
        return []
    for p in owns:
        lit = literal(p)
        parts = [x for x in lit.rstrip("/").split("/") if x]
        if p.startswith("/") or "\\" in p or ".." in parts or re.match(r"^[A-Za-z]:", p):
            rep.say(path, "error", f"owns `{p}`: a repo-relative path with `/` only")
            continue
        if p.rstrip("/") == "plan/STATUS.md":
            rep.say(path, "error", "owns plan/STATUS.md: its one Landed line is always allowed; don't list it")
            continue
        if lit.rstrip("/") in ("", "crates", "apps", "docs", "plan") or p in ("*", "**"):
            rep.say(path, "warning", f"owns `{p}` is very broad: name the files or folders the ticket changes")
        if any(x in BUILD_DIRS for x in parts):
            rep.say(path, "error", f"owns `{p}`: build output or local-only folder")
        if any(x == "Data" or x.lower() in ("steamapps", "my games", "mo2-starfield") for x in parts):
            rep.say(path, "error", f"owns `{p}`: never anything under a game Data folder or MO2")
        if os.path.splitext(lit.rstrip("/"))[1].lower() in BINARY_EXT:
            rep.say(path, "error", f"owns `{p}`: binaries and Bethesda files never go in the repo")
        if lit.startswith("plan/tickets/") and not lit.startswith("plan/tickets/drafts/"):
            rep.say(path, "error", f"owns `{p}`: a ticket never changes the tickets, their README or this lint")
        if lit.rstrip("/") in ("plan/tickets",):
            rep.say(path, "error", f"owns `{p}`: a ticket never changes the tickets, their README or this lint")
        here = lit.rstrip("/")
        if any(fnmatch.fnmatch(x, p) for x in _glob_candidates(p)):
            continue
        parent = os.path.dirname(here)
        if here and (exists(here) or parent == "" or os.path.isdir(os.path.join(ROOT, parent))):
            continue
        rep.say(path, "error", f"owns `{p}`: neither it nor its parent folder exists")
    return owns


def _glob_candidates(p):
    if not re.search(r"[*?\[]", p):
        return []
    base = os.path.dirname(literal(p)) or "."
    out = []
    for d, dirs, files in os.walk(os.path.join(ROOT, base)):
        dirs[:] = [x for x in dirs if x not in BUILD_DIRS]
        out += [os.path.relpath(os.path.join(d, f), ROOT).replace(os.sep, "/") for f in files + dirs]
        if len(out) > 5000:
            break
    return out


# ---- shell parsing ------------------------------------------------------------------------------

def parse_shell(cmd):
    """[statement], statement = [(op, negated, [[word]] pipeline)], op the && / || before it."""
    lex = shlex.shlex(cmd, posix=True, punctuation_chars=";&|<>()")
    lex.whitespace_split = True
    lex.commenters = ""
    try:
        toks = list(lex)
    except ValueError as e:
        raise ValueError(f"does not parse as shell: {e}")
    stmts, stmt, words, pipe, op, neg = [], [], [], [], None, False
    skip = False

    def end_cmd():
        nonlocal words
        if words:
            pipe.append(words)
        words = []

    def end_pipe(next_op):
        nonlocal pipe, op, neg
        end_cmd()
        if pipe:
            stmt.append((op, neg, pipe))
        pipe, op, neg = [], next_op, False

    for i, tok in enumerate(toks):
        if skip:
            skip = False
            continue
        if tok in (";", "&", "\n"):
            end_pipe(None)
            if stmt:
                stmts.append(stmt)
            stmt = []
        elif tok in ("&&", "||"):
            end_pipe(tok)
        elif tok == "|":
            end_cmd()
        elif tok in (">", ">>", "<", ">&", "&>", "<&", ">|", "&>>", "<<", "<<<", "2>"):
            if words and words[-1].isdigit():
                words.pop()
            skip = True
        elif tok in ("(", ")"):
            continue
        elif tok == "!" and not words and not pipe:
            neg = True
        else:
            words.append(tok)
    end_pipe(None)
    if stmt:
        stmts.append(stmt)
    return stmts


def parse_grep(words):
    """(pattern, files, inverted) for a grep command, or None."""
    if not words or os.path.basename(words[0]) not in ("grep", "egrep", "fgrep"):
        return None
    pat, files, inverted, i = None, [], False, 1
    while i < len(words):
        w = words[i]
        if w == "--":
            i += 1
            break
        if w in ("-e", "--regexp"):
            pat, i = (words[i + 1] if i + 1 < len(words) else ""), i + 2
            continue
        if w in ("-f", "-m", "--max-count", "-A", "-B", "-C"):
            i += 2
            continue
        if w.startswith("--"):
            inverted |= w == "--invert-match"
            i += 1
            continue
        if w.startswith("-") and len(w) > 1:
            inverted |= "v" in w
            i += 1
            continue
        break
    rest = words[i:]
    if pat is None:
        if not rest:
            return None
        pat, rest = rest[0], rest[1:]
    return pat, rest, inverted


def parse_cargo_test(words):
    """{'pkg', 'target', 'filters', 'exact', 'ignored', 'nocapture'} for `cargo test ...`, or None."""
    if len(words) < 2 or os.path.basename(words[0]) != "cargo":
        return None
    i = 1
    if words[i].startswith("+"):
        i += 1
    if i >= len(words) or words[i] != "test":
        return None
    i += 1
    c = {"pkg": None, "target": None, "filters": [], "exact": False, "ignored": False, "nocapture": False,
         "list": False}
    while i < len(words) and words[i] != "--":
        w = words[i]
        if w in ("-p", "--package") and i + 1 < len(words):
            c["pkg"] = words[i + 1]
        elif w == "--test" and i + 1 < len(words):
            c["target"] = ("test", words[i + 1])
        elif w == "--bin" and i + 1 < len(words):
            c["target"] = ("bin", words[i + 1])
        elif w == "--lib":
            c["target"] = ("lib", None)
        elif w == "--no-run":
            c["list"] = True
        if w in CARGO_VALUE_OPTS:
            i += 2
            continue
        if w.startswith("-p") and len(w) > 2:
            c["pkg"] = w[2:]
        elif not w.startswith("-"):
            c["filters"].append(w)
        i += 1
    i += 1
    while i < len(words):
        w = words[i]
        if w in HARNESS_VALUE_OPTS:
            i += 2
            continue
        if w == "--exact":
            c["exact"] = True
        elif w in ("--ignored", "--include-ignored"):
            c["ignored"] = True
        elif w in ("--nocapture", "--show-output"):
            c["nocapture"] = True
        elif w == "--list":
            c["list"] = True
        elif not w.startswith("-"):
            c["filters"].append(w)
        i += 1
    return c


# ---- acceptance -----------------------------------------------------------------------------------

def check_accept(t, path, owns, is_merged, brief, rep):
    acc = t.get("accept")
    if not isinstance(acc, list) or not acc:
        rep.say(path, "error", "no [[accept]] commands: acceptance commands decide whether the ticket is done")
        return []
    names, strict, outlets, named, quoted_types = set(), [], 0, [], []
    for n, a in enumerate(acc, 1):
        if not isinstance(a, dict):
            rep.say(path, "error", f"[[accept]] #{n} is not a table")
            continue
        extra = set(a) - ACCEPT_KEYS
        if extra:
            rep.say(path, "warning", f"[[accept]] #{n}: unknown keys {sorted(extra)}")
        name, cmd = a.get("name"), a.get("cmd")
        if not isinstance(name, str) or not name.strip():
            rep.say(path, "error", f"[[accept]] #{n} has no name")
            name = f"#{n}"
        elif not re.match(r"^[A-Za-z0-9._-]+$", name):
            rep.say(path, "error", f"[[accept]] `{name}`: a name is a log file name: letters, digits, `.`, `_`, `-`")
        if name in names:
            rep.say(path, "error", f"[[accept]] name `{name}` used twice")
        names.add(name)
        if not isinstance(cmd, str) or not cmd.strip():
            rep.say(path, "error", f"[[accept]] `{name}` has no cmd")
            continue
        r = check_cmd(name, cmd, path, rep)
        if r is None:
            continue
        for test in r["strict"]:
            strict.append((name, test))
        outlets += r["outlets"]
        named += [(name, x) for x in r["named"]]
        quoted_types += [(name, x) for x in re.findall(r"['\"]([A-Z][A-Z0-9_]{3})['\"]", cmd)]
    check_named_tests(t, path, owns, is_merged, brief, named, rep)
    return strict, outlets, named, quoted_types


def check_cmd(name, cmd, path, rep):
    where = f"[[accept]] `{name}`"
    if re.search(r"\b[A-Za-z]:\\|/mnt/[a-z]/|\bC:/", cmd):
        rep.say(path, "error", f"{where}: names a host path; commands run in /src, a copy of the repo")
    try:
        stmts = parse_shell(cmd)
    except ValueError as e:
        rep.say(path, "error", f"{where}: {e}")
        return None
    if not stmts:
        rep.say(path, "error", f"{where}: empty command")
        return None
    flat = [(si, pi, ci, words, neg) for si, st in enumerate(stmts) for pi, (_, neg, pipe) in enumerate(st)
            for ci, words in enumerate(pipe)]
    pipefail = "pipefail" in cmd
    for _, _, _, words, _ in flat:
        w0 = os.path.basename(words[0])
        if w0 in ("curl", "wget", "ssh", "scp", "pip", "pip3", "npm", "rustup") or (
                w0 == "cargo" and len(words) > 1 and words[1] in ("install", "update", "fetch", "publish")) or (
                w0 == "git" and any(x in words for x in ("fetch", "pull", "push", "clone"))):
            rep.say(path, "error", f"{where}: `{' '.join(words[:3])}` needs the network; acceptance runs offline")
        if any(re.search(r"(?i)starfield\.exe|creationkit\.exe|modorganizer", x) for x in words) or (
                w0 == "kit" and len(words) > 1 and words[1] in ("ui", "game")):
            rep.say(path, "error", f"{where}: acceptance is headless: no window, no game, no Creation Kit")

    # The last statement decides the exit status.
    last = stmts[-1]
    ops = [op for op, _, _ in last]
    _, last_neg, last_pipe = last[-1]
    last_word = os.path.basename(last_pipe[-1][0])
    if ops and ops[-1] == "||" and last_word in NO_FAIL_LAST | {"exit"}:
        rep.say(path, "error", f"{where}: ends in `|| {last_word}`, so it can't fail")
    elif last_word in NO_FAIL_LAST and not (last_word == "tee" and pipefail) and not last_neg:
        rep.say(path, "error", f"{where}: its exit status is `{last_word}`'s, so it can't fail "
                               f"(end with the check, e.g. `; grep -q '^test result: ok. 1 passed' /tmp/o`)")
    if all(neg for st in stmts for _, neg, _ in st[-1:]) and not any(
            parse_grep(w) and not neg for st in stmts for _, neg, pipe in st for w in pipe):
        positive = [w for st in stmts for _, neg, pipe in st for w in pipe if not neg]
        if not positive:
            rep.say(path, "warning", f"{where}: only negative checks (`! grep`): it also passes when the file is gone")

    # A check in a statement before `;` decides nothing.
    for st in stmts[:-1]:
        for _, _, pipe in st:
            for words in pipe:
                if parse_grep(words) or os.path.basename(words[0]) in ("test", "[", "diff", "cmp"):
                    rep.say(path, "error", f"{where}: the check `{' '.join(words[:3])} ...` comes before `;`, "
                                           f"so its result is ignored: join checks with `&&`")
                    break

    # Greps per statement/pipeline: positive ones and the files they read.
    greps = []
    for si, st in enumerate(stmts):
        for pi, (_, neg, pipe) in enumerate(st):
            for ci, words in enumerate(pipe):
                g = parse_grep(words)
                if g:
                    pat, files, inv = g
                    greps.append({"pos": (si, pi, ci), "pat": pat, "files": files, "positive": not (neg or inv),
                                  "piped": ci > 0})

    res = {"strict": [], "outlets": 0, "named": []}
    cargo_positions = []
    for si, st in enumerate(stmts):
        for pi, (_, neg, pipe) in enumerate(st):
            for ci, words in enumerate(pipe):
                c = parse_cargo_test(words)
                if c and not c["list"]:
                    cargo_positions.append(((si, pi, ci), c, pipe[ci + 1:]))
    for k, (pos, c, rest) in enumerate(cargo_positions):
        nxt = cargo_positions[k + 1][0] if k + 1 < len(cargo_positions) else (10 ** 6,)
        sinks = {w[1] for w in rest if os.path.basename(w[0]) == "tee" and len(w) > 1}
        sinks |= {x for w in rest if os.path.basename(w[0]) == "tee" for x in w[1:] if not x.startswith("-")}
        mine = [g for g in greps if pos < g["pos"] < nxt and (
            (g["piped"] and g["pos"][:2] == pos[:2]) or set(g["files"]) & sinks)]
        pos_greps = [g for g in mine if g["positive"]]
        nums = [int(x) for g in pos_greps for x in re.findall(r"(\d+) passed", g["pat"])]
        lower_bound = any(re.search(r"\[1-9\].{0,12}passed", g["pat"]) for g in pos_greps)
        lines_ok = [g for g in pos_greps if re.search(r"(\\\.){3}|\.\.\.|\\\.\\\.\\\.", g["pat"]) and "ok" in g["pat"]]
        printed = [g for g in pos_greps if "test result" not in g["pat"] and g not in lines_ok]
        skip_guard = any(not g["positive"] and re.search(r"(?i)skip", g["pat"]) for g in mine)
        proof = any(n > 0 for n in nums) or lower_bound or lines_ok or (c["nocapture"] and printed)
        allows_ignored = any("ignored" in g["pat"] for g in pos_greps)
        if allows_ignored:
            res["outlets"] += 1
        # A statement before `;` whose output nobody greps: its exit status is dropped.
        if pos[0] < len(stmts) - 1 and not pos_greps:
            rep.say(path, "error", f"{where}: `cargo test` before `;` with no grep of its output: "
                                   f"its exit status is ignored")
        if c["filters"] and not proof:
            known = all(filter_matches(f) for f in c["filters"])
            rep.say(path, "warning" if known else "error",
                    f"{where}: `cargo test` with filter {c['filters']} " + (
                        "checks no count: existing tests match it today, but a filter that matches no test "
                        "exits 0" if known else "can pass vacuously (a filter that matches no test exits 0)")
                    + ": check the count, `2>&1 | tee /tmp/o; grep -q '^test result: ok. 1 passed' /tmp/o`")
        if c["exact"] and c["filters"] and nums and not lower_bound and len(c["filters"]) not in nums:
            rep.say(path, "error", f"{where}: `--exact` names {len(c['filters'])} test(s) but the check expects "
                                   f"{' or '.join(map(str, sorted(set(nums))))} passed")
        if c["filters"] and not c["exact"] and lower_bound and not nums and not GUARD_NAME.search(name):
            hits = sorted({x for f in c["filters"] for x in filter_matches(f)})
            if hits:
                rep.say(path, "warning", f"{where}: filter {c['filters']} without `--exact` also matches "
                                         f"{len(hits)} existing test(s) (e.g. {hits[0]}), so `[1-9]... passed` can be met "
                                         f"without the new tests")
        if c["ignored"] and not printed and not skip_guard and not lines_ok:
            rep.say(path, "warning", f"{where}: `--ignored` (a corpus test?) checks only the test result line; a test "
                                     f"that skips without KITKRAFT_DATA_DIR passes too: print a count and grep it")
        # Tests this command names: exact filters, or filters whose count is pinned to their number.
        exact_names = []
        if c["filters"] and (c["exact"] or (nums and len(c["filters"]) in nums)):
            exact_names = list(c["filters"])
        for g in lines_ok:
            m = re.match(r"^\^?(?:test )?([\w:]+)", g["pat"].replace("\\", ""))
            if m and "(" not in g["pat"]:
                exact_names.append(m.group(1))
        for f in exact_names:
            res["named"].append({"filter": f, "pkg": c["pkg"], "target": c["target"], "exact": c["exact"]})
            if not c["ignored"] and not allows_ignored:
                res["strict"].append(f.split("::")[-1])
    return res


def check_named_tests(t, path, owns, is_merged, brief, named, rep):
    tests = repo_tests()
    paras = [p for p in re.split(r"\n\s*\n", brief or "")]
    seen = set()
    for accept_name, n in named:
        f = n["filter"]
        bare = f.split("::")[-1]
        if (accept_name, bare) in seen or not re.match(r"^\w+$", bare):
            continue
        seen.add((accept_name, bare))
        tail = bare.split("_", 1)[-1]
        if brief and bare not in brief and not (len(tail) > 12 and "_" + tail in brief):
            rep.say(path, "warning", f"[[accept]] `{accept_name}`: the brief never names test `{bare}`; "
                                     f"say the exact name so the agent writes it")
        if bare in tests:
            if is_merged or t.get("base"):
                continue
            if GUARD_NAME.search(accept_name):
                continue
            sents = [s for p in paras for s in re.split(r"(?<=[.!?])\s+", p) if bare in s]
            para = " ".join(p for p in paras if bare in p)
            if any(GUARD_WORDS.search(s) for s in sents):
                continue
            if sents and not NEW_TEST.search(para):
                continue   # the brief mentions it, not as a test to write
            rep.say(path, "error" if sents else "warning",
                    f"[[accept]] `{accept_name}`: test `{bare}` already exists ({tests[bare][0]})" + (
                        ": a new test needs a new name, or the brief says it exists and must keep passing"
                        if sents else ", and the brief never mentions it: a guard? say so in the brief"))
            continue
        # A new test: the agent must be able to write it.
        pkg = packages().get(n["pkg"] or "")
        if not pkg or not owns:
            continue
        kind, tname = n["target"] or ("lib", None)
        mods = f.split("::")[:-1]
        if kind == "test":
            cands = [f"{pkg}/tests/{tname}.rs", f"{pkg}/tests/{tname}/main.rs"]
        else:
            cands = []
            for i in range(len(mods), 0, -1):
                stem = "/".join(mods[:i])
                cands += [f"{pkg}/src/{stem}.rs", f"{pkg}/src/{stem}/mod.rs"]
            if not mods:
                cands += [f"{pkg}/src/lib.rs", f"{pkg}/src/main.rs", f"{pkg}/src/"]
        if any(in_scope(c, owns) for c in cands) or (not mods and kind != "test" and any(
                o.startswith(f"{pkg}/src/") for o in owns)):
            continue
        rep.say(path, "error", f"[[accept]] `{accept_name}`: new test `{bare}` would go in "
                               f"{cands[0]}{' (or its parent module)' if len(cands) > 2 else ''}, which owns doesn't cover")


# ---- the brief ------------------------------------------------------------------------------------

def upstream_owns(t, tickets):
    """Paths the tickets in `after` own (transitively): files they add may be named in the brief."""
    out, todo, seen = [], list(t.get("after") or []), set()
    while todo:
        x = todo.pop()
        if x in seen or x not in tickets:
            continue
        seen.add(x)
        out += [p for p in tickets[x].get("owns") or [] if isinstance(p, str)]
        todo += list(tickets[x].get("after") or [])
    return out


def check_brief_paths(t, path, owns, is_draft, rep, tickets, merged=()):
    brief = t.get("brief") or ""
    owns = list(owns or []) + upstream_owns(t, tickets)
    # A file a queued ticket adds (the brief says which) is not a typo either.
    owns += [p for k, x in tickets.items() if k not in merged for p in x.get("owns") or [] if isinstance(p, str)]
    found, missing = [], []
    for tok in re.findall(r"`([^`\n]+)`", brief):
        tok = tok.strip().rstrip(".,;:)")
        if " " in tok or not tok.startswith(TOP_DIRS) or re.search(r"[<>*{}]|::|#L|\$", tok):
            continue
        tok = tok.split(":")[0]
        if exists(tok):
            found.append(tok)
        elif not in_scope(tok, owns or []) and tok not in missing:
            missing.append(tok)
    for m in missing:
        rep.say(path, "warning", f"brief names `{m}`, which does not exist and is not owned (a typo, or a file "
                                 f"another ticket adds?)")
    if is_draft and not found:
        rep.say(path, "error", "the brief names no existing repo file in backticks: name the starting files")


def check_ignore_rule(t, path, brief, strict, outlets, quoted_types, rep):
    """KK-002: the brief let QUST and WRLD be ignored; the acceptance required them to pass."""
    text = (t.get("title") or "") + ".\n\n" + (brief or "")
    sentences = [s for p in re.split(r"\n\s*\n", text) for s in re.split(r"(?<=[.!?])\s+(?=[A-Z*`(])", p)]
    permits = [s for s in sentences if any(r.search(s) for r in PERMIT)]
    if not permits:
        return
    specific, generic = set(), []
    for s in permits:
        hit = False
        s_full, s = s, " ".join(permit_clauses(s))
        for _, test in strict:
            prefix = test.split("_")[0].upper()
            if test in s or re.search(rf"\b{re.escape(prefix)}\b", s):
                specific.add(test)
                hit = True
        types = set(re.findall(r"\b[A-Z][A-Z0-9_]{3}\b", s))
        for accept_name, ty in quoted_types:
            if ty in types:
                rep.say(path, "error", f"[[accept]] `{accept_name}` requires {ty}, but the brief lets it be "
                                       f"ignored (\"{_short(s)}\")")
                hit = True
        if not hit and not types:
            generic.append(s_full)
    for accept_name, test in strict:
        if test in specific:
            rep.say(path, "error", f"[[accept]] `{accept_name}` requires `{test}` to pass (`1 passed`), but the "
                                   f"brief lets it be ignored: accept `1 ignored` too, or drop the permission (KK-002)")
    rest = [(a, x) for a, x in strict if x not in specific]
    if generic and rest:
        level = "error" if not outlets else "warning"
        rep.say(path, level, f"the brief lets a test be ignored (\"{_short(generic[0])}\"), but "
                             f"{', '.join(sorted({x for _, x in rest}))} must pass with no `1 ignored` outlet "
                             f"(KK-002: acceptance and brief must agree)")


def permit_clauses(s):
    """The clause around each permission in sentence `s`, e.g. "QUST and WRLD may be ignored"."""
    out = []
    for r in PERMIT:
        for m in r.finditer(s):
            start = max(s.rfind(c, 0, m.start()) for c in "(,;:—") + 1
            ends = [i for i in (s.find(c, m.end()) for c in ",;)") if i >= 0]
            out.append(s[start:min(ends) if ends else len(s)])
    return out


def _short(s):
    s = " ".join(s.split())
    return s if len(s) <= 110 else s[:107] + "..."


# ---- one ticket -------------------------------------------------------------------------------------

def lint_file(path, tickets, merged, rep, lint_merged):
    ids = set(tickets)
    try:
        t = tomllib.load(open(path, "rb"))
    except (OSError, tomllib.TOMLDecodeError) as e:
        rep.say(path, "error", f"does not parse: {e}")
        return True
    stem = os.path.basename(path)[:-5]
    is_draft = os.path.basename(os.path.dirname(os.path.abspath(path))) == "drafts"
    tid = t.get("id")
    is_merged = isinstance(tid, str) and tid in merged and not is_draft
    if is_merged and not lint_merged:
        print(f"{os.path.relpath(path, ROOT)}: skipped: {tid} is merged (--all lints it)")
        return False
    extra = set(t) - KNOWN_KEYS
    if extra:
        rep.say(path, "warning", f"unknown keys {sorted(extra)} (typo? known: {', '.join(sorted(KNOWN_KEYS))})")
    if is_draft:
        if tid != "DRAFT":
            rep.say(path, "error", f'a draft has id = "DRAFT" (the ID is assigned at promotion), not {tid!r}')
    elif not isinstance(tid, str) or not ID_RE.match(tid):
        rep.say(path, "error", f"id {tid!r} is not KK-nnn")
    elif tid != stem:
        rep.say(path, "error", f"id {tid} does not match the file name {stem}.toml")
    title = t.get("title")
    if not isinstance(title, str) or not title.strip():
        rep.say(path, "error", "no title")
    elif "\n" in title:
        rep.say(path, "error", "title is one line")
    elif len(title) > 120:
        rep.say(path, "warning", f"title is {len(title)} characters; keep it to one short line")
    brief = t.get("brief")
    if not isinstance(brief, str) or not brief.strip():
        rep.say(path, "error", "no brief: the agent can't read plan/STATUS.md, so the brief carries the context")
        brief = ""
    elif is_draft and len(brief) < 600:
        rep.say(path, "error", f"brief is {len(brief)} characters: a draft's brief carries the context "
                               f"(starting files, rules, what done means)")
    elif len(brief) < 200:
        rep.say(path, "warning", f"brief is {len(brief)} characters; does it carry the context?")
    tm = t.get("timeout_min", 45)
    if not isinstance(tm, int) or isinstance(tm, bool) or not 5 <= tm <= 240:
        rep.say(path, "error", f"timeout_min {tm!r}: minutes per grading command, 5 to 240")
    if "budget_h" in t:
        b = t["budget_h"]
        if not isinstance(b, (int, float)) or isinstance(b, bool) or not 0.25 <= b <= 8:
            rep.say(path, "error", f"budget_h {b!r}: hours, 0.25 to 8")
    elif is_draft:
        rep.say(path, "warning", "no budget_h (default 1.5 h): say how long the run should take")
    if "hold" in t and (not isinstance(t["hold"], str) or not t["hold"].strip()):
        rep.say(path, "error", "hold is the reason it's held: a non-empty string")
    if "gate" in t and (not isinstance(t["gate"], str) or not t["gate"].strip()):
        rep.say(path, "error", "gate must be a command")
    after = t.get("after", [])
    if not isinstance(after, list) or not all(isinstance(x, str) for x in after):
        rep.say(path, "error", "after must be a list of ticket IDs")
        after = []
    for a in after:
        if not ID_RE.match(a):
            rep.say(path, "error", f"after: {a!r} is not a ticket ID")
        elif a == tid:
            rep.say(path, "error", "after names the ticket itself")
        elif a not in ids:
            rep.say(path, "error", f"after: {a} is not in plan/tickets")
    if "base" in t:
        base = t["base"]
        if not isinstance(base, str) or not REF_RE.match(base):
            rep.say(path, "error", f"base {base!r} does not look like a git ref")
        elif base.startswith("kk/") and tid not in base and not is_draft:
            rep.say(path, "warning", f"base {base} is another ticket's WIP branch")
        if is_draft:
            rep.say(path, "warning", "a draft with a base: drafts normally start from main")
    owns = check_owns(t, path, rep)
    r = check_accept(t, path, owns, is_merged, brief, rep)
    if r:
        strict, outlets, _, quoted_types = r
        check_ignore_rule(t, path, brief, strict, outlets, quoted_types, rep)
    check_brief_paths(t, path, owns, is_draft, rep, tickets, merged)
    return True


def cycles(tickets):
    out, state = [], {}

    def visit(n, stack):
        if state.get(n) == 1:
            out.append(stack[stack.index(n):] + [n])
            return
        if state.get(n) == 2:
            return
        state[n] = 1
        for m in tickets.get(n, {}).get("after", []) or []:
            if m in tickets:
                visit(m, stack + [n])
        state[n] = 2

    for n in tickets:
        visit(n, [])
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("paths", nargs="+", metavar="FILE_OR_DIR")
    ap.add_argument("--min", type=int, default=0, help="fail unless at least N tickets were linted")
    ap.add_argument("--all", action="store_true", help="lint merged tickets too")
    a = ap.parse_args(argv)
    rep = Report()
    files = []
    for p in a.paths:
        if os.path.isdir(p):
            files += sorted(os.path.join(p, f) for f in os.listdir(p) if f.endswith(".toml"))
        elif os.path.isfile(p):
            files.append(p)
        else:
            rep.say(os.path.abspath(p), "error", "no such file or folder")
    tickets = all_tickets()
    merged = merged_ids()
    linted = sum(1 for f in files if lint_file(f, tickets, merged, rep, a.all))
    for c in cycles(tickets):
        rep.say(TICKETS, "error", f"after cycle: {' -> '.join(c)}")
    if linted < a.min:
        rep.say(os.path.abspath(a.paths[0]), "error", f"{linted} ticket(s) linted, --min {a.min}")
    print(f"lint: {linted} ticket(s), {rep.errors} error(s), {rep.warnings} warning(s)")
    return 1 if rep.errors else 0


if __name__ == "__main__":
    sys.exit(main())
