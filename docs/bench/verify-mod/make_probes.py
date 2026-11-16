"""Write the bench verification mod's probes.toml (docs/bench/verify-mod.md).

`kit testplan` emits one row per form in form order (101 rows for this plugin), with no
reporter rows, no field checks and an LVLI row that cannot pass. This script writes the
session's rows instead, ordered so a crash late loses little, each row independent of the
others. It takes everything load-order dependent from the `probes.toml` that
`kit testplan` wrote for the same plugin on the same profile: the plugin name and SHA-256,
the runtime-ID basis and the plugin's runtime slot. It also checks that every EditorID
the rows rely on has the object ID the reporter script expects (verify-mod.toml), and
refuses to write otherwise.

    python docs/bench/verify-mod/make_probes.py <testplan probes.toml> <out probes.toml>

Standard library only (Python 3.8+); the testplan file is read with regular expressions
over the lines `kit testplan` writes (plugin, sha256, runtime_id_basis, forms).
"""

import re
import sys

# EditorID -> object ID in KitKraftVerify.esm (verify-mod.toml; the reporter's RefIds).
EXPECTED = {
    "KKV10Acti": 0x1000, "KKV11Stat": 0x1001, "KKV12Mstt": 0x1002, "KKV13Furn": 0x1003,
    "KKV14Flor": 0x1004, "KKV15Door": 0x1005, "KKV16Ligh": 0x1006, "KKV17Cont": 0x1007,
    "KKV18Tmlm": 0x1008, "KKV19Term": 0x1009, "KKV20Pkin": 0x100A, "KKV21Npc": 0x100B,
    "KKV30Misc": 0x100C, "KKV31Misc": 0x100D, "KKV32Alch": 0x100E, "KKV33Book": 0x100F,
    "KKV34Keym": 0x1010, "KKV35Armo": 0x1011, "KKV40Avif": 0x1012, "KKV41Perk": 0x1013,
    "KKV42Kywd": 0x1014, "KKV43Otft": 0x1015, "KKV50Mesg": 0x1016, "KKV51Soun": 0x1017,
    "KKV52Ires": 0x1018, "KKV53Txst": 0x1019, "KKV54Hazd": 0x101A, "KKV55Lvln": 0x101B,
    "KKV56Lvlp": 0x101C, "KKV57Pack": 0x101D, "KKV58Rspj": 0x101E, "KKV59Wkmf": 0x101F,
    "KKV60Wwed": 0x1020, "KKV61Ench": 0x1021, "KKV62Cobj": 0x1022, "KKV63Regn": 0x1023,
    "KKV64Pcbn": 0x1024, "KKV65Pccn": 0x1025, "KKV66Sfbk": 0x1026, "KKV67Sfpt": 0x1027,
    "KKV68Sftr": 0x1028, "KKV01Glob": 0x1029, "KKV02Flst": 0x102A, "KKV03Fact": 0x102B,
    "KKV04Qust": 0x102C, "KKV05Lctn": 0x102D, "KKV06Lvli": 0x102E,
    "KKV70Reporter": 0x102F, "KKV71StatRef": 0x1030, "KKV72MsttRef": 0x1031,
    "KKV73FurnRef": 0x1032, "KKV74FlorRef": 0x1033, "KKV75DoorRef": 0x1034,
    "KKV76LighRef": 0x1035, "KKV77ContRef": 0x1036, "KKV78TermRef": 0x1037,
    "KKV79PkinRef": 0x1038, "KKV80MiscRef": 0x1039, "KKV81NpcRef": 0x103A,
}

CELL = "CityNewAtlantisApartmentTEMPLATE"
CELL_ID = "0021966F"  # 2026-11-01-T03 row T-01, game-20261103-08 T-08
LEAVE = "CityNewAtlantisLodgeInt"
LEAVE_ID = "0006BB29"  # game-20261101-T1, game-20261103-08 T-22
ONE = r"(?m)>>\s*1(\.0+)?\s*$"
AT_LEAST_ONE = r">>\s*[1-9]"
FLUSH = "prid 00000014"  # names nothing: a late `help` line arriving in its reply is routed by term

# (row suffix for the reporter's label, type, reference EditorID, base EditorID)
PLACED = [
    ("20", "ACTI", "KKV70Reporter", "KKV10Acti"),
    ("21", "STAT", "KKV71StatRef", "KKV11Stat"),
    ("22", "MSTT", "KKV72MsttRef", "KKV12Mstt"),
    ("23", "FURN", "KKV73FurnRef", "KKV13Furn"),
    ("24", "FLOR", "KKV74FlorRef", "KKV14Flor"),
    ("25", "DOOR", "KKV75DoorRef", "KKV15Door"),
    ("26", "LIGH", "KKV76LighRef", "KKV16Ligh"),
    ("27", "CONT", "KKV77ContRef", "KKV17Cont"),
    ("28", "TERM", "KKV78TermRef", "KKV19Term"),
    ("30", "MISC", "KKV80MiscRef", "KKV30Misc"),
    ("31", "NPC_", "KKV81NpcRef", "KKV21Npc"),
]

ITEMS = [("MISC", "KKV30Misc"), ("ALCH", "KKV32Alch"), ("BOOK", "KKV33Book"),
         ("KEYM", "KKV34Keym"), ("ARMO", "KKV35Armo")]

# The reporter's lines (VerifyReport.psc), one row each: (label, types, expected line).
REPORT = [
    ("P00", "ACTI VMAD", "KKV-P00 reporter loaded"),
    ("P01", "GLOB", "KKV-P01 GLOB value=4242"),
    ("P02", "FLST", "KKV-P02 FLST size=2"),
    ("P03", "FLST", "KKV-P03 FLST has-mug=True has-wrench=True"),
    ("P04", "MISC", "KKV-P04 MISC gold=777"),
    ("P05", "KYWD", "KKV-P05 KYWD on-reporter=True"),
    ("P06", "FACT", "KKV-P06 FACT player-in=True"),
    ("P07", "PERK", "KKV-P07 PERK player-has=True"),
    ("P08", "AVIF", "KKV-P08 AVIF player-value=7"),
    ("P09", "QUST", "KKV-P09 QUST running=True stage10-done=True"),
    ("P10", "LCTN", "KKV-P10 LCTN reporter-in=True"),
    ("P11", "CONT", "KKV-P11 CONT mugs=3"),
    ("P12", "LVLI", "KKV-P12 LVLI wrench-added=1"),
] + [("P" + s, t, "KKV-P%s %s base-is=True" % (s, t)) for s, t, _, _ in PLACED] + [
    ("P29", "PKIN", "KKV-P29 PKIN base-is=True"),
    ("P98", "PKIN (diagnostic)", "KKV-P98 PKIN reference base="),
    ("P99", "-", "KKV-P99 done"),
]

# Presence only: (type, EditorID, help term). A FULL is searched when the form has one.
PRESENCE = [
    ("TMLM", "KKV18Tmlm", '"KKV18 Terminal Menu"'), ("PKIN", "KKV20Pkin", "KKV20Pkin"),
    ("MESG", "KKV50Mesg", '"KKV50 Message"'), ("SOUN", "KKV51Soun", "KKV51Soun"),
    ("IRES", "KKV52Ires", '"KKV52 Resource"'), ("TXST", "KKV53Txst", "KKV53Txst"),
    ("HAZD", "KKV54Hazd", '"KKV54 Hazard"'), ("LVLN", "KKV55Lvln", "KKV55Lvln"),
    ("LVLP", "KKV56Lvlp", "KKV56Lvlp"), ("PACK", "KKV57Pack", "KKV57Pack"),
    ("RSPJ", "KKV58Rspj", '"KKV58 Research"'), ("WKMF", "KKV59Wkmf", "KKV59Wkmf"),
    ("WWED", "KKV60Wwed", "KKV60Wwed"), ("ENCH", "KKV61Ench", '"KKV61 Enchantment"'),
    ("COBJ", "KKV62Cobj", "KKV62Cobj"), ("REGN", "KKV63Regn", "KKV63Regn"),
    ("PCBN", "KKV64Pcbn", "KKV64Pcbn"), ("PCCN", "KKV65Pccn", "KKV65Pccn"),
    ("SFBK", "KKV66Sfbk", "KKV66Sfbk"), ("SFPT", "KKV67Sfpt", "KKV67Sfpt"),
    ("SFTR", "KKV68Sftr", "KKV68Sftr"), ("OTFT", "KKV43Otft", "KKV43Otft"),
    ("KYWD", "KKV42Kywd", "KKV42Kywd"), ("GLOB", "KKV01Glob", "KKV01Glob"),
    ("FLST", "KKV02Flst", "KKV02Flst"), ("FACT", "KKV03Fact", '"KKV03 Faction"'),
    ("QUST", "KKV04Qust", '"KKV04 Quest"'), ("LCTN", "KKV05Lctn", '"KKV05 Location"'),
    ("AVIF", "KKV40Avif", "KKV40Avif"), ("PERK", "KKV41Perk", '"KKV41 Perk"'),
]


def q(s):
    """A TOML basic string."""
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def lit(s):
    """A TOML literal string (regexes)."""
    assert "'" not in s
    return "'" + s + "'"


class Rows:
    def __init__(self):
        self.out = []

    def row(self, rid, title, forms, steps, screenshot=False):
        o = ["", "[[probe]]", "row = " + q(rid), "title = " + q(title),
             "forms = [" + ", ".join(q(f) for f in forms) + "]"]
        if screenshot:
            o.append("screenshot = true")
        for st in steps:
            o += ["", "[[probe.step]]", "command = " + q(st.get("command", ""))]
            if "matcher" in st:
                o.append("matcher = " + q(st["matcher"]))
                exp = st["expected"]
                o.append("expected = " + (lit(exp) if st["matcher"] == "regex" else q(exp)))
            if "tolerance" in st:
                o.append("tolerance = %s" % st["tolerance"])
            if "why" in st:
                o.append("why = " + q(st["why"]))
            if "wait_secs" in st:
                o.append("wait_secs = %d" % st["wait_secs"])
        self.out += o


def step(command, wait=None):
    s = {"command": command}
    if wait:
        s["wait_secs"] = wait
    return s


def check(command, matcher, expected, why, tolerance=None):
    s = {"command": command, "matcher": matcher, "expected": expected, "why": why}
    if tolerance is not None:
        s["tolerance"] = tolerance
    return s


def plog(expected, why):
    return {"command": "", "matcher": "papyrus-log", "expected": expected, "why": why}


def read_plan(path):
    text = open(path, encoding="utf-8").read()
    def one(key):
        m = re.search(r'(?m)^%s = "([^"]*)"' % key, text)
        if not m:
            sys.exit("%s: no %s line (is it a kit testplan probes.toml?)" % (path, key))
        return m.group(1)
    plugin, sha, basis = one("plugin"), one("sha256"), one("runtime_id_basis")
    found = {}
    for m in re.finditer(r'forms = \[(.*)\]', text):
        parts = re.findall(r'"([^"]*)"', m.group(1))
        lab = next((p for p in parts if p.startswith(plugin + ":")), None)
        if lab is None or len(parts) < 3:
            continue
        obj = int(lab.split(":")[1], 16)
        edid, rid = parts[1], parts[2]
        if re.fullmatch(r"[0-9A-F]{8}", rid):
            found[edid] = (obj, rid)
    return plugin, sha, basis, found


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    plugin, sha, basis, found = read_plan(sys.argv[1])
    if plugin != "KitKraftVerify.esm":
        sys.exit("the plan is for %s, not KitKraftVerify.esm" % plugin)
    slots = {rid[:2] for _, rid in found.values()}
    bad = [e for e, o in EXPECTED.items() if e not in found or found[e][0] != o]
    if bad or len(slots) != 1:
        sys.exit("object IDs or slot differ from verify-mod.toml's: %s %s" % (bad, slots))
    def R(edid):
        return found[edid][1]

    r = Rows()
    # Block A: the test cell and the placed references, by commands seen working.
    r.row("V-01", "coc %s: the test cell" % CELL, [CELL, CELL_ID], [
        step("coc " + CELL, 10),
        check("player.getincell " + CELL_ID, "regex", ONE,
              "the player is in the test cell (game-20261103-08 T-08)")])
    for s, t, ref, base in PLACED:
        r.row("V-%s" % s, "prid %s; getisid %s: the %s %s is the base of %s" % (R(ref), R(base), t, base, ref),
              [plugin + ":%06X" % EXPECTED[ref], ref, R(ref), base, R(base)], [
                  step("prid " + R(ref)),
                  check("getisid " + R(base), "regex", ONE,
                        "the reference's base is the %s the plugin wrote (getisid observed: game-20261103-04 T-03, -06 T-03, -08 T-10, T-15)" % t)])
    # Block B: the reporter. Its OnLoad runs on a fresh load of the cell after leaving it
    # (game-20261103-03 reading, T-12), so leave, return and wait for the log.
    r.row("V-40", "leave to %s and return: the reporter's OnLoad runs" % LEAVE, [LEAVE, CELL, CELL_ID], [
        step("coc " + LEAVE, 10), step("coc " + CELL, 15),
        check("player.getincell " + CELL_ID, "regex", ONE, "back in the test cell, freshly loaded")])
    for label, types, line in REPORT:
        why = "VerifyReport.psc writes this line on OnLoad only when the check holds; the log written so far in the session is read"
        if label == "P98":
            why = "diagnostic: the line is written whatever the base is; a person reads which form follows `base=` (KK-250)"
        r.row("V-%s" % label, "Papyrus log: %s (%s)" % (line, types), ["KKV70Reporter", R("KKV70Reporter")],
              [plog(line, why)])
    # Block C: inventory (player.additem / getitemcount: grade D).
    for t, e in ITEMS:
        r.row("V-I-%s" % t, "player.additem %s 1: the %s %s in the inventory" % (R(e), t, e),
              [plugin + ":%06X" % EXPECTED[e], e, R(e)], [
                  step("player.additem %s 1" % R(e)),
                  check("player.getitemcount " + R(e), "regex", AT_LEAST_ONE, "the item was added (getitemcount grade D)")])
    r.row("V-I-ARMO-EQ", "player.equipitem %s: the ARMO is worn" % R("KKV35Armo"), ["KKV35Armo", R("KKV35Armo")], [
        step("player.equipitem " + R("KKV35Armo")),
        check("player.getequipped " + R("KKV35Armo"), "regex", ONE, "the armor is equipped (getequipped grade D)")])
    r.row("V-I-LVLI", "player.additem %s 1: the LVLI KKV06Lvli gives its one entry, the wrench %s" % (R("KKV06Lvli"), R("KKV31Misc")),
          ["KKV06Lvli", R("KKV06Lvli"), "KKV31Misc", R("KKV31Misc")], [
              step("player.additem %s 1" % R("KKV06Lvli")),
              check("player.getitemcount " + R("KKV31Misc"), "regex", AT_LEAST_ONE,
                    "a leveled list is resolved when added: the count of its entry, never of the list (additem of an LVLI grade D)")])
    # Block D: field checks by console commands no session has sent yet (unverified).
    r.row("V-C-GLOB", "getglobalvalue KKV01Glob: 4242 (unverified command)", ["KKV01Glob", R("KKV01Glob")], [
        check("getglobalvalue KKV01Glob", "number", "4242", "the plugin's FLTV (command unverified in Starfield)", 0.5)])
    r.row("V-C-FACT", "player.getinfaction %s: 1 after the reporter (unverified command)" % R("KKV03Fact"), ["KKV03Fact", R("KKV03Fact")], [
        check("player.getinfaction " + R("KKV03Fact"), "regex", ONE, "the reporter added the player (GetInFaction lines seen in coc replies; the command itself unverified)")])
    r.row("V-C-PERK", "player.hasperk %s: 1 after the reporter (unverified command)" % R("KKV41Perk"), ["KKV41Perk", R("KKV41Perk")], [
        check("player.hasperk " + R("KKV41Perk"), "regex", ONE, "the reporter added the perk (command unverified)")])
    r.row("V-C-AVIF", "player.getav KKV40Avif: 7 after the reporter (unverified command)", ["KKV40Avif", R("KKV40Avif")], [
        check("player.getav KKV40Avif", "number", "7", "the reporter set the value (command and EditorID form unverified)", 0.5)])
    r.row("V-C-QUST", "getstage KKV04Qust: 10 after the reporter (getstage grade D)", ["KKV04Qust", R("KKV04Qust")], [
        check("getstage KKV04Qust", "number", "10", "the reporter set stage 10 (getstage grade D)", 0.5)])
    # Block E: persistence, as kit testplan writes it (proven in game-20261103-08 T-22).
    r.row("V-S-01", "persistence 1-3: leave, save to the session's own save, go elsewhere, load it",
          [LEAVE, LEAVE_ID, CELL, CELL_ID], [
              step("coc " + LEAVE, 10),
              check("player.getincell " + LEAVE_ID, "regex", ONE, "the player is in the leave cell when the save is made"),
              step("save {save}", 10),
              step("coc " + CELL, 10),
              check("player.getincell " + CELL_ID, "regex", ONE, "the player left the leave cell after the save"),
              step("load {save}", 10),
              check("player.getincell " + LEAVE_ID, "regex", ONE, "the load took the player back to the leave cell: the reload happened")])
    # Block F: after the reload.
    r.row("V-S-02", "return to the test cell after the reload", [CELL, CELL_ID], [
        step("coc " + CELL, 10),
        check("player.getincell " + CELL_ID, "regex", ONE, "back in the test cell after the reload")])
    for s, t, ref, base in PLACED:
        r.row("V-S-%s" % s, "after the reload: prid %s; getisid %s (%s)" % (R(ref), R(base), t),
              [ref, R(ref), base, R(base)], [
                  step("prid " + R(ref)),
                  check("getisid " + R(base), "regex", ONE, "the reference and its %s base came back from the save" % t)])
    for t, e in ITEMS + [("LVLI", "KKV31Misc")]:
        r.row("V-S-I-%s" % t, "after the reload: player.getitemcount %s (%s)" % (R(e), t), [e, R(e)], [
            check("player.getitemcount " + R(e), "regex", AT_LEAST_ONE, "the item added before the save is still held")])
    r.row("V-S-GLOB", "after the reload: getglobalvalue KKV01Glob (unverified command)", ["KKV01Glob"], [
        check("getglobalvalue KKV01Glob", "number", "4242", "the global after the reload", 0.5)])
    r.row("V-S-FACT", "after the reload: player.getinfaction %s (unverified command)" % R("KKV03Fact"), ["KKV03Fact"], [
        check("player.getinfaction " + R("KKV03Fact"), "regex", ONE, "the faction membership was saved")])
    r.row("V-S-PERK", "after the reload: player.hasperk %s (unverified command)" % R("KKV41Perk"), ["KKV41Perk"], [
        check("player.hasperk " + R("KKV41Perk"), "regex", ONE, "the perk was saved")])
    r.row("V-S-AVIF", "after the reload: player.getav KKV40Avif (unverified command)", ["KKV40Avif"], [
        check("player.getav KKV40Avif", "number", "7", "the actor value was saved", 0.5)])
    r.row("V-S-QUST", "after the reload: getstage KKV04Qust (grade D)", ["KKV04Qust"], [
        check("getstage KKV04Qust", "number", "10", "the quest stage was saved", 0.5)])
    # Block G: presence by `help` (answers come late, game-20261103-03; each row ends with
    # a command that names nothing, so a late line has a reply to arrive in).
    for t, e, term in PRESENCE:
        r.row("V-H-%s" % t, "help %s 4: the %s %s is loaded (presence only)" % (term, t, e),
              [plugin + ":%06X" % EXPECTED[e], e, R(e)], [
                  check("help %s 4" % term, "contains", R(e),
                        "the game lists the form under its runtime ID (presence only: no field effect is shown)"),
                  step(FLUSH)])

    head = [
        "# Written by docs/bench/verify-mod/make_probes.py from a kit testplan probes.toml;",
        "# regenerate, never edit (docs/bench/verify-mod.md).",
        "format = 1",
        "plugin = " + q(plugin),
        "sha256 = " + q(sha),
        "generated_by = " + q("docs/bench/verify-mod/make_probes.py (bench 2026-11-16) over kit testplan"),
        'profile = "KitKraft Dev"',
        "runtime_id_basis = " + q(basis),
        "notes = [",
        "    " + q("Rows are independent: each names its forms by runtime ID and sets up what it reads, except the V-P rows, which read the lines the reporter wrote after V-40, and the V-C and V-S rows, which read what the reporter changed.") + ",",
        "    " + q("Order: proven console commands first, the reporter, inventory, unverified commands, the save and reload, the checks after it, presence by help last.") + ",",
        "]",
    ]
    open(sys.argv[2], "w", encoding="utf-8", newline="\n").write("\n".join(head + r.out) + "\n")
    rows = sum(1 for x in r.out if x == "[[probe]]")
    print("%s: %d rows for %s (slot %s)" % (sys.argv[2], rows, plugin, slots.pop()))


if __name__ == "__main__":
    main()
