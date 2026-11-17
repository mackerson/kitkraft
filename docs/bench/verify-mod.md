# Bench verification mod (`KitKraftVerify.esm`)

Prepared at the desk on 2026-11-15 for the bench session of 2026-11-16. **Nothing in it
has been run in game.** It is one plugin, made only by `kit`, with one `probes.toml`. It is
built so that a single `kit game session` gives game-column evidence for as many
typed-writable record types as possible. It also gives save/reload evidence for the
types whose state the session can read again after a reload.

| File | What |
| ---- | ---- |
| `docs/bench/verify-mod/verify-mod.toml` | the `kit build` recipe: 41 clones, 6 new forms, the test cell's `XLCN`, 12 placed references |
| `docs/bench/verify-mod/scripts/KitKraft/VerifyReport.psc` | the reporter: on `OnLoad` it checks the plugin's forms and writes one `Debug.Trace` line per check |
| `docs/bench/verify-mod/make_probes.py` | writes the session's `probes.toml` from the `probes.toml` that `kit testplan` writes for the same profile |
| `docs/bench/verify-mod/probes.toml` | its output for tonight's dev profile (slot `0A`); regenerated tomorrow, never edited |
| `docs/bench/verify-mod/propose_evidence.py` | reads the session's `report.json` and prints the `evidence.toml` entries the rows support |
| `crates/game/tests/verify_mod.rs` | the probes parse; the controller passes all 106 rows against a console model; one failed check fails only its own row |

## What was checked tonight (headless)

| Check | Result |
| ----- | ------ |
| `kit papyrus compile` (PapyrusCompiler 4.7.0.5, base-game sources) | `VerifyReport.pex` 5,867 bytes, SHA-256 `964310a8...7b17ac6d0`. The compile also checks that every function the reporter calls is declared |
| `kit build` | `KitKraftVerify.esm` 81,840 bytes, SHA-256 `a9632f11d0ad0ade56170fa424c48d5f5a29bb8475c8977e8870a0289be39faa` (two builds byte-identical); 61 records |
| `kit validate --plugins Starfield.esm` | 0 errors, 1 warning (`cell.actor-without-navmesh`: the apartment has no navmesh, so the placed actor stands still; no row needs it to move) |
| `kit roundtrip --verify` | identical |
| `kit package` | 3 files, validate passed |
| `kit testplan --profile "...\Kitkraft Dev"` | 101 rows, slot `0A`; `make_probes.py` checked all 59 EditorIDs against the object IDs the reporter expects |
| `kit deploy ... --dev --dry-run` | refused only by `live-profile` (MO2's active profile was `Default`); plan: 2 files into `KitKraft Dev - KitKraftVerify` |
| `kit deploy enable ... --dry-run` | refused by `running` (MO2 was open) and `mod-missing` (not deployed tonight, by the lane rules) |
| `kit game session ... --dry-run` | the probes parse; refused only by `selected-profile` (`Default`) and `plugin-not-deployed`; session `2026-11-16-T01`, budget 1,800 s |
| `cargo test -p kitkraft-game --test verify_mod` | 3 passed |

## Methods, cheapest first

- **(a) Console, seen working before:** `coc`, `player.getincell`, `prid` + `getisid`
  (game-20261103-04 T-03, -06 T-03, -08 T-10 and T-15), and the persistence row of
  game-20261103-08 T-22.
- **(a') Console, first sent on the bench of 2026-11-16** (`game-20261116-03` to `-05`).
  The shapes that worked, as `make_probes.py` writes them:

  | Command | Answer | Note |
  | ------- | ------ | ---- |
  | `player.additem <runtime ID> 1`, `player.getitemcount <runtime ID>` | `GetItemCount >> 1.00` | |
  | `player.equipitem <id>`, `player.getequipped <id>` | `GetEquipped >> 1.00` | |
  | `getglobalvalue <runtime ID>` | `GetGlobalValue >> 4242.00` | the EditorID fails: `Item 'KKV01Glob' not found for parameter Global.` |
  | `getstage <runtime ID>` | `GetStage >> 10.00` | the EditorID fails: `Item 'KKV04Qust' not found for parameter Quest.` |
  | `player.getinfaction <runtime ID>` | `GetInFaction >> 1.00` | |
  | `player.hasperk <runtime ID>` | `Perk Rank >> 1` | no decimals; the rank rises each time the reporter adds the perk again |
  | `player.getav KKV40Avif` | `GetActorValue: KKV40Avif >> 7.00` | the AVIF EditorID works; `correlate.rs` maps `getav` to `GetActorValue` since this bench (before, the row was `inconclusive`); `player.getactorvalue` is no console function (`Unknown variable or function`, `game-20261116-05`) |
  | `help <term> 4 <TYPE>` | `TYPE: <EditorID or empty> (<runtime ID>) '<FULL>'`, in its own reply | without `<TYPE>` the answer came about 20 s late or never; an empty list means the term matched nothing |
- **(b) Placed references** in `CityNewAtlantisApartmentTEMPLATE` (`coc` and `getincell`
  observed), checked with `getisid`.
- **(c) A self-reporting script.** A quest cannot carry it today: `kit` writes `QUST` and
  its `DNAM` `StartGameEnabled` flag, but attaching a script to a `QUST` writes a `VMAD`
  with no quest fragment block (`edit/vmad.rs` `attach` starts from `Vmad::new()`, whose
  tail is empty), and the game expects that block on a quest. So the reporter is attached
  to a placed activator instead, where an `OnLoad` Trace was seen in game
  (game-20261103-04 T-05). Its `OnLoad` runs on a fresh load of the cell after leaving it
  (game-20261103-03 reading), so row V-40 leaves to the Lodge and comes back before the
  `papyrus-log` rows.
- **(d) Presence, `help <term> 4 <TYPE>`:** the game lists the form under its runtime ID.
  This was seen late but correct in game-20261103-03 and -08 (`KYWD`, `TMLM`, `TERM`), and
  with the type in its own reply in game-20261116-04 (10 types). It is the
  weakest evidence: the record loaded and kept its name, and no field of it was seen to
  take effect. `help` answers often arrive one request late, so each row ends with
  `prid 00000014`, a command that names nothing, so a late line has a reply to arrive in.
  The correlator routes it by its term. These rows come last.
- **(d') Presence by the reporter** (since 2026-11-16): for the 20 forms `help` listed
  nothing for in game-20261116-04, the reporter writes `KKV-H<nn> <TYPE> present=True` when
  `Game.GetFormFromFile` of the form's object ID in the plugin is not `None`
  (`VerifyReport.psc`, `Present`). As weak as (d): the record loaded, no field was read.

## Type by type

Runtime IDs are tonight's (slot `0A`). Tomorrow `make_probes.py` rewrites them for the
slot `kit testplan` reads from the profile.

| Type | Method | Rows | Expected (line or answer) |
| ---- | ------ | ---- | ------------------------- |
| ACTI | b, c | V-20, V-P00, V-P05, V-P20; V-S-20 | `GetIsID >> 1.00`; `KKV-P00 reporter loaded`; `KKV-P05 KYWD on-reporter=True`; `KKV-P20 ACTI base-is=True` |
| STAT | b, c | V-21, V-P21; V-S-21 | `GetIsID >> 1.00`; `KKV-P21 STAT base-is=True` |
| MSTT | b, c | V-22, V-P22; V-S-22 | as STAT |
| FURN | b, c | V-23, V-P23; V-S-23 | as STAT |
| FLOR | b, c | V-24, V-P24; V-S-24 | as STAT |
| DOOR | b, c | V-25, V-P25; V-S-25 | as STAT |
| LIGH | b, c | V-26, V-P26; V-S-26 | as STAT |
| CONT | b, c | V-27, V-P11, V-P27; V-S-27 | `GetIsID >> 1.00`; `KKV-P11 CONT mugs=3` (its `COCT`/`CNTO` written by kit) |
| TERM | b, c | V-28, V-P28; V-S-28 | as STAT |
| MISC | b, c, a' | V-30, V-P04, V-P30, V-I-MISC; V-S-30, V-S-I-MISC | `KKV-P04 MISC gold=777` (`DATA.Value`); `GetItemCount >> 1` |
| NPC_, ACHR | b, c | V-31, V-P31; V-S-31 | `GetIsID >> 1.00` on the placed `ACHR`; `KKV-P31 NPC_ base-is=True` |
| PKIN | c, d' | V-P29, V-P98 (diagnostic), V-H-PKIN | `KKV-P29 PKIN base-is=True` (passed in game-20261116-03 and -04, answering game-20261103-08 T-20's `0.00`); V-P98 writes the base the game reports, for a person to read |
| GLOB | c, a', d' | V-P01, V-C-GLOB, V-H-GLOB; V-S-GLOB | `KKV-P01 GLOB value=4242`; `getglobalvalue 0A001029` `>> 4242.00` |
| FLST | c, d' | V-P02, V-P03, V-H-FLST | `KKV-P02 FLST size=2`; `KKV-P03 FLST has-mug=True has-wrench=True` |
| KYWD | c, d | V-P05, V-H-KYWD | `KKV-P05 KYWD on-reporter=True` |
| FACT | c, a', d | V-P06, V-C-FACT, V-H-FACT; V-S-FACT | `KKV-P06 FACT player-in=True`; `GetInFaction >> 1` |
| PERK | c, a', d | V-P07, V-C-PERK, V-H-PERK; V-S-PERK | `KKV-P07 PERK player-has=True`; `Perk Rank >> 1` |
| AVIF | c, a', d | V-P08, V-C-AVIF, V-H-AVIF; V-S-AVIF | `KKV-P08 AVIF player-value=7`; `GetActorValue: KKV40Avif >> 7.00` |
| QUST | c, a', d' | V-P09, V-C-QUST, V-H-QUST; V-S-QUST | `KKV-P09 QUST running=True stage10-done=True` (its `INDX` stages); `getstage 0A00102C` `>> 10.00` |
| LCTN | c, d' | V-P10, V-H-LCTN | `KKV-P10 LCTN reporter-in=True` (the cell's added `XLCN`) |
| LVLI | c, a' | V-P12, V-I-LVLI | `KKV-P12 LVLI wrench-added=1`; `player.getitemcount` of the wrench `>> 1` |
| ALCH, BOOK, KEYM | a' | V-I-ALCH, V-I-BOOK, V-I-KEYM; V-S-I-* | `GetItemCount >> 1` |
| ARMO | a' | V-I-ARMO, V-I-ARMO-EQ; V-S-I-ARMO | `GetItemCount >> 1`; `GetEquipped >> 1` |
| TMLM, MESG, HAZD, RSPJ, WWED, ENCH | d | V-H-<type> | the answer contains the form's runtime ID (`0A001008` ... ); all six passed in game-20261116-04 |
| SOUN, IRES, TXST, LVLN, LVLP, PACK, WKMF, COBJ, REGN, PCBN, PCCN, SFBK, SFPT, SFTR, OTFT | d' | V-H-<type> | `KKV-H<nn> <TYPE> present=True`; not run yet (the corrected rows, below) |

Note on the save/reload rows V-S-GLOB, -FACT, -PERK, -AVIF and -QUST: V-S-02's `coc` reloads
the test cell, so the reporter's `OnLoad` runs again after the reload and re-applies what
those rows read (game-20261116-04, Papyrus log). They pass but show no persistence, and no
evidence is entered from them.

Control rows (no type of their own): V-01 (the test cell), V-40 (leave and return), V-P99
(the reporter reached its end), V-S-01 (save in the Lodge, go to the apartment, load,
back in the Lodge: proves the reload), V-S-02 (return after the reload).

**Coverage: 47 of the 58 typed-writable types.** 26 types have a field or behaviour
check (method a, a', b or c). That count includes PKIN, whose check answers an open
question and may fail. 21 types are presence only (method d). The game column has 8
passes tonight (after the transcription below). Every row passing would bring it to 47
types, 21 of them presence only. Save/reload would go from 2 to 21 types: ACHR, ACTI, ALCH,
ARMO, AVIF, BOOK, CONT, DOOR, FACT, FLOR, FURN, GLOB, KEYM, LIGH, MISC, MSTT, NPC_, PERK,
QUST, STAT, TERM (plus CELL and REFR, which already pass).

### Not observable by this method (11)

| Type | Why |
| ---- | --- |
| AMDL, CELL, OMOD, REFR, WEAP | already pass in game; not re-tested (CELL and REFR are exercised by every placed row anyway) |
| ARMA | an armor addon shows only as the worn model: a screenshot and a person, no getter |
| DIAL, INFO | need a conversation (dialogue input is deferred, ADR 0010 Amendment 1 (b)) and a quest with fragments, which kit cannot attach yet (above) |
| PCMT, PNDT, STDT | planet-generation and galaxy roots: no console getter, and a clone is read at game load (a duplicate planet or star may change world generation before the first row, which would cost the whole session) |

## Commands for tomorrow

Use the worktree's release `kit` (built before the window, never while the game runs).
From the worktree root, with `KITKRAFT_DATA_DIR` set and `$SF = C:\Program Files
(x86)\Steam\steamapps\common\Starfield`:

```text
kit papyrus compile docs\bench\verify-mod\scripts\KitKraft\VerifyReport.psc out\verify\Scripts --sources-zip "$SF\Tools\ContentResources.zip"
kit build docs\bench\verify-mod\verify-mod.toml out\verify\KitKraftVerify.esm --overwrite
kit validate --plugin out\verify\KitKraftVerify.esm --plugins Starfield.esm
kit roundtrip out\verify\KitKraftVerify.esm --verify
kit package out\verify\KitKraftVerify.esm out\verify\pkg --scripts out\verify\Scripts\KitKraft\VerifyReport.pex --spec docs\bench\verify-mod\verify-mod.toml --plugins Starfield.esm --overwrite
```

Expect SHA-256 `a9632f11...e39faa` for the plugin. Then, with MO2 and the game closed,
and MO2's selected profile `Kitkraft Dev`:

```text
kit deploy out\verify\pkg\KitKraftVerify C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev" --dry-run
kit deploy out\verify\pkg\KitKraftVerify C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev"
kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftVerify --dev --profile "Kitkraft Dev" --dry-run
kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftVerify --dev --profile "Kitkraft Dev"
kit testplan out\verify\KitKraftVerify.esm out\verify\plan-dev --profile "C:\Modding\MO2-Starfield\profiles\Kitkraft Dev"
python docs\bench\verify-mod\make_probes.py out\verify\plan-dev\probes.toml out\verify\probes.toml
kit game session C:\Modding\MO2-Starfield out\verify\probes.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger <worktree>\docs\game-observations.md --dry-run
```

Read every refusal. Then run the same session command without `--dry-run`, synchronously,
and leave the screen alone. Afterwards run `kit deploy disable ... --mods KitKraftVerify`
(`--dry-run` first). The plugin owns forms, but only the `KitKraftSession-*` saves ever
held them.

Load order. `KitKraftFirstPOI.esm` is enabled in `Kitkraft Dev`, and it carries the same
apartment cell without the `XLCN`. The verification plugin must load **after** it, or
V-P10 (LCTN) fails on the POI's copy of the cell. `kit testplan` assumes it is appended
last (slot `0A` tonight). Check the slot `make_probes.py` prints against the order
`deploy enable` wrote. The mini-site, the scripts recipe and the hostile fixture also edit
this cell; keep them disabled (they are tonight).

Expected duration: about 2 minutes of launch, settle and base load. The 106 rows take 8
to 12 minutes: seven 10 to 15 s waits around `coc`, `save` and `load`, a few seconds a
getter, and up to 6 s for each `help` that answers late. In the worst case every getter
retries twice, which is still under the 1,800 s budget. Allow 25 minutes for the whole
step, including the deploy and enable.

Measured on 2026-11-16 (`game-20261116-04`): about 100 s to the base load, 106 rows in
about 5.5 minutes, 6.5 minutes in all.

## Re-run of the corrected rows (for the bench with the owner, 2026-11-16 evening)

`docs/bench/verify-mod/rerun.toml` holds 25 rows of `probes.toml`, unchanged (the test
`the_rerun_file_is_rows_of_the_full_file` checks it): V-01 and V-40 (the cell, leave and
return, so the reporter runs), V-P00 and V-P99 (it started and finished), V-C-AVIF (the
`getav` correlation), and the 20 presence rows that `help` could not grade in
`game-20261116-04`, now read from the reporter: V-H-PKIN, -OTFT, -SOUN, -IRES, -TXST, -LVLN,
-LVLP, -PACK, -WKMF, -COBJ, -REGN, -PCBN, -PCCN, -SFBK, -SFPT, -SFTR, -GLOB, -FLST, -QUST,
-LCTN (each expects `KKV-H<nn> <TYPE> present=True`). Until a session runs them these rows
are `not-run`. Game time about 3 minutes (100 s to the base load, 35 s for V-01 and V-40,
seconds for the rest).

Before the window (no game, MO2 closed; the plugin is unchanged, `a9632f11...e39faa`; only
`VerifyReport.pex` is new):

1. `kit` must contain the `getav` alias in `crates/game/src/correlate.rs` (branch
   `opus/bench-run-20261116`, or `main` once merged). The release `kit.exe` built from
   `a618e7d6` does not: with it V-C-AVIF stays `inconclusive`, the other 24 rows grade the
   same. Rebuild as the bench sheet says ("Release binaries").
2. From the worktree, with `KITKRAFT_DATA_DIR` set:

   ```text
   kit papyrus compile docs\bench\verify-mod\scripts\KitKraft\VerifyReport.psc out\verify\Scripts --sources-zip "$SF\Tools\ContentResources.zip"
   kit package out\verify\KitKraftVerify.esm out\verify\pkg --scripts out\verify\Scripts\KitKraft\VerifyReport.pex --spec docs\bench\verify-mod\verify-mod.toml --plugins Starfield.esm --overwrite
   kit deploy out\verify\pkg\KitKraftVerify C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev" --dry-run
   kit deploy out\verify\pkg\KitKraftVerify C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev"
   kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftVerify --dev --profile "Kitkraft Dev" --dry-run
   kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftVerify --dev --profile "Kitkraft Dev"
   ```

   The deploy should replace one file (the `.pex`); the mod was left deployed and disabled
   on 2026-11-16, so enable sets its two lines back. `rerun.toml` is for slot `0A` (the
   rename plugin enabled, the verification plugin last). If anything else was enabled
   since, run `kit testplan` as in "Commands for tomorrow" and rewrite it:
   `python docs\bench\verify-mod\make_probes.py out\verify\plan-dev\probes.toml out\verify\rerun.toml --only <the 25 rows above, comma-separated>`.

The re-run, one command (dry run first with `--dry-run` appended):

```text
kit game session C:\Modding\MO2-Starfield docs\bench\verify-mod\rerun.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger <worktree>\docs\game-observations.md
```

Afterwards `kit deploy disable C:\Modding\MO2-Starfield --mods KitKraftVerify --dev
--profile "Kitkraft Dev"` (`--dry-run` first). Enter a passing V-H row as the six presence
rows of `game-20261116-04` were: `game-runtime-tested`, `inconclusive`, note "Presence
only". A V-H row that fails says the form did not load: a real finding, enter it `fail`.

## Entering the results

```text
python docs\bench\verify-mod\propose_evidence.py C:\Modding\kitkraft-observe\<session>\report.json
```

It prints one `[[entry]]` per type and status whose rows passed with none failed, citing
the session's ledger entry, and lists the types to review (a failed row beside a pass).
Read the ledger entry and each row's `seen` first. For a type that already has an entry
for that status (ACTI, KYWD, TERM, CELL, REFR), add the ledger pointer to its `evidence`
instead of a second entry. A presence-only entry keeps the note the script writes. Then
run `cargo test -p kitkraft-engine --lib every_evidence_pointer_resolves` and
`cargo xtask parity`. An inconclusive row proposes nothing. Enter it by hand as
`inconclusive` only when the ledger says why.

## Evidence transcribed tonight (game-20261103-08)

The ledger entry's table shows 14 passes. Its writer's reading re-grades them on
`e6028aaa`: T-16, T-21 and T-23 become inconclusive (ambiguous reply), which leaves 11:
T-01, T-08 to T-11, T-13 to T-15, T-18, T-19 and T-22. Entered in
`docs/schema/evidence.toml`:

- KYWD game pass (T-01: `help` listed `KYWD: KitKraftLocTypeMiniSite (09001000)`; presence
  only).
- ACTI game pass (T-09, T-10: `getisid` on the cloned switch answered 1).
- TERM game pass (T-14, T-15: the same for the cloned terminal).
- PKIN game **inconclusive** (T-19 passed, but T-20's `getisid` on the pack-in answered 0.00:
  the question is open).
- The ledger pointer was added to CELL's game entry (T-08, T-13 and T-18 show the
  `XLCN`-edited override loaded).

Not entered: save/reload rows. T-22 proves the reload, but T-23 (the references after it)
is inconclusive after the re-grade, so no type gains save/reload evidence. The game
column goes from 5 passes to 8, and 1 type is now inconclusive.

## Gaps found (ticket ideas, one line each)

- `kit testplan`: an `LVLI` gets `player.getitemcount <LVLI>`, which can never pass, since a
  leveled list is never held. It should count the list's entries.
- `kit testplan`: it writes no field checks (a global's value, faction membership, a perk,
  an actor value, container contents) and no reporter script; `make_probes.py` and
  `VerifyReport.psc` stand in for them.
- `kit testplan`: rows come in form order (101 for this plugin, with a `coc` and a
  `player.moveto` per reference), not cheapest first or crash-safe last, and no command
  follows a `help` (the session lane's own request from game-20261103-08).
- `kit testplan`: `papyrus-log` rows are not placed after a leave and return of the cell
  (the request from game-20261103-03).
- `kit build` scripts: an object property cannot name a `[[place]]` of the same build
  (`papyrus.property-unresolved: unknown name`). The reporter finds its references with
  `Game.GetFormFromFile` and object IDs that the recipe fixes by order.
- `kit build` scripts: attaching a script to a `QUST` writes a `VMAD` with no quest
  fragment block, so a self-reporting quest cannot be authored.
- `game.session`: no command turns a `report.json` into evidence entries;
  `propose_evidence.py` does it outside the catalog.

## Self-reporting quest (second recipe, added 2026-11-16)

A separate plugin, `KitKraftVerifyQuest.esm`, closes the two `kit build` script gaps above
(commits `faa830ef` and `75e5351b` on `opus/authgaps-20261116`). The recipe and probes of
`KitKraftVerify.esm` are unchanged. **Not run in game.**

The quest gap was half misread. 209 of the 1,703 official quest `VMAD`s hold scripts and
no fragment block, and no official block is empty. So a quest script alone was already
written in the official form. What `kit` could not write was the block's content: a
fragment script and its stage fragments. A `scripts` entry on a `QUST` now takes
`stages = [{ stage = 10 }]`. `crates/engine/tests/quest_vmad_corpus.rs` checks the
format: all 1,494 official blocks roundtrip, and `attach` rebuilds two official quest
`VMAD`s byte for byte. A script property may now also name a `[[place]]` of the same build.

| File | What |
| ---- | ---- |
| `docs/bench/verify-mod/verify-quest.toml` | `KKQ01Qust` (start game enabled, stage 10) with the quest script `KitKraft:VerifyQuestReport` (`Marker = "new:marker"`) and the fragment script `Fragments:Quests:QF_KKQ01Qust` at stage 10; `KKQ70Marker`, a persistent crate in `CityNewAtlantisApartmentAthena01` (not the test cell, so the cell copy it carries does not hide the first plugin's `XLCN`) |
| `docs/bench/verify-mod/scripts/KitKraft/VerifyQuestReport.psc`, `scripts/Fragments/Quests/QF_KKQ01Qust.psc` | on `OnQuestInit`: lines `KKQ-Q01`, `Q02` (the marker property and its base) and `Q04` (stage 10 done after `SetStage`); the fragment writes `KKQ-Q03` |
| `docs/bench/verify-mod/verify-quest-probes.toml` | rows Q-01 to Q-04, `papyrus-log` only (no runtime slot needed); hand-written |

Checked headless (kit built from `75e5351b`):

| Check | Result |
| ----- | ------ |
| `kit papyrus compile` (both scripts, base-game sources) | `VerifyQuestReport.pex` 1,699 bytes, `QF_KKQ01Qust.pex` 646 bytes |
| `kit build docs\bench\verify-mod\verify-quest.toml out\verify-quest\KitKraftVerifyQuest.esm` | 3,238 bytes, 4 records, SHA-256 `94d2ad19feaa10bd8f8ac5da6b80d5f55bb0c2bfe6211cfe666ffb681dea3f6d` (two builds identical) |
| `kit validate --plugin ... --plugins Starfield.esm` | 0 errors, 0 warnings |
| `kit roundtrip ... --verify` | identical |
| `kit game session C:\Modding\MO2-Starfield docs\bench\verify-mod\verify-quest-probes.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2... --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger <worktree>\docs\game-observations.md --dry-run` | probes parse; refused only by `plugin-not-deployed` (not deployed tonight) |

To run it: `kit package` with both `.pex` files, `deploy --dev`, and `deploy enable`, as for
the first plugin. Then run the session command above without `--dry-run`. The two plugins
may be enabled together. The quest starts when the base save loads, so its lines are in
the log before Q-01 reads it.

### Alias script (row Q-05, added 2026-11-17, passed in game)

The same recipe now gives `KKQ01Qust` reference alias 0, `MarkerAlias`, forced to the
placed crate (`ALFR = "new:marker"`, through `fields`), and attaches
`KitKraft:VerifyAliasReport` to it with `alias = "MarkerAlias"`: `kit` writes it into the
alias section of the quest's `VMAD` fragment block (`docs/recipes/scripts.md`, "Scripts on
quest aliases"; `crates/engine/tests/quest_vmad_corpus.rs` rebuilds two official quests'
alias sections byte for byte). On `OnAliasInit` the script writes `KKQ-Q05` with its
`Count` property (7; the script's default is 0) and whether the alias holds the crate.
Row Q-05 of `verify-quest-probes.toml` expects that line. **Passed in game** (`game-20261117-06`, 5 of 5 rows, with stage 10's `QSDT = 0`; plugin `f341e23a...445e62`, 3,413 bytes).

The alias's flags (`FNAM`) cannot be given in `fields`: the signature is also the
objective's `FNAM`, so `build` refuses it (`new.no-group-lead`) and writes the required
member as zero. `VTCK` takes a raw hex FormID (`"00000000"`); TOML has no `null`.

Checked headless (kit built from this branch, 2026-11-17):

| Check | Result |
| ----- | ------ |
| `kit papyrus compile` (the three scripts, base-game sources) | `VerifyAliasReport.pex` 1,368 bytes; the other two as above (1,699 and 646) |
| `kit build docsencherify-moderify-quest.toml outerify-quest\KitKraftVerifyQuest.esm` | 3,390 bytes, 4 records, SHA-256 `e6b58d333db8c33a5d49a0475c687670e72c5f39833c4ee88cff1b2b26e58281` (two builds identical; the probe file's `sha256`) |
| `kit validate --plugin ... --plugins Starfield.esm` | 0 errors, 0 warnings |
| `kit roundtrip ... --verify` | identical |
| `kit inspect KKQ01Qust --plugin ...` | `VMAD` ends with the alias section `0100` (one entry), `0000 0000 00100001` (unused, alias 0, the quest `01001000`), `0600 0200 0100` (version 6, object format 2, one script), then `KitKraft:VerifyAliasReport` with `Count` 7 and `Expected` the crate `01001001`: the layout of `Starfield.esm` `QUST` `000D40ED` |

The table is the alias branch before `QSDT = 0` joined the recipe. The integrated recipe (both
changes) builds 3,413 bytes, SHA-256
`f341e23a2bd926d6d445dcf1ea0a0b9d0a2d81fa731d635ec23149a20b445e62` (two builds identical,
validate 0 errors 0 warnings, roundtrip identical; 2026-11-17), the plugin of
`game-20261117-06` and the probe file's `sha256`.
