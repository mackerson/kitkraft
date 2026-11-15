# Recipe: a quest with a dialogue topic and two responses

KK-279. How to author dialogue with `kit build`: a new quest (`QUST`), one topic (`DIAL`)
under it and two responses (`INFO`) under the topic, every form new, `Starfield.esm` the
only master. The spec is [`dialogue.toml`](dialogue.toml); the design it follows is
`docs/design/dialogue.md`.

**Unverified.** The layout is read off the 14 official files only. Group type 10 (quest
children) has no name on a permitted source, and no Creation Kit plugin with quest
children has been observed; the game has not been run on this recipe. Responses have no
voice file or lip data, so the game would show the text at best.

```text
kit build docs/recipes/dialogue.toml out/recipes/KitKraftRecipeDialogue.esp
```

## What it writes

| Object ID | Type | EditorID | Where |
| --------- | ---- | -------- | ----- |
| `001000` | `QUST` | `KitKraftRecipeDialogueQuest` | top-level `QUST` group |
| `001001` | `DIAL` | `KitKraftRecipeGreeting` | quest children (type 10) of `001000` |
| `001002` | `INFO` | `KitKraftRecipeHello` | topic children (type 7) of `001001` |
| `001003` | `INFO` | `KitKraftRecipeGoodbye` | topic children (type 7) of `001001` |

- The topic's `QNAM` is the quest (written from `parent`: every official topic's `QNAM`
  is the quest of its group). Its `TIFL` lists `001002`, `001003` in that order and `TIFC`
  is 2 (every official topic lists exactly the responses of its group and counts them);
  the order is the order the responses are allocated, a choice of ours.
- Each response has one response line: `TRDA` (it starts the `Response` group; `new`
  refuses a `NAM1` without it, `new.no-group-lead`) and `NAM1`, its text, inline (the
  plugin is not localized). The definition's other required members of the line (`NAM2`,
  `NAM3`, `NAM4`) are written empty, and the record's (`ENAM`, `NAM0`, `INAM`) as zeros
  (the build's `limitations` name them).
- The empty top-level `DIAL` and `INFO` groups the official files keep beside their
  nested records are written too (the writer's official placement style).

## Checked

`crates/engine/tests/dialogue_recipe_corpus.rs`
(`the_dialogue_recipe_builds_and_every_claim_holds`, release, ignored; it needs
`KITKRAFT_DATA_DIR`) builds the spec on `Starfield.esm` and checks every line above:
the four forms, their group paths, `QNAM`, `TIFL`, `TIFC`, the response texts read back
through `inspect`; `validate` 0 errors; `roundtrip` identical; a second build
byte-identical with the SHA-256 below.

SHA-256 of `KitKraftRecipeDialogue.esp`: `ab474168b7593a37ac0f34a4a7f0777a7467269a984cf2e4978b64a87a8178e2` (707 bytes).

## Variations

- A response under a vanilla topic: `parent = "Starfield.esm:XXXXXX"` (or the topic's
  EditorID). The topic is then carried into the patch as an override whose `TIFL` lists the
  new response, and its quest rides along as an unedited override, so the patch conflicts
  with any later plugin that edits either (`limitations` says so).
- A copy of a vanilla response: a `[[clone]]` with `parent = "new:topic"`; `clone`
  refuses a nested record without a `parent` and names the source's.
- Scenes (`SCEN`) and branches (`DLBR`) are refused: their payloads are not typed.
