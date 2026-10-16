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

## Reference sources (not assets)

Sources that inform our own work without any of their content being committed:

- **xEdit `Core/wbDefinitionsSF1.pas`**, <https://github.com/TES5Edit/TES5Edit>, MPL-2.0,
  revision `9fb016884bec138ea6c7b872cec831537d464c3e`. Starfield record definitions in
  `crates/schema/defs/` are informed by it (ADR 0004); no code or comments are copied, and
  each definition cites the revision and line range in its `xedit:` source. Details in
  `docs/xedit-reference.md`.
