# Attribution

Every third-party asset in this repository is listed here **before** it is committed: icons,
fonts, images, sample files, test fixtures not created by us, and any vendored text.

Rules:

- Only assets whose license is compatible with MIT OR Apache-2.0 redistribution.
- No Bethesda game data, binaries, or assets of any kind (plugins, archives, meshes,
  textures, scripts, strings, screenshots of the Creation Kit).
- No assets taken from GPL/LGPL/AGPL projects.
- Test fixtures we create ourselves (synthetic plugins built by our own tests) are not listed.
- Code dependencies are tracked by `Cargo.lock`, not here.

| Asset | Path in repo | Source (URL) | Author | License | Added | Notes |
| ----- | ------------ | ------------ | ------ | ------- | ----- | ----- |
| Compiled Papyrus test fixtures (`HelloOnLoad.pex`, `OracleProperties.pex`, `OracleControl.pex`) | `crates/pex/tests/fixtures/` | Compiled locally by Bethesda's `PapyrusCompiler.exe` 4.7.0.5 (not release) from our own `.psc` beside each | KitKraft | MIT OR Apache-2.0 (our sources) | 2026-10-15 | Listed because a Bethesda tool produced them: they hold only our code, names and text plus engine type, function and flag names; no Bethesda source, script or binary (`docs/oracles.md`, "Papyrus compiler oracle") |

## Code dependencies recorded by decision

`Cargo.lock` tracks every code dependency. The ones below are also listed here because
the decision that admitted them asked for it.

| Crate | Version | Used by | Source (URL) | Author | License | Added | Decision |
| ----- | ------- | ------- | ------------ | ------ | ------- | ----- | -------- |
| `glam` | 0.34.1 (`std` only, no dependencies) | `crates/ui-egui/src/viewport/`: camera, projection and marker transforms of the 3D viewport | <https://github.com/bitshifter/glam-rs> | the glam-rs contributors (the crate names no author) | MIT OR Apache-2.0 | 2026-10-16 | M7.R1; ADR 0007 Amendment 3, dependency table |

Code dependency note (lane C, 2026-10-16; for the owner's confirmation at the M4.T3 merge,
M5.O2): `crates/game` (`kitkraft-game`, ADR 0010) adds **no** third-party crate: no HTTP
client and no process crate. It uses `std::net` and `std::process` plus workspace
dependencies already present (`serde`, `serde_json`, `toml`, `thiserror`, `regex`, all MIT
OR Apache-2.0). The third-party game plugins it talks to (Console API and the others in
`docs/source-registry.md`) are run-only tools installed by the owner, never bundled.

## Reference sources (not assets)

Sources that inform our own work without any of their content being committed:

- **xEdit `Core/wbDefinitionsSF1.pas`**, <https://github.com/TES5Edit/TES5Edit>, MPL-2.0,
  revision `9fb016884bec138ea6c7b872cec831537d464c3e`. Starfield record definitions in
  `crates/schema/defs/` are informed by it (ADR 0004); no code or comments are copied, and
  each definition cites the revision and line range in its `xedit:` source. Details in
  `docs/xedit-reference.md`.
