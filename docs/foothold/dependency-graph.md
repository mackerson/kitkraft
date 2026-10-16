# M5 dependency graph

The tickets of M5 (`plan/execution-plan.md`, "M5") and the owner actions they wait on.
A plain arrow means "must finish first". An arrow labelled `or` is one of two routes, either
of which is enough (the hexagon gathers them); a dotted arrow labelled `refines` improves a
ticket after it has run, without blocking it. Owner actions are the rounded red nodes; the
two targets of "the shortest validated path" are the double-bordered nodes, and the path's
edges are thick. M4 tasks that M5 builds on are named with their M4 ids. Update this file
in the commit that changes a ticket's dependencies.

```mermaid
flowchart LR
  classDef owner fill:#fde2e2,stroke:#b42318,color:#000
  classDef target fill:#e3f1e3,stroke:#1a7f37,stroke-width:3px,color:#000
  classDef done fill:#eeeeee,stroke:#888,color:#000
  classDef fth fill:#e8eefc,stroke:#3858c9,color:#000

  %% done on main (fcb443a)
  H2obs[M3.H2 observe: done]:::done
  E9[M4.E9 script attachment: done]:::done
  E10[M4.E10 procedural placement experiment]

  %% owner actions
  O7([M4.O7 install console plugins, INI, KitKraftBase]):::owner
  O1([M5.O1 CK sitting: F29, F30]):::owner
  MO1([M4.O1 CK sittings: F20..F28]):::owner
  O2([M5.O2 confirm crates/game deps]):::owner
  O4([M4.O4 owner game session 2, O4-07]):::owner
  O3([M5.O3 owner rows lane C leaves open]):::owner
  OX([M5.O4 owner sitting for an FTH-EXP ticket]):::owner

  %% lane A: CK oracle
  A1[M5.A1 CK session script F29 F30 F21 F20 F06]
  A2[M5.A2 xtask oracle ck]
  A3[[M5.A3 golden run: path a]]:::target
  A4[M5.A4 unattended CK experiment]
  A5[M5.A5 fixtures F20..F28]

  %% lane B: Papyrus
  B1[M5.B1 CK sources provenance]
  B2[M5.B2 PEX differential]
  B3[M5.B3 scripted activator in game]

  %% lane C: runtime (M4 lane T)
  T1a["M4.T1 (a): MO2 command line"]
  T1["M4.T1 (b)..(h): session facts, V0"]
  T2[M4.T2 probes.toml]
  T3[M4.T3 session controller]
  C1[M5.C1 ADR 0010 Am. 1 in controller]
  C2[[M5.C2 first unattended save/reload: path b]]:::target
  T6[M4.T6 V3 recipes and overhaul]
  T7[M4.T7 V4 regression replay]
  RT{{"game rows: M5.C2 or M5.O4"}}

  %% lane D: authoring
  D1[M5.D1 generated support matrix]
  D2[M5.D2 LCTN typed write]
  D3[M5.D3 TERM typed write]
  D4[M5.D4 PKIN typed write]
  D5[M5.D5 FACT typed write]
  D6[M5.D6 reference integrity incl. VMAD values]
  D7[M5.D7 K0 mini-plugin]

  %% lane F: documentation
  F4[M5.F4 ledger transcription]
  F1[M5.F1 engine limitation register]
  F2[M5.F2 gap matrix upkeep]
  F3[M5.F3 version and DLC matrix]
  F5[M5.F5 Free Lanes and Terran Armada outpost changes]

  %% lane H: research
  H1[M5.H1 hostile-encounter research]
  H2b[M5.H2 generic encounter fixture: backlog]

  %% lane E: Foothold experiments
  X1[FTH-EXP-01 authored-POI workshop]:::fth
  X2[FTH-EXP-02 interior decorator]:::fth
  X3[FTH-EXP-03 stable claim]:::fth
  X4[FTH-EXP-04 restoration stages]:::fth
  X5[FTH-EXP-05 site resource]:::fth
  X6[FTH-EXP-06 instance identity]:::fth
  X7[FTH-EXP-07 reset protection]:::fth
  X8[FTH-EXP-08 crew and navigation]:::fth
  X9[FTH-EXP-09 Free Lanes and Terran Armada interop]:::fth
  X10[FTH-EXP-10 master-aware roundtrip]:::fth
  X11[FTH-EXP-11 loose vs BA2]:::fth
  X12[FTH-EXP-12 update and uninstall]:::fth

  %% path (a)
  O1 ==> A3
  H2obs ==> A3
  H2obs --> A2
  A2 -. refines .-> A3
  A3 --> A4
  A1 --> A5
  A2 --> A5
  MO1 --> A5

  %% path (b)
  O7 ==> T1
  T1a ==> T3
  T1 ==> T3
  T2 ==> T3
  T3 ==> C1
  C1 ==> C2
  O7 ==> C2
  O2 -.-> T3
  T3 --> T6
  C2 --> T7

  %% lane B
  B1 --> B2
  O4 -- or --> B3
  T6 -- or --> B3
  O4 --> O3

  %% lane D
  D2 --> D6
  D3 --> D6
  D4 --> D6
  D5 --> D6
  D2 --> D7
  D3 --> D7
  T2 --> D7
  E9 --> D7

  %% lane H
  H1 --> H2b
  C2 --> H2b
  D7 --> H2b

  %% lane E: game rows by the controller or by the owner
  C2 -- or --> RT
  OX -- or --> RT
  D2 --> X1
  D4 --> X1
  D7 --> X1
  A5 --> X1
  RT --> X1
  X1 --> X2
  D3 --> X3
  D5 --> X3
  B3 --> X3
  RT --> X3
  RT --> X4
  A5 --> X4
  B3 --> X5
  D7 --> X5
  E10 --> X6
  RT --> X6
  RT --> X7
  A5 --> X8
  RT --> X8
  X1 --> X9
  F3 --> X9
  F5 --> X9
  A5 --> X10
  RT --> X10
  RT --> X11
  X3 --> X12
  RT --> X12
```

Tickets with no incoming edge start now: M5.A1, M5.A2 (its only dependency is done),
M5.B1, M4.T1 (a), M4.T2, M5.D1 to M5.D5, M5.F1 to M5.F5, M5.H1
(`docs/foothold/next-sprint.md`), and the owner's M5.O1. The dotted edge M5.O2 -> M4.T3
blocks the merge only if a dependency's license is not MIT or Apache-2.0 (ADR 0010
Amendment 1 (c)). M5.B3 takes either M4.O4 (owner) or M4.T6 (lane C); every FTH-EXP game
row takes either M5.C2 (controller) or M5.O4 (an owner-assisted sitting from the ticket's
testplan), so an experiment can run before the automation lands.

## Shortest validated path

From `main` at `fcb443a`; owner actions in bold (`plan/execution-plan.md`, "The shortest
validated path").

(a) A CK-authored golden fixture round-trips through KitKraft with a clean differential:

1. **M5.O1**: the owner-assisted CK sitting for F29 and F30. It can start now:
   `plan/STATUS.md` "Owner next actions" row 12 already holds the steps. M5.A1 (session
   script) and M5.A2 (`cargo xtask oracle ck`) run in parallel, off the path.
2. M5.A3: the golden run with the tools that exist (`cargo xtask observe`, `kit roundtrip
   --verify --decode`, `validate`, `kit diff`), differences classed in the ledger entry;
   M5.A2 reruns it when it lands.

(b) One unattended game session records a save/reload result:

1. **M4.O7**: the owner installs the console plugins, sets the INI switches and makes
   `KitKraftBase`. In parallel, agents: M4.T1 (a) (the MO2 command line; needs no
   M4.O7), M4.T2, and M4.T3 + M5.C1's parts that need no game.
2. M4.T1 (b)..(h): session facts and V0.
3. M4.T3 + M5.C1 completed; **M5.O2** at the merge.
4. M5.C2: the first unattended save/reload row (O2-08).
