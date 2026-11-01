# Game observations (sample)

A trimmed copy of `docs/game-observations.md` for the workbench's harness tests
(`crates/ui-egui/src/app/harness/tests.rs`): the session prefill reads the newest passing
session that names the plugin. Rows shortened; the hashes are the ledger's.

## Template: session O2 (M4.O2, first POI and the rename package)

| Field | Content |
| ----- | ------- |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/template/probes.toml` SHA-256 `00` |
| Save | base `Template` SHA-256 `1111111111111111111111111111111111111111111111111111111111111111` |
| Status | pass |

## Ledger

### game-20261101-01: agent session 2026-11-01-T01 (`kit game session`), KitKraftFirstPOI.esm

| Field | Content |
| ----- | ------- |
| Status | fail (3 row(s) failed) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261101-01` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/2026-11-01-poi-plan/probes.toml` SHA-256 `e739f90babeecf0ccaedf89d955d50ce02ef9afb016c76e5c05982ac5482458e` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisApartmentTEMPLATE` | 0021966F | regex | GetInCell >> 1.00 | **pass** | - |
| T-02 | `prid KitKraftFirstPOICrate` | KitKraftFirstPOICrate | number | Item not found | **fail** | - |

### game-20261101-03: agent session 2026-11-01-T03 (`kit game session`), KitKraftFirstPOI.esm

| Field | Content |
| ----- | ------- |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261101-03` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/2026-11-01-poi-plan2/probes.toml` SHA-256 `bb3f88e7c5f041ebf0e57818760f1d7de843fccbfca8e22d1482ab6ded3d3170` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisApartmentTEMPLATE` | 0021966F | regex | GetInCell >> 1.00 | **pass** | - |
| T-04 | `prid 06001000`, `getpos x` | 06001000 | number | GetPos: X >> -17.00 | **pass** | T-04-1.png |

### game-20261101-04: agent session 2026-11-01-T04 (`kit game session`), KitKraftFirstPOI.esm

| Field | Content |
| ----- | ------- |
| Status | inconclusive (1 profile file(s) changed during the session and were restored or reported) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`; session save `KitKraftSession-20261101-04` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/2026-11-01-poi-plan3/probes.toml` SHA-256 `6152085153e2c0d59bf84a2298758fbf3fa7e1bdfad2216fb3cd962cf13d35d8` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisApartmentTEMPLATE` | 0021966F | regex | GetInCell >> 1.00 | **pass** | - |
| T-05 | `save`, `load`, `player.getincell 0006BB29` | 0006BB29 | regex | GetInCell >> 1.00 | **pass** | - |

### game-20261101-05: a later session of another plugin

| Field | Content |
| ----- | ------- |
| Status | pass |
| Save | base `OtherBase` SHA-256 `2222222222222222222222222222222222222222222222222222222222222222` |
| Plugins | `KitKraftOverhaul.esp` SHA-256 `33` |
| Probes | `observe/other/probes.toml` SHA-256 `44` |

### game-20261101-06: a later session of the POI that failed

| Field | Content |
| ----- | ------- |
| Status | fail (1 row(s) failed) |
| Save | base `KitKraftBase` SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf` |
| Plugins | `KitKraftFirstPOI.esm` SHA-256 `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1` |
| Probes | `observe/2026-11-01-poi-plan4/probes.toml` SHA-256 `55` |

| Row | Step | Forms | Expected | Seen | Result | Evidence |
| --- | ---- | ----- | -------- | ---- | ------ | -------- |
| T-01 | `coc CityNewAtlantisApartmentTEMPLATE` | 0021966F | regex | GetInCell >> 0.00 | **fail** | - |
