# Planet content: how the official files link planets to placed content

Research note for M4.D3 (workstream W7, and the M4.E10 experiment). It records what the
typed links of the planet and world families show in the 14 official ESMs, what is known
about it and from where, and what only the Creation Kit or the game can settle. `kit
planet <form>` (`docs/mcp.md`, "`planet`") prints every chain described here for one
record; the layouts behind it are in `docs/schema/planet-family.md` and
`docs/schema/world-family.md`.

Every claim carries its source and an evidence grade (ADR 0005 Amendment 3 §3: A upstream
code, official output or a fixture we reproduce; B a documented experiment; C a secondary
overview or analogy; D a hypothesis). Counts are of winning records unless stated, measured
2026-10-15 with `crates/engine/tests/planet_corpus.rs` (release, ignored) and `kit planet`
over the 14 official files in the `docs/corpus.md` order. Nothing here claims what the game
does at runtime: that is listed under "Open questions".

## The records and their links

| Record | Role (from its definition) | Winning records |
| ------ | -------------------------- | --------------: |
| `PCMT` | tree root, one per request kind | 5 |
| `PCBN` | branch node | 365 |
| `PCCN` | content node | 2,148 |
| `LVLP` | leveled pack-in | 52 |
| `PKIN` | pack-in (a cell of references placed as a group) | 15,120 |
| `BIOM` | biome | 434 |
| `PNDT` | planet, moon, belt or station | 1,784 |
| `STDT` | star | 124 |

Which record types each typed FormID member names, and how often (every link of every
winning record of these types; `null` is `00000000`, `missing` a value that resolves to no
record, `engine_reserved` a low object ID of `Starfield.esm` no file defines). Source: our
corpus pass over the pinned definitions (A).

| Type | Member | Names (count) |
| ---- | ------ | ------------- |
| PCBN | `PRTN.ParentNode` | PCBN 351, PCMT 14 |
| PCBN | `PRVN.PreviousNode` | PCBN 270 |
| PCBN | `KWDA.Keywords` | KYWD 26 |
| PCBN | `NAM3.CountCurve`, `NAM4.DistributionCurve` | CURV 7 each |
| PCCN | `PRTN.ParentNode` | PCBN 2,148 |
| PCCN | `PRVN.PreviousNode` | PCCN 1,878 |
| PCCN | `PCCC.Content` | WRLD 1,496, PKIN 631, LVLP 12, REFR 5 |
| PCCN | `KWDA.Keywords` | KYWD 9 |
| LVLP | `LVLO.Data.PackIn` | PKIN 344, LVLP 12 |
| LVLP | `FTYP.ForcedLocRefTypes`, `LVLG.UseGlobal` | LCRT 4, GLOB 1 |
| PKIN | `CNAM.Cell` | CELL 15,118 |
| PKIN | `KWDA.Forms` | KYWD 3,492 |
| PKIN | `PRPS.Properties.ActorValue` | AVIF 181, engine_reserved 23 |
| PKIN | `SNTP.SnapTemplate`, `SNBH.SnapBehavior`, `NTRM.NativeTerminal` | STMP 269, STBH 104, TMLM 35 |
| BIOM | `GNAM.Objects.Object` | PKIN 46,870, LVLP 3,293, REFR 9 |
| BIOM | `PNAM.Flora` | FLOR 43,570 |
| BIOM | `KWDA.Forms`, `KNAM.MarkerType` | KYWD 4,642, KYWD 2,344 |
| BIOM | `RNAM.ResourceGeneration` | RSGD 432 |
| BIOM | climate, water, ambience, music, pattern style, swaps, globals, land textures | CLMT 433, WATR 329, AMBS 433, MUSC 433, PTST 434, LMSW 2,303, OSWP 13, GLOB 1,291, LTEX 5,037, GCVR 1,256 |
| PNDT | `CNAM.MasterWorldspaces.Worldspace` | WRLD 85 |
| PNDT | `EOVR.EditedWorldspaces.Worldspace` | missing 1 |
| PNDT | `PPBD.Biomes.Biome` | BIOM 3,211 |
| PNDT | `PPBD.Biomes.Fauna`, `.Flora.Flora`, `.Flora.Resource` | NPC_ 2,259, FLOR 1,596, MISC 1,596 |
| PNDT | `PPBD.Biomes.ResourceGeneration` | RSGD 11, null 3,200 |
| PNDT | `FNAM.SurfaceTree` | SFTR 1,512, null 272 |
| PNDT | `INAM.AtmosphereData.Atmosphere` | ATMO 1,473, REFR 1, null 39 |
| STDT | `PNAM.SunPreset`, `KWDA.Forms` | SUNP 123, KYWD 123 |

Read off the table (A, the same pass):

- **No tree member names a planet, biome, star or location.** The trees reach planets only
  through what a content node places: a worldspace (`PCCC` -> `WRLD`), which a planet
  lists in `CNAM`/`EOVR`. This restates `docs/schema/planet-family.md` ("routed by ...
  keywords, not by direct record links") on the full load order.
- **A planet names its biomes; a biome names its pack-ins.** `PNDT PPBD` -> `BIOM`, and
  `BIOM GNAM` -> `PKIN`/`LVLP`: a second path from a planet to placed content that does
  not go through any tree.
- **Targets outside the definition's list**: 5 `PCCC` naming a `REFR` (upstream allows
  `WRLD`, `LVLP`, `PKIN`), 9 `BIOM` objects naming a `REFR` (allowed upstream), one
  `PNDT` atmosphere naming a `REFR`. `kit planet` lists them per tree
  (`target_mismatches`).

## Trees

All five trees, walked from the nodes' upward links on the 14 official files, with no
link that fails to resolve (A: `kit planet <tree>`, `unresolved` empty for each; the
`Starfield.esm` content counts match `docs/schema/planet-family.md` exactly, pinned in
`corpus_every_pcmt_tree_is_walked`).

| Tree | Nodes | Branches | Content nodes (`Starfield.esm` alone) | Content | Deepest |
| ---- | ----: | -------: | ------------------------------------: | ------- | ------: |
| `PCM_BlockCreationRequest` | 1,181 | 178 | 1,002 (892) | WRLD 999, REFR 2, none 1 | 6 |
| `PCM_CellLoadRequest` | 764 | 114 | 649 (525) | PKIN 631, LVLP 12, REFR 3, none 3 | 6 |
| `PCM_QuestRequest` | 390 | 46 | 343 (249) | WRLD 343 | 4 |
| `PCM_ScanPlanetRequest` | 174 | 26 | 147 (132) | WRLD 147 | 4 |
| `PCM_SpaceshipLandingRequest` | 9 | 1 | 7 (7) | WRLD 7 | 2 |

- **How deep.** Content nodes sit at depth 2 (7), 3 (332), 4 (623), 5 (914) and 6 (272)
  below their tree (A).
- **Upward links only.** Every branch and content node names its parent (`PRTN`); none of
  the official `PCMT` or `PCBN` records writes the downward `PCCB` list (A; also
  `docs/schema/planet-family.md`). Siblings are ordered by `PRVN`: of 2,513 non-root
  nodes, 365 start a chain (no previous sibling), 2,136 follow one, 4 share a previous
  sibling with another node and 8 are on no chain from a first node (A: `kit planet`'s
  `order` field). What the game does with that order is not known (open question 2).
- **Default objects.** Each tree's FormID is held in the payload of exactly one `DFOB`
  record (A for the bytes; `DFOB` has no definition, so this is a stored-bytes match, not a
  typed link; reading it as "the engine finds the tree through this default object" is D).
- **Selection.** Branches choose children `Random` (230) or `Stacked` (135) (A, the
  `ChildSelection` value; its meaning in game is upstream's naming only, C).

## Routing: conditions

Branches carry 428 conditions in the five trees. The function index is typed, its table
is not transcribed (`common/Condition.toml`); the most frequent indices are 858 (90), 74
(70), 875 (54), 861 (51), 560 (40), 837 (33), 77 (28), 859 and 865 (14 each), 359 (11) and
860 (10) (A for the counts). `docs/schema/planet-family.md` names the functions behind
them (planetary body, biome, system and location keywords, globals, random percentages);
those names come from the upstream function table (C here: we have not transcribed or
checked it). Parameters, the comparison value and the reference are unions the format
cannot express yet, so they stay raw bytes, and `kit planet` shows them as stored without
evaluating any condition.

`kit planet <BIOM or LCTN>` lists `keyword_conditions`: conditions of `PCMT`, `PCBN` and
`LVLP` records whose raw parameter equals one of the biome's or location's keywords as the
condition's plugin stores it. That equality is A; that the parameter is a keyword there is
D until the function table says which parameter is a form for that function.

A planet's own keywords and name are inside its base form component blocks, which the
schema keeps raw (`docs/gaps.md`), so no typed path tells which planets a body-keyword
condition admits.

## Planets, stars, locations

- **Planet to star.** A planet's `Body.GalaxyData.StarSystemId` equals its star's
  `SystemId` (`STDT DNAM`) in all 1,786 official `PNDT` records
  (`docs/schema/planet-family.md`, A). It is a value match, not a FormID link: `kit planet
  <STDT>` lists the planets by it. Example: `SolStar` lists 47 bodies; `MarsPlanetData`
  has 10 master worldspaces placed by 12 content nodes under three trees (A,
  `corpus_a_planet_reaches_its_star_biomes_and_placements`).
- **Worldspace to location.** `WRLD XLCN` names a location (536 placed in the corpus,
  `docs/schema/world-family.md`, A); `kit planet <LCTN>` goes location -> worldspaces ->
  planets and the content nodes that place those worldspaces.
- **Few planets list a worldspace.** Only 85 master worldspace entries exist over 1,784
  winning planets (A). Most bodies are reached by no tree through a worldspace link;
  whether the game builds their surfaces from the biome path alone is D (open question 4).

## How a POI could reach a planet (M4.E10)

Two typed paths exist in the data (A): (1) a tree branch -> content node -> pack-in or
worldspace, gated by conditions; (2) planet -> biome -> biome object (pack-in or leveled
pack-in). A new POI attached the way the official content is attached would be a `PKIN`
(or `LVLP`) and either a `PCCN` under an existing branch (its `PRTN` the branch, its
`PRVN` the branch's last child) or a new `BIOM GNAM` object. Which of these spawns it, when,
and how often is D: M4.E10 tests one of them in game.

## Open questions (for the CK, M4.O1, and in game, M4.O4)

1. The condition function table: which index is which function and which parameter is a
   form (would make `keyword_conditions` A and allow evaluating routes).
2. Sibling order: does the game use the `PRVN` chain, and what do the 8 unchained and 4
   shared-previous nodes mean? Does the CK rewrite the chain when a node is added?
3. The five content nodes that name a `REFR` and the four with no content: what the CK
   shows and what the game does with them.
4. Planets without worldspaces: is their content only the biome path?
5. The `DFOB` records holding each tree: confirm the role (a CK default-object window, or
   observation that removing it stops the requests).
6. The content placement properties (per-planet and per-system limits, reset days) live in
   component blocks we keep raw; their effect on a new POI is untested.
