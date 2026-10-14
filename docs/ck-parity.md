# Creation Kit parity

What the real Creation Kit does, observed as a black box, and where KitCraft stands. Work on
this list is M3; the viewport and world editing are deferred to M4+.

## Observation protocol

1. Launch `CreationKit.exe` through MO2 (executable #4). Never attach a debugger,
   disassembler, or REA-style tool to it.
2. Start from a known input (a plugin we built, or an empty plugin), perform one action,
   save.
3. Diff CK's output against ours with `kit diff` and a byte diff. Record the finding below
   with the date and the CK version shown in its title bar.
4. Screenshots and CK-saved files stay in `observe/` (gitignored). Write findings in our own
   words.

## Checklist

Priority: P1 needed for data work, P2 editor convenience, P3 deferred.

| Area     | CK feature                                | Kit command       | Priority | Status   | Evidence |
| -------- | ----------------------------------------- | ----------------- | -------- | -------- | -------- |
| Plugins  | Data dialog: choose masters + active file | `load`            | P1       | planned  | -        |
| Plugins  | Save plugin (header, ONAM, masters)       | `build`, `set`    | P1       | planned  | -        |
| Plugins  | Compression choice on save                | -                 | P1       | unknown  | -        |
| Plugins  | ESM / light / medium plugin flags         | -                 | P1       | unknown  | -        |
| Records  | Edit a field in the object window         | `set`             | P1       | planned  | -        |
| Records  | "Use info" / references to a form         | `inspect`         | P1       | planned  | -        |
| Records  | Duplicate / new form, FormID allocation   | -                 | P1       | unknown  | -        |
| Records  | Localized strings on save                 | `strings.*`       | P1       | planned  | -        |
| Scripts  | Compile Papyrus                           | `papyrus.compile` | P2       | planned  | -        |
| Archives | Pack BA2                                  | `ba2.pack`        | P2       | planned  | -        |
| Checks   | Plugin errors / warnings on load          | `validate`        | P2       | planned  | -        |
| World    | Cell / worldspace editing                 | -                 | P3       | deferred | -        |
| World    | Navmesh generation                        | -                 | P3       | deferred | -        |
| Audio    | FaceFX / LIP generation, Wwise            | -                 | P3       | deferred | -        |
| View     | Render window                             | -                 | P3       | deferred | -        |

## Findings

None yet.

| Date | CK version | Action | Observed | Kit difference | Follow-up |
| ---- | ---------- | ------ | -------- | -------------- | --------- |
