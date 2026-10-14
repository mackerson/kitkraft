# corpus/

The roundtrip corpus is **external**. The official Starfield ESMs live in the game install
and are read in place through `KITCRAFT_DATA_DIR`; they are never copied here or committed.

This directory holds only:

- `manifest.toml`: file names, sizes, and SHA-256 of the official ESMs the tests expect.
- `out/` (gitignored): scratch output from local roundtrip runs, if any.

Plugin, archive, script, and string files under `corpus/` are gitignored as a safety net.
See [docs/corpus.md](../docs/corpus.md).
