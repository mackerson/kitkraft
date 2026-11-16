"""Propose docs/schema/evidence.toml entries from a verification-mod session's report.

    python docs/bench/verify-mod/propose_evidence.py <observe>\\<session>\\report.json [--ledger game-yyyymmdd-nn]

Reads the rows the session graded (`rows[].row`, `rows[].verdict.status`) and prints, for
each record type and status, the `[[entry]]` the rows support, or why there is none. It
writes nothing: a person reads the ledger entry and the rows' `seen` first, then pastes
what holds into evidence.toml (AGENTS.md "Reporting": a row with no observation is not-run,
ambiguous evidence is inconclusive, never pass). The rules:

- A type and status gets `result = "pass"` only when at least one of its rows passed and
  none failed; with a failed row it gets nothing and is listed for review.
- Rows of block G (`V-H-*`, `help` presence) alone make a pass whose note says "presence
  only": the game listed the form, no field effect was seen.
- save-reload-verified needs V-S-01 (the reload proved by the cell) to pass; otherwise
  every V-S row is ignored.

The ledger id comes from the report (`ledger_entry`) unless --ledger is given.
"""

import json
import sys
from collections import defaultdict

GAME = "game-runtime-tested"
SAVE = "save-reload-verified"

# row -> (status, [types], what the row shows)
ROWS = {
    "V-20": (GAME, ["ACTI"], "getisid: the reference's base is the cloned activator"),
    "V-21": (GAME, ["STAT"], "getisid: the reference's base is the cloned static"),
    "V-22": (GAME, ["MSTT"], "getisid: the reference's base is the cloned movable static"),
    "V-23": (GAME, ["FURN"], "getisid: the reference's base is the cloned furniture"),
    "V-24": (GAME, ["FLOR"], "getisid: the reference's base is the cloned flora"),
    "V-25": (GAME, ["DOOR"], "getisid: the reference's base is the cloned door"),
    "V-26": (GAME, ["LIGH"], "getisid: the reference's base is the cloned light"),
    "V-27": (GAME, ["CONT"], "getisid: the reference's base is the cloned container"),
    "V-28": (GAME, ["TERM"], "getisid: the reference's base is the cloned terminal"),
    "V-30": (GAME, ["MISC"], "getisid: the placed reference's base is the cloned item"),
    "V-31": (GAME, ["NPC_", "ACHR"], "getisid: the placed actor's base is the cloned NPC_"),
    "V-P00": (GAME, ["ACTI"], "the script attached by kit ran on OnLoad"),
    "V-P01": (GAME, ["GLOB"], "Papyrus read the global's FLTV 4242"),
    "V-P02": (GAME, ["FLST"], "Papyrus read the list's two LNAM entries"),
    "V-P03": (GAME, ["FLST"], "Papyrus found both entries in the list"),
    "V-P04": (GAME, ["MISC"], "Papyrus read the item's DATA Value 777"),
    "V-P05": (GAME, ["KYWD", "ACTI"], "the activator's added KWDA holds the keyword"),
    "V-P06": (GAME, ["FACT"], "the player was added to the new faction"),
    "V-P07": (GAME, ["PERK"], "the player was given the cloned perk"),
    "V-P08": (GAME, ["AVIF"], "the cloned actor value was set and read on the player"),
    "V-P09": (GAME, ["QUST"], "the new quest started and its stage 10 was set"),
    "V-P10": (GAME, ["LCTN"], "the cell's added XLCN put the reporter in the new location"),
    "V-P11": (GAME, ["CONT"], "the container's CNTO gave three of the item"),
    "V-P12": (GAME, ["LVLI"], "adding the leveled list gave its one entry"),
    "V-P20": (GAME, ["ACTI"], "Papyrus: the reference's base is the cloned activator"),
    "V-P21": (GAME, ["STAT"], "Papyrus: base is the cloned static"),
    "V-P22": (GAME, ["MSTT"], "Papyrus: base is the cloned movable static"),
    "V-P23": (GAME, ["FURN"], "Papyrus: base is the cloned furniture"),
    "V-P24": (GAME, ["FLOR"], "Papyrus: base is the cloned flora"),
    "V-P25": (GAME, ["DOOR"], "Papyrus: base is the cloned door"),
    "V-P26": (GAME, ["LIGH"], "Papyrus: base is the cloned light"),
    "V-P27": (GAME, ["CONT"], "Papyrus: base is the cloned container"),
    "V-P28": (GAME, ["TERM"], "Papyrus: base is the cloned terminal"),
    "V-P29": (GAME, ["PKIN"], "Papyrus: the placed pack-in reference's base is the cloned pack-in"),
    "V-P30": (GAME, ["MISC"], "Papyrus: base is the cloned item"),
    "V-P31": (GAME, ["NPC_", "ACHR"], "Papyrus: the actor's base is the cloned NPC_"),
    "V-I-MISC": (GAME, ["MISC"], "player.additem / getitemcount"),
    "V-I-ALCH": (GAME, ["ALCH"], "player.additem / getitemcount"),
    "V-I-BOOK": (GAME, ["BOOK"], "player.additem / getitemcount"),
    "V-I-KEYM": (GAME, ["KEYM"], "player.additem / getitemcount"),
    "V-I-ARMO": (GAME, ["ARMO"], "player.additem / getitemcount"),
    "V-I-ARMO-EQ": (GAME, ["ARMO"], "player.equipitem / getequipped"),
    "V-I-LVLI": (GAME, ["LVLI"], "player.additem of the list gave its entry"),
    "V-C-GLOB": (GAME, ["GLOB"], "getglobalvalue answered 4242"),
    "V-C-FACT": (GAME, ["FACT"], "player.getinfaction answered 1"),
    "V-C-PERK": (GAME, ["PERK"], "player.hasperk answered 1"),
    "V-C-AVIF": (GAME, ["AVIF"], "player.getav answered 7"),
    "V-C-QUST": (GAME, ["QUST"], "getstage answered 10"),
}
for s, t in [("20", "ACTI"), ("21", "STAT"), ("22", "MSTT"), ("23", "FURN"), ("24", "FLOR"),
             ("25", "DOOR"), ("26", "LIGH"), ("27", "CONT"), ("28", "TERM"), ("30", "MISC"),
             ("31", "NPC_")]:
    ROWS["V-S-" + s] = (SAVE, [t] + (["ACHR"] if t == "NPC_" else []),
                        "after the reload, getisid on the reference still named the cloned base")
for t in ["MISC", "ALCH", "BOOK", "KEYM", "ARMO"]:
    ROWS["V-S-I-" + t] = (SAVE, [t], "after the reload the item added before the save was still held")
for t, what in [("GLOB", "getglobalvalue"), ("FACT", "getinfaction"), ("PERK", "hasperk"),
                ("AVIF", "getav"), ("QUST", "getstage")]:
    ROWS["V-S-" + t] = (SAVE, [t], "after the reload %s still answered what the reporter set" % what)
PRESENCE = ["TMLM", "PKIN", "MESG", "SOUN", "IRES", "TXST", "HAZD", "LVLN", "LVLP", "PACK",
            "RSPJ", "WKMF", "WWED", "ENCH", "COBJ", "REGN", "PCBN", "PCCN", "SFBK", "SFPT",
            "SFTR", "OTFT", "KYWD", "GLOB", "FLST", "FACT", "QUST", "LCTN", "AVIF", "PERK"]
for t in PRESENCE:
    ROWS["V-H-" + t] = (GAME, [t], "help listed the form under its runtime ID (presence only)")


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    report = json.load(open(args[0], encoding="utf-8"))
    ledger = report.get("ledger_entry") or "game-yyyymmdd-nn"
    if "--ledger" in args:
        ledger = args[args.index("--ledger") + 1]
    verdict = {r["row"]: r["verdict"]["status"] for r in report.get("rows", [])}
    reload_ok = verdict.get("V-S-01") == "pass"
    per = defaultdict(lambda: {"pass": [], "other": []})
    for row, (status, types, what) in ROWS.items():
        if status == SAVE and not reload_ok:
            continue
        v = verdict.get(row, "not-run")
        for t in types:
            key = (t, status)
            if v == "pass":
                per[key]["pass"].append((row, what))
            elif v == "fail":
                per[key]["other"].append((row, "fail"))
    review = []
    for (t, status), got in sorted(per.items()):
        if got["other"]:
            review.append("%s %s: failed rows %s; passes %s" % (
                t, status, [r for r, _ in got["other"]], [r for r, _ in got["pass"]]))
            continue
        if not got["pass"]:
            continue
        rows = [r for r, _ in got["pass"]]
        presence = all(r.startswith("V-H-") for r in rows)
        whats = "; ".join(sorted({w for _, w in got["pass"]}))
        note = "bench verification mod (KitKraftVerify.esm), rows %s: %s" % (", ".join(rows), whats)
        if presence:
            note += ". Presence only: no field of the record was seen to take effect"
        print("[[entry]]")
        print('type = "%s"' % t)
        print('status = "%s"' % status)
        print('result = "pass"')
        print('evidence = ["ledger: %s"]' % ledger)
        print("note = %s\n" % json.dumps(note))
    if not reload_ok:
        print("# V-S-01 did not pass (%s): no save-reload-verified entry is proposed" % verdict.get("V-S-01", "not-run"))
    for line in review:
        print("# review, not proposed: " + line)


if __name__ == "__main__":
    main()
