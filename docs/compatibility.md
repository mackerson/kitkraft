# Compatibility: installed versions, masters and DLC (M5.F3)

This document records which game, Creation Kit and tool versions KitKraft is checked
against, and every official master in the game's `Data` folder. It also checks the owner's
mapping of products to masters (ADR 0011 §3), using the files themselves.

Recorded on 2026-10-16 on the dev box, at commit `5d03d01`, with the release binary
`C:\Modding\kitcraft\bin\kit.exe` (SHA-256 `4fa997f8...0672d`, `kit 0.0.0`) and
`KITKRAFT_DATA_DIR="C:/Program Files (x86)/Steam/steamapps/common/Starfield/Data"`.
Every command used is read-only, and nothing was written under the game folder, Steam or
MO2. Update this file in its own commit when the game or the CK updates. The corpus
manifest `corpus/manifest.toml` must change in the same commit when a master's hash does.

## Installed versions

| Component | Version | Source of the value | File SHA-256 |
| --------- | ------- | ------------------- | ------------ |
| Starfield (`Starfield.exe`) | 1.16.244.0 | `kit doctor` ("Game version ... file version of ... Starfield.exe"); Steam app 1716740, `buildid` 23518663 (`steamapps\appmanifest_1716740.acf`) | `7e9adb14...0f9d28` |
| Creation Kit (`CreationKit.exe`) | 1.16.244.0 | file version (Windows file properties; `kit doctor` does not report the CK); Steam app 2722710 "Starfield: Creation Kit", `buildid` 23516818 (`appmanifest_2722710.acf`) | `ed54e490...a14020` |
| Papyrus compiler (`Tools\Papyrus Compiler\PapyrusCompiler.exe`) | 4.7.0.5 | `kit doctor` (found in the game folder); file version; `docs/source-registry.md` PAPYRUS-COMPILER | `a1b7faee...0a97` |
| Archive2 (`Tools\Archive2\Archive2.exe`) | 1.2.0.1 | file version; black-box facts in `docs/gaps.md` | - |
| SFSE | `sfse_1_16_244.dll` 0.2.21 (file version `0, 0, 2, 21`), `sfse_loader.exe` 1.1.1 | `kit doctor` ("SFSE ... (for game 1.16.244)"); file versions; ADR 0008 §4 | dll `28c5bda4...f82b`, loader `16916c2e...7526` |
| xEdit (`C:\Modding\tools\xEdit\xSFEdit64.exe`, `xDump64.exe`) | 4.1.5.0 (release 4.1.5f, 2024) | `kit doctor`; file version; `docs/source-registry.md` SF1EDIT. This build is older than the pinned source SHA `9fb01688...` (`docs/xedit-reference.md`, `docs/gaps.md`) | - |
| KitKraft | `kit 0.0.0` | `kit --version`; `kit doctor` line "KitKraft (kit)" | `4fa997f8...0672d` |
| Steam language | english | `appmanifest_1716740.acf` `UserConfig.language` | - |

No game update happened between the owner's in-game sessions of 2026-10-15 and this
record. The saves of those sessions name game 1.16.244.0 in their headers
(`docs/game-observations.md`, game-20261015-02), and `Starfield.exe` is dated 2026-08-25.

## Official masters

All 14 `.esm` files in the `Data` folder. Size and SHA-256 come from `sha256sum` on
2026-10-16, and every one equals `corpus/manifest.toml` (recorded 2026-10-14). The flags,
scale, form version, record count and masters come from `kit --json load --plugins
<all 14>`. The `HEDR` version, record count and next object ID come from `kit --json
inspect 000000 --plugin <file>`, decoded from `HEDR`'s 12 bytes (f32, u32, u32). The masters
are listed as each file stores them in `MAST`, case included.

| File | Size (bytes) | SHA-256 | Flags (scale) | `HEDR` version / records / next ID | Form version | Records (`load`) | Masters (`MAST`) |
| ---- | -----------: | ------- | ------------- | ---------------------------------- | -----------: | ---------------: | ---------------- |
| `Starfield.esm` | 1,457,098,709 | `1dabed00c3f4282dd3bb54d2e9601e40b577d8742d078b7ccef203adbfef0da7` | `0x81` (full) | 0.96 / 3,829,247 / `0xFF001000` | 581 | 3,829,246 | none |
| `Constellation.esm` | 39,210 | `a314299ba8394a682b5bfca5ac29d5ee33fea9ca4e1ff70931e9d6022c81b0bc` | `0x181` (small) | 0.96 / 57 / `0xFF001000` | 552 | 56 | `Starfield.esm` |
| `OldMars.esm` | 24,358 | `96f74cb008ee4b699b5430068906132dc63777453123ef227b93a629db924bc9` | `0x181` (small) | 0.96 / 39 / `0xFF001000` | 552 | 38 | `Starfield.esm` |
| `BlueprintShips-Starfield.esm` | 303,656,303 | `4341d569fb5b840895d59880972c8f4622790a5229373860d0de0df352252371` | `0x881` (full, blueprint) | 0.96 / 1,505,736 / `0xFF0CB016` | 581 | 1,503,332 | `Starfield.esm` |
| `SFBGS007.esm` | 27,312 | `40687f89ac820a750436c71f5a023fdaff552c3f1d314601dbb0177d341a2323` | `0x181` (small) | 0.96 / 63 / `0xFF001000` | 581 | 62 | `Starfield.esm` |
| `SFBGS008.esm` | 11,302,160 | `e4426499386fcfcf74ed3047be298a426cb2faae2b710904093a028a04b8b462` | `0x181` (small) | 0.96 / 182 / `0xFF001000` | 576 | 181 | `starfield.esm` |
| `SFBGS006.esm` | 7,315,458 | `964a00bcfa2e9d1d183251ec883e6d7888c2ae49579f2b98395126eafb325c45` | `0x481` (medium) | 0.96 / 60,922 / `0xFF001000` | 581 | 60,921 | `Starfield.esm` |
| `SFBGS003.esm` | 33,279,036 | `e3bab5d184bc0d948e01a23d9166e0cb626d04878f01ae87d75d6f5421f2cf63` | `0x481` (medium) | 0.96 / 65,476 / `0xFF001000` | 582 | 65,475 | `starfield.esm` |
| `SFBGS004.esm` | 254,945 | `2637ff51120c013e0742132f6e6b2c795a48c02a23feffea0e5eec531808c41d` | `0x181` (small) | 0.96 / 892 / `0xFF001000` | 581 | 891 | `starfield.esm` |
| `SFBGS00D.esm` | 102,054,268 | `35271d3221310094c70ac2f418d7d393db59dd94d303e830536f85034f44da95` | `0x81` (full) | 0.96 / 434,991 / `0xFF001000` | 582 | 434,990 | `starfield.esm` |
| `SFBGS047.esm` | 472,022 | `a0d739b07988d49bfefd2a8a6c0d054de6a73781dc86c70ab917da93c9dcc215` | `0x181` (small) | 0.96 / 43 / `0xFF001000` | 582 | 42 | `starfield.esm`, `sfbgs004.esm`, `sfbgs00d.esm` |
| `ShatteredSpace.esm` | 501,394,621 | `f8b9333000aba2a4a8a417cb74a11ede6f5b824d27db1918fd1c1f7b64385a43` | `0x81` (full) | 0.96 / 1,029,529 / `0xFF001000` | 582 | 1,029,528 | `starfield.esm` |
| `SFBGS050.esm` | 106,017,249 | `0bda63ae7e4fa48e1149a62e557c56ce65ce8c97badfc6fad1a5a0faf5abeb9d` | `0x81` (full) | 0.96 / 485,320 / `0xFF001000` | 582 | 485,319 | `starfield.esm`, `sfbgs00d.esm` |
| `BlueprintShips-SFBGS050.esm` | 16,136,720 | `8848942d4b8a5143e434e17ea37828c920512af8c3a9ffd87c109d537b704ac6` | `0x881` (full, blueprint) | 0.96 / 80,290 / `0xFF000802` | 582 | 80,007 | `Starfield.esm`, `sfbgs00d.esm`, `SFBGS050.esm` |

Notes on the table:

- Every file has the localized flag (`0x80`) and the master flag (`0x1`). `kit load` names
  `0x100` small, `0x400` medium and `0x800` blueprint (`docs/load-order.md`).
- `HEDR` record count: 12 files store `load`'s count plus one (`TES4` counted). The two
  `BlueprintShips` files store another meaning (`docs/gaps.md`, "`HEDR` record count has
  two meanings").
- Masters are stored in lower case in 8 files. `kit load` resolves them by name, ignoring
  case, and reports no missing or late master for this load order.
- Only the two `BlueprintShips` files have `CNAM` "DEFAULT" and no `SNAM`. The other 12
  have `CNAM` "Python" and `SNAM` "Script" (`kit inspect 000000 --plugin <file>`).
- Archives and string tables beside each master (files named after it in `Data`):
  `BlueprintShips-SFBGS050.esm` has none. It is localized but ships no string tables
  (`docs/gaps.md`, lstring row).

## Products and the owner's mapping (ADR 0011 §3)

The owner's mapping is Free Lanes = `SFBGS00D.esm`; Terran Armada = `SFBGS050.esm` plus
`BlueprintShips-SFBGS050.esm`; Moon Jumper = `SFBGS047.esm`. Three kinds of evidence
were checked, all read only:

1. **Steam's install manifest** `steamapps\appmanifest_1716740.acf` lists the installed
   depots: base depots 1716741 and 1716742, and DLC depots 2401180, 2401181, 2721670 and
   3763700, each with its size in bytes. The DLC depots carry `dlcappid` equal to their
   depot id.
2. **Steam's app cache** `appcache\appinfo.vdf` names each app id. The app's `name` string
   sits at the same position after its id in every record. The method was checked on the
   two apps whose names the `.acf` files also give: 1716740 "Starfield" and 2722710
   "Starfield: Creation Kit". It gives 2401180 "STARFIELD DIGITAL PREMIUM EDITION
   UPGRADE", 2401181 "Starfield - Preorder", 2721670 "Starfield - Shattered Space" and
   3763700 "Starfield - Terran Armada".
3. **File sizes.** Each DLC depot's size was compared with the files in `Data` named after
   a master (the `.esm` and its `.ba2` archives).

| Product (Steam name) | Depot / app | Depot size (bytes) | Files | Their total (bytes) | Result |
| -------------------- | ----------- | -----------------: | ----- | ------------------: | ------ |
| Starfield - Terran Armada | 3763700 | 4,398,842,722 | `SFBGS050.esm`, `SFBGS050 - Main.ba2`, `- Textures.ba2`, `- Voices_en.ba2`, `BlueprintShips-SFBGS050.esm` | 4,398,842,722 | **exact**: these five files are this DLC's depot |
| Starfield - Shattered Space | 2721670 | 10,710,872,042 | `ShatteredSpace.esm` and its 4 archives | 10,710,872,042 | **exact** |
| STARFIELD DIGITAL PREMIUM EDITION UPGRADE | 2401180 | 50,772,412 | `Constellation.esm` and its 2 archives | 50,661,413 | close: 110,999 bytes of the depot unaccounted for |
| Starfield - Preorder | 2401181 | 14,760,227 | `OldMars.esm` and its 2 archives | 14,648,715 | close: 111,512 bytes unaccounted for |
| (base game) | 1716741, 1716742 | 270,321,264; 137,661,049,605 | everything else, `SFBGS00D*` and `SFBGS047*` among it | - | `SFBGS00D` (3,322,090,799 bytes with its 7 archives) is too large for 2401180 or 2401181. 2721670 and 3763700 are taken exactly by other files, so `SFBGS00D` and `SFBGS047` ship in the base depots, not in any DLC depot |

Verdict per product:

| Owner's mapping | Verdict | Evidence |
| --------------- | ------- | -------- |
| Terran Armada = `SFBGS050.esm` + `BlueprintShips-SFBGS050.esm` | **confirmed** | DLC app 3763700 is named "Starfield - Terran Armada", and its depot's size equals these two masters plus `SFBGS050`'s three archives, to the byte. The content agrees: quest `SFBGS050.esm:0059A0` `SFTER_MQIntro` is named "Attack of the Terran Armada", and 106 of the 128 quests that `SFBGS050.esm` adds have EditorIDs that start with `SFTER` (`kit query QUST "@plugin == 'SFBGS050.esm'" --scope all`) |
| Free Lanes = `SFBGS00D.esm` | **confirmed by content; no store product** | It ships in the base game depots, not in a DLC, which fits a free update. Its own records name it: quest `SFBGS00D.esm:0953A6` `Patch_Update10_SFFL` "[Update 10 - Free Lanes]", messages `SFBGS00D.esm:075B85` "Free Lanes - Cruise Mode" and `:086B9C` "Gameplay Options - Free Lanes", and 103 of its 116 added quests start with `SFFL` (`kit query QUST/MESG ... "@edid ~ 'FreeLane' or FULL ~ 'Free Lanes'"`). The update's public name is an **owner statement, unverified** in local files beyond these strings |
| Moon Jumper = `SFBGS047.esm` | **consistent with content; product unverified** | It ships in the base depots. Its 42 records are a vehicle: WEAP `SFBGS047.esm:000002` `MoonJumper_Turret`, FURN `:000001` `MoonJumper` "Moon Jumper", PERK `:000022` "Moon Jumper Perk", COBJ `:000017` `SFBGS047_co_Vehicle_MoonJumper` (`kit query <type> "@plugin == 'SFBGS047.esm'" --scope all`, load order Starfield, SFBGS004, SFBGS00D, SFBGS047). Which store product or update delivered it: **owner statement, unverified** |

### Dependencies between the DLC masters (from `MAST`)

- `SFBGS050.esm` (Terran Armada) masters `SFBGS00D.esm` (Free Lanes): Terran Armada does
  not load without Free Lanes' master. `BlueprintShips-SFBGS050.esm` masters
  `Starfield.esm`, `SFBGS00D.esm` and `SFBGS050.esm`.
- `SFBGS047.esm` (Moon Jumper) masters `SFBGS004.esm` and `SFBGS00D.esm`. One of its
  records names the other DLC: `SFBGS047.esm:000023` `s_40_MoonJumper_MountedLaser_SFBGS050`
  (an EditorID only; it is not a master link).
- `ShatteredSpace.esm`, `SFBGS003`, `004`, `006`, `007` and `008` master only
  `Starfield.esm`.
- `SFBGS050.esm` also adds records whose EditorIDs carry other content prefixes
  (`SFBGS033`, `SFBGS034`, `SFBGS037`, `SFBGS022`; see
  `docs/research/free-lanes-outposts-2026-10-16.md`). What those prefixes stand for is not
  in the files.

### Consequences for fixtures (ADR 0011 §3)

- A fixture that masters only `Starfield.esm` is valid with or without any DLC. This is
  the default.
- A Free Lanes fixture (`SFBGS00D.esm`) needs no paid DLC: the master ships with the base
  game.
- A Terran Armada fixture needs the paid DLC (app 3763700) and, through `MAST`,
  `SFBGS00D.esm` as well. It stays optional and is never a dependency of `cargo xtask ci`.
- Runtime slots of these masters on the owner's dev profile are confirmed by its saves:
  `ShatteredSpace` `01`, `SFBGS00D` `02`, `SFBGS050` `03`, blueprint plugins last among
  the full slots, and `SFBGS047` `FE:005` (`kit load --profile "Kitkraft Dev" --from-save
  <Exitsave0_..._000052>`; `docs/load-order.md`).

## Outpost and workshop changes of Free Lanes and Terran Armada (M5.F5)

The count and the examples of what `SFBGS00D.esm` and `SFBGS050.esm` add to or override in
`Starfield.esm`'s outpost and workshop families are in
`docs/research/free-lanes-outposts-2026-10-16.md`. Each row there names its command. That
note chooses FTH-EXP-09's rows.

## How to refresh

```powershell
$env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
kit doctor
kit --json load --plugins Starfield.esm --plugins Constellation.esm ...   # all 14, repeat --plugins
kit --json inspect 000000 --plugin SFBGS050.esm                           # TES4 HEDR, CNAM, SNAM, MAST
Get-FileHash -Algorithm SHA256 "$env:KITKRAFT_DATA_DIR\*.esm"             # compare corpus/manifest.toml
(Get-Item '...\Starfield\CreationKit.exe').VersionInfo.FileVersion
```

The depot sizes are in `steamapps\appmanifest_1716740.acf` (`InstalledDepots`). Compare
them with the summed sizes of each master's files in `Data`.
