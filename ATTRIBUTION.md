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

## Reference sources (not assets)

Sources that inform our own work without any of their content being committed:

- **xEdit `Core/wbDefinitionsSF1.pas`**, <https://github.com/TES5Edit/TES5Edit>, MPL-2.0,
  revision `9fb016884bec138ea6c7b872cec831537d464c3e`. Starfield record definitions in
  `crates/schema/defs/` are informed by it (ADR 0004); no code or comments are copied, and
  each definition cites the revision and line range in its `xedit:` source. Details in
  `docs/xedit-reference.md`.
