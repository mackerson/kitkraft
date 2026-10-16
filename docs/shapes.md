# Data shapes of the inspection commands (M4.W0)

Generated from `crates/engine/src/shapes.rs` by `cargo test -p kitkraft-engine write_shape_docs -- --ignored`; a test fails when this file and the code differ. The rules (markers, `?`, `by` cases) are in `docs/mcp.md`, "Data shapes and markers". One line per member path of `data`: `name?` is not always present, `[object]` is a list of the objects below it, `<formref>` and the other `<...>` are marker objects, `value` is a decoded schema value.

## `load`

```text
runtime_basis: "inferred" | "confirmed-by-save"
save: object or null
save.path: str
save.game_version: str
save.full: int
save.small: int
save.medium: int
save.plugin_block_offset: int
save.plugin_block_size: int
save.confirmed: bool
save.only_in_load_order: [str]
save.only_in_save: [str]
save.scale_mismatch: [str]
save.differs_from_inferred: [str]
loaded: bool
source: str
language: str
plugins: [object]
plugins[].index: int
plugins[].name: str
plugins[].path: str
plugins[].active: str | null
plugins[].provider: str | null
plugins[].flags: str | null
plugins[].flag_names: [str] | null
plugins[].scale: "full" | "medium" | "small" | null
plugins[].localized: bool | null
plugins[].form_version: int | null
plugins[].masters: [object]
plugins[].masters[].name: str
plugins[].masters[].index: int | null
plugins[].missing_masters: [str]
plugins[].late_masters: [str]
plugins[].resolvable: bool
plugins[].resolve_error: str | null
plugins[].records: int
plugins[].unresolved_records: int
plugins[].full_style_self_records: int
plugins[].full_style_self_masked_records: int
plugins[].assumed_records: [object]
plugins[].assumed_records[].raw_form_id: str
plugins[].assumed_records[].form: str
plugins[].assumed_records[].basis: str
plugins[].duplicate_records: int
plugins[].runtime_slot: str | null
plugins[].runtime_basis: "inferred" | "confirmed-by-save"
plugins[].cache: "hit" | "built" | "uncached"
plugins[].error: str | null
inactive: [str]
missing_active_plugins: [str]
records: int
versions: int
cache_dir: str | null
cache_hits: int
index_ms: int
table_ms: int
elapsed_ms: int
summary: str
```

## `inspect`

```text
-- data, when `mode` = "load_order":
mode: "load_order"
load_order: str
language: str
form: str
matched_by: "formref" | "form_id" | "runtime_id" | "editor_id"
global: str
defining_plugin: str
object_id: str
runtime_id: str | null
runtime_id_basis: str | null
signature: str
editor_id: str | null
winner: str
winner_deleted: bool
winner_source: object
winner_source.safe: bool
winner_source.blocking: [str]
winner_source.unrelated: [str]
summary: str
versions: [object]
versions[].plugin: str
versions[].load_index: int
versions[].winner: bool
versions[].deleted: bool
versions[].resolved_by: str
versions[].raw_form_id: str
versions[].record: object
versions[].record.signature: str
versions[].record.flags: str
versions[].record.compressed: bool
versions[].record.offset: int
versions[].record.data_size: int
versions[].record.timestamp: int
versions[].record.version_control: int
versions[].record.form_version: int
versions[].record.unknown: int
versions[].subrecord_offsets: "file" | "decompressed"
versions[].subrecords: [object]
versions[].subrecords[].signature: str
versions[].subrecords[].size: int
versions[].subrecords[].oversized: bool
versions[].subrecords[].offset: int
versions[].subrecords[].data_offset: int
versions[].subrecords[].payload_hex: str
versions[].subrecords[].payload_truncated: bool
versions[].subrecords[].text?: str | null
versions[].subrecords[].string_id?: str
versions[].subrecords[].string_table?: str
versions[].subrecords[].string_kind?: str
versions[].subrecords[].string_offset?: int | null
versions[].subrecords[].malformed_len?: int
versions[].subrecords[].member?: str
versions[].subrecords[].value?: value
versions[].subrecords[].vmad?: any
versions[].vs_previous: object or null
versions[].vs_previous.class: "identical-to-previous" | "deleted" | "differs"
versions[].vs_previous.flags_differ?: bool
versions[].vs_previous.order_differs?: bool
versions[].vs_previous.subrecords?: [object]
versions[].vs_previous.subrecords[].signature: str
versions[].vs_previous.subrecords[].occurrence: int
versions[].vs_previous.subrecords[].change: "added" | "removed" | "changed"
versions[].vs_previous.subrecords[].member?: str
versions[].vs_previous.subrecords[].fields?: [object]
versions[].vs_previous.subrecords[].fields[].path: str
versions[].vs_previous.subrecords[].fields[].before: value | null
versions[].vs_previous.subrecords[].fields[].after: value | null
versions[].vs_previous.subrecords[].as_bytes?: str
versions[].payloads_comparable: bool | null
-- data, when `mode` absent:
plugin: str
form: str
matched_by: "form_id" | "editor_id"
record: object
record.signature: str
record.flags: str
record.compressed: bool
record.offset: int
record.data_size: int
record.timestamp: int
record.version_control: int
record.form_version: int
record.unknown: int
record.form_id: str
record.editor_id: str | null
record.groups: [object]
record.groups[].offset: int
record.groups[].type: int
record.groups[].label: str
subrecord_offsets: "file" | "decompressed"
subrecords: [object]
subrecords[].signature: str
subrecords[].size: int
subrecords[].oversized: bool
subrecords[].offset: int
subrecords[].data_offset: int
subrecords[].payload_hex: str
subrecords[].payload_truncated: bool
```

## `query`

```text
type: str
where: str | null
scope: "winner" | "all"
limit: int
matched: int
unknown: int
scanned: int
candidates: int
next_cursor: str | null
results: [object]
results[].form: str
results[].signature: str | null
results[].editor_id: str | null
results[].full: object or null
results[].full.text?: str | null
results[].full.string_id?: str
results[].full.string_table?: str
results[].full.string_kind?: str
results[].full.string_offset?: int | null
results[].full.malformed_len?: int
results[].plugin: str
results[].load_index: int
results[].raw_form_id: str | null
results[].offset: int | null
results[].runtime_id: str | null
results[].winner: bool
results[].deleted: bool
results[].resolved_by: str
unknown_records: [object]
unknown_records[].form: str
unknown_records[].plugin: str
unknown_records[].reasons: [str]
elapsed_ms: int
summary: str
```

## `diff`

```text
a: str
b: str
form: str | null
keyed_by: "name" | "raw"
counts: object
counts.added: int
counts.removed: int
counts.changed: int
counts.unchanged: int
counts.equal_by_value: int
added: [object]
added[].form: str
added[].signature: str
added[].raw_form_id: str
added[].offset: int
removed: [object]
removed[].form: str
removed[].signature: str
removed[].raw_form_id: str
removed[].offset: int
changed: [object]
changed[].form: str
changed[].signature: str
changed[].class: "identical-to-previous" | "deleted" | "differs"
changed[].a: object
changed[].a.raw_form_id: str
changed[].a.offset: int
changed[].a.flags: str
changed[].a.compressed: bool
changed[].b: object
changed[].b.raw_form_id: str
changed[].b.offset: int
changed[].b.flags: str
changed[].b.compressed: bool
changed[].flags_differ: bool
changed[].order_differs: bool
changed[].subrecords: [object]
changed[].subrecords[].signature: str
changed[].subrecords[].occurrence: int
changed[].subrecords[].change: "added" | "removed" | "changed"
changed[].subrecords[].member?: str
changed[].subrecords[].fields?: [object]
changed[].subrecords[].fields[].path: str
changed[].subrecords[].fields[].before: value | null
changed[].subrecords[].fields[].after: value | null
changed[].subrecords[].as_bytes?: str
changed[].subrecords[].a: object or null
changed[].subrecords[].a.file_offset: int | null
changed[].subrecords[].a.body_offset: int
changed[].subrecords[].a.length: int
changed[].subrecords[].b: object or null
changed[].subrecords[].b.file_offset: int | null
changed[].subrecords[].b.body_offset: int
changed[].subrecords[].b.length: int
changed[].subrecords[].first_difference: int | null
summary: str
```

## `conflicts`

```text
master: str | null
plugin: str | null
type: str | null
only_conflicts: bool
detail: "summary" | "fields" | "full"
counts: [object]
counts[].master: str
counts[].plugin: str
counts[].total: int
counts[].types: map<int>
override_records: int
forms: int
items: [object]
items[].form: str
items[].signature: str | null
items[].editor_id: str | null
items[].base_present: bool
items[].versions: [str]
items[].winner: str | null
items[].overrides: [object]
items[].overrides[].plugin: str
items[].overrides[].previous: str | null
items[].overrides[].class: "identical-to-previous" | "deleted" | "differs" | "first-version"
items[].overrides[].resolved_by: str
items[].overrides[].comparable: bool
items[].overrides[].changes: map<map<int>>
items[].overrides[].subrecords?: [object]
items[].overrides[].subrecords[].signature: str
items[].overrides[].subrecords[].occurrence: int
items[].overrides[].subrecords[].change: "added" | "removed" | "changed"
items[].overrides[].subrecords[].member?: str
items[].overrides[].subrecords[].fields?: [object]
items[].overrides[].subrecords[].fields[].path: str
items[].overrides[].subrecords[].fields[].before: value | null
items[].overrides[].subrecords[].fields[].after: value | null
items[].overrides[].subrecords[].as_bytes?: str
items[].overrides[].flags_differ: bool
class_counts: map<int>
class_counts_all: map<int> | null
next_cursor: str | null
summary: str
```

## `refs`

```text
-- data, when `direction` = "outgoing":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
depth: int
total: int
next_cursor: str | null
truncated: bool
summary: str
elapsed_ms: int
direction: "outgoing"
forms_read: int
counts: object
counts.by_status: map<int>
edges: [object]
edges[].kind: "link"
edges[].path: str
edges[].raw: str
edges[].status: "resolved" | "null" | "none" | "missing" | "engine_reserved" | "unresolvable"
edges[].to: str | null
edges[].to_signature: str | null
edges[].to_editor_id: str | null
edges[].targets: [str]
edges[].target_allowed: bool | null
edges[].resolved_by: str | null
edges[].error: str | null
edges[].offset: int | null
edges[].body_offset: int
edges[].depth: int
edges[].from: str
edges[].from_signature: str | null
edges[].from_editor_id: str | null
edges[].plugin: str
-- data, when `direction` = "incoming":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
depth: int
total: int
next_cursor: str | null
truncated: bool
summary: str
elapsed_ms: int
direction: "incoming"
scope: "winner" | "all"
counts: object
counts.referrers: int
counts.placed: int
counts.by_signature: map<int>
edges: [object]
edges[].kind: "referrer"
edges[].depth: int
edges[].group: "referrer" | "placed"
edges[].from: str | null
edges[].from_raw: str
edges[].from_signature: str
edges[].from_editor_id: str | null
edges[].plugin: str
edges[].winner: bool
edges[].to: str
edges[].placed: object or null
edges[].placed.cell: <formref>
edges[].placed.world: <formref> | null
edges[].placed.persistent: bool
edges[].placed.group_type: int
edges[].path?: str
edges[].raw?: str
edges[].status?: "resolved" | "null" | "none" | "missing" | "engine_reserved" | "unresolvable"
edges[].resolved_by?: str | null
edges[].offset?: int | null
edges[].body_offset?: int
placed_by_cell: [object]
placed_by_cell[].world: str | null
placed_by_cell[].cell: str | null
placed_by_cell[].count: int
index: object
index.plugins: int
index.cache_hits: int
index.built: int
index.uncached: int
index.elapsed_ms: int
not_searched: object
not_searched.no_definition: [object]
not_searched.no_definition[].type: str
not_searched.no_definition[].records: int
not_searched.records_with_raw_subrecords: int
```

## `planet`

```text
-- data, when `kind` = "tree":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
kind: "tree" | "planet" | "star" | "biome" | "location" | "worldspace" | "pack-in"
next_cursor: str | null
records_read: int
tree_nodes_indexed: int
elapsed_ms: int
summary: str
tree: str
ancestors: [{form, depth, parent, signature, editor_id, order, node_type, child_selection, consume_request_on_failure, count_curve, distribution_curve, parent_link, previous, content, override_content_properties, keywords, conditions, content_detail?} | {form, depth, parent, readable}]
default_objects?: [object]
default_objects[].form: str
default_objects[].editor_id: str | null
default_objects[].subrecord: str
default_objects[].payload_offset: int
default_objects[].offset: int | null
default_objects[].match: "raw-bytes"
nodes: [{form, depth, parent, signature, editor_id, order, node_type, child_selection, consume_request_on_failure, count_curve, distribution_curve, parent_link, previous, content, override_content_properties, keywords, conditions, content_detail?} | {form, depth, parent, readable}]
counts: object
counts.nodes: int
counts.branches: int
counts.content_nodes: int
counts.max_depth: int
counts.by_content: map<int>
total: int
unresolved: [object]
unresolved[].node: str
unresolved[].editor_id: str | null
unresolved[].path: str
unresolved[].raw: str
unresolved[].status: "resolved" | "null" | "none" | "missing" | "engine_reserved" | "unresolvable"
unresolved[].to: str | null
unresolved[].error: str | null
target_mismatches: [object]
target_mismatches[].node: str
target_mismatches[].editor_id: str | null
target_mismatches[].path: str
target_mismatches[].to: str
target_mismatches[].targets: [str]
-- data, when `kind` = "planet":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
kind: "tree" | "planet" | "star" | "biome" | "location" | "worldspace" | "pack-in"
next_cursor: str | null
records_read: int
tree_nodes_indexed: int
elapsed_ms: int
summary: str
planet: object
planet.worldspaces: [<link>]
planet.biomes: [<link>]
planet.resource_links: int
planet.galaxy: value | null
planet.star: object or null
planet.star.form: str
planet.star.signature: str | null
planet.star.editor_id: str | null
planet.other_links: [<link>]
placements: [object]
placements[].node: str
placements[].editor_id: str | null
placements[].content: <link> | null
placements[].tree: str | null
placements[].chain: [object]
placements[].chain[].form: str
placements[].chain[].signature: str | null
placements[].chain[].editor_id: str | null
placements[].chain[].child_selection: value | null
placements[].chain[].conditions: [value]
-- data, when `kind` = "star":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
kind: "tree" | "planet" | "star" | "biome" | "location" | "worldspace" | "pack-in"
next_cursor: str | null
records_read: int
tree_nodes_indexed: int
elapsed_ms: int
summary: str
star: object
star.system_id: value | null
star.links: [<link>]
planets: [object]
planets[].planet: object
planets[].planet.form: str
planets[].planet.signature: str | null
planets[].planet.editor_id: str | null
planets[].worldspaces: [str]
planets[].placements: [object]
planets[].placements[].node: str
planets[].placements[].editor_id: str | null
planets[].placements[].content: <link> | null
planets[].placements[].tree: str | null
planets[].placements[].chain: [object]
planets[].placements[].chain[].form: str
planets[].placements[].chain[].signature: str | null
planets[].placements[].chain[].editor_id: str | null
planets[].placements[].chain[].child_selection: value | null
planets[].placements[].chain[].conditions: [value]
-- data, when `kind` = "biome":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
kind: "tree" | "planet" | "star" | "biome" | "location" | "worldspace" | "pack-in"
next_cursor: str | null
records_read: int
tree_nodes_indexed: int
elapsed_ms: int
summary: str
biome: object
biome.objects: map<int>
biome.flora: int
biome.keywords: [str]
biome.links: [<link>]
planets: [object]
planets[].planet: str
planets[].editor_id: str | null
planets[].paths: [str]
keyword_conditions: [object]
keyword_conditions[].node: str
keyword_conditions[].signature: str
keyword_conditions[].editor_id: str | null
keyword_conditions[].condition: int
keyword_conditions[].member: str
keyword_conditions[].function: value | null
keyword_conditions[].parameter: str
keyword_conditions[].raw: str
keyword_conditions[].keyword: str
keyword_conditions[].match: "raw-bytes"
-- data, when `kind` = "location":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
kind: "tree" | "planet" | "star" | "biome" | "location" | "worldspace" | "pack-in"
next_cursor: str | null
records_read: int
tree_nodes_indexed: int
elapsed_ms: int
summary: str
location: object
location.links: [<link>]
worldspaces: [object]
worldspaces[].form: str
worldspaces[].signature: str | null
worldspaces[].editor_id: str | null
planets: [object]
planets[].form: str
planets[].signature: str | null
planets[].editor_id: str | null
placements: [object]
placements[].node: str
placements[].editor_id: str | null
placements[].content: <link> | null
placements[].tree: str | null
placements[].chain: [object]
placements[].chain[].form: str
placements[].chain[].signature: str | null
placements[].chain[].editor_id: str | null
placements[].chain[].child_selection: value | null
placements[].chain[].conditions: [value]
keyword_conditions: [object]
keyword_conditions[].node: str
keyword_conditions[].signature: str
keyword_conditions[].editor_id: str | null
keyword_conditions[].condition: int
keyword_conditions[].member: str
keyword_conditions[].function: value | null
keyword_conditions[].parameter: str
keyword_conditions[].raw: str
keyword_conditions[].keyword: str
keyword_conditions[].match: "raw-bytes"
-- data, when `kind` = "worldspace":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
kind: "tree" | "planet" | "star" | "biome" | "location" | "worldspace" | "pack-in"
next_cursor: str | null
records_read: int
tree_nodes_indexed: int
elapsed_ms: int
summary: str
detail: {cell} | {entries} | {location, planets} | null
placements: [object]
placements[].node: str
placements[].editor_id: str | null
placements[].content: <link> | null
placements[].tree: str | null
placements[].chain: [object]
placements[].chain[].form: str
placements[].chain[].signature: str | null
placements[].chain[].editor_id: str | null
placements[].chain[].child_selection: value | null
placements[].chain[].conditions: [value]
in_leveled?: [object]
in_leveled[].leveled: str
in_leveled[].editor_id: str | null
in_leveled[].paths: [str]
in_biomes?: [object]
in_biomes[].biome: str
in_biomes[].editor_id: str | null
in_biomes[].objects: int
-- data, when `kind` = "pack-in":
form: str
matched_by: str
global: str
signature: str | null
editor_id: str | null
kind: "tree" | "planet" | "star" | "biome" | "location" | "worldspace" | "pack-in"
next_cursor: str | null
records_read: int
tree_nodes_indexed: int
elapsed_ms: int
summary: str
detail: {cell} | {entries} | {location, planets} | null
placements: [object]
placements[].node: str
placements[].editor_id: str | null
placements[].content: <link> | null
placements[].tree: str | null
placements[].chain: [object]
placements[].chain[].form: str
placements[].chain[].signature: str | null
placements[].chain[].editor_id: str | null
placements[].chain[].child_selection: value | null
placements[].chain[].conditions: [value]
in_leveled?: [object]
in_leveled[].leveled: str
in_leveled[].editor_id: str | null
in_leveled[].paths: [str]
in_biomes?: [object]
in_biomes[].biome: str
in_biomes[].editor_id: str | null
in_biomes[].objects: int
```

## `coverage`

```text
plugin: str
file: str
bytes: int
decode: bool
records: int
groups: int
compressed_records: int
compressed_examined: int
record_types_seen: int
record_types_defined: int
record_types_listed: int
typed_records: int
subrecords: int
occurrences: object
occurrences.typed: int
occurrences.failed: int
occurrences.opaque: int
occurrences.unknown: int
decoded_by_confidence: object
decoded_by_confidence.reference: int
decoded_by_confidence.container-reference: int
decoded_by_confidence.corpus: int
decoded_by_confidence.ck-diff: int
decoded_by_confidence.verified: int
reencode_conflicts: int
typed_failures: [object]
typed_failures[].record: str
typed_failures[].signature: str
typed_failures[].index: int
typed_failures[].offset: int | null
typed_failures[].kind: "not_decoded" | "reencoded"
signatures: bool
top: int | null
summary: str
record_types: [object]
record_types[].signature: str
record_types[].definition: str | null
record_types[].records: int
record_types[].compressed: int
record_types[].compressed_examined: int
record_types[].typed_records: int
record_types[].subrecords: int
record_types[].occurrences: object
record_types[].occurrences.typed: int
record_types[].occurrences.failed: int
record_types[].occurrences.opaque: int
record_types[].occurrences.unknown: int
record_types[].decoded_by_confidence: object
record_types[].decoded_by_confidence.reference: int
record_types[].decoded_by_confidence.container-reference: int
record_types[].decoded_by_confidence.corpus: int
record_types[].decoded_by_confidence.ck-diff: int
record_types[].decoded_by_confidence.verified: int
record_types[].reencode_conflicts: int
record_types[].distinct: object
record_types[].distinct.seen: int
record_types[].distinct.typed: int
record_types[].distinct.opaque: int
record_types[].distinct.unknown: int
record_types[].first_offset: int
record_types[].signatures: [object] or null
record_types[].signatures[].signature: str
record_types[].signatures[].count: int
record_types[].signatures[].class: "typed" | "opaque" | "unknown"
record_types[].signatures[].failed: int
record_types[].signatures[].outside_components: int
record_types[].signatures[].confidence: str | null
record_types[].signatures[].members: [str]
record_types[].signatures[].first_record: str
```

## `schema`

```text
-- data, when `kind` = "record":
subrecords: int
typed: int
opaque: int
confidence_counts: object
confidence_counts.reference: int
confidence_counts.container-reference: int
confidence_counts.corpus: int
confidence_counts.ck-diff: int
confidence_counts.verified: int
type: str
defined: true
kind: "record"
name: str
confidence: "reference" | "container-reference" | "corpus" | "ck-diff" | "verified"
source: str
evidence: [str]
note: str | null
header_flags: [object]
header_flags[].name: str
header_flags[].mask: str
deep: bool
member: str | null
members: [object]
members[].kind: "subrecord" | "group" | "any_until"
members[].name: str
members[].confidence: "reference" | "container-reference" | "corpus" | "ck-diff" | "verified"
members[]....: (more members may follow)
summary: str
-- data, when `kind` = "header":
type: str
defined: bool
kind: "header"
name: str
confidence: "reference" | "container-reference" | "corpus" | "ck-diff" | "verified"
source: str
evidence: [str]
size: int
deep: bool
fields: [object]
fields[].name: str
fields[].type: str
fields[]....: (more members may follow)
summary: str
-- data, when `kind` absent:
-- data, when `defined` = false:
type: str
defined: bool
defined_types: [str]
summary: str
-- data, when `defined` absent:
subrecords: int
typed: int
opaque: int
confidence_counts: object
confidence_counts.reference: int
confidence_counts.container-reference: int
confidence_counts.corpus: int
confidence_counts.ck-diff: int
confidence_counts.verified: int
records: [object]
records[].subrecords: int
records[].typed: int
records[].opaque: int
records[].confidence_counts: object
records[].confidence_counts.reference: int
records[].confidence_counts.container-reference: int
records[].confidence_counts.corpus: int
records[].confidence_counts.ck-diff: int
records[].confidence_counts.verified: int
records[].type: str
records[].name: str
records[].confidence: "reference" | "container-reference" | "corpus" | "ck-diff" | "verified"
headers: [object]
headers[].type: str | null
headers[].name: str
headers[].size: int
headers[].confidence: "reference" | "container-reference" | "corpus" | "ck-diff" | "verified"
common: [object]
common[].name: str
common[].kind: "group" | "type"
common[].confidence: "reference" | "container-reference" | "corpus" | "ck-diff" | "verified"
summary: str
```

## `validate`

```text
plugin: str | null
scripts: [str]
passed: bool
errors: int
warnings: int
rules: [str]
findings: [object]
findings[].rule: str
findings[].severity: "error" | "warning"
findings[].plugin: str
findings[].record: str | null
findings[].subrecord: str | null
findings[].offset: int | null
findings[].message: str
findings_total: int
masters: [object]
masters[].name: str
masters[].path: str | null
masters[].flags: str | null
masters[].localized: bool | null
records: int
groups: int
subrecords: int
checked_form_ids: int
unresolved_form_ids: int
reserved_form_ids: int
reserved_by_member: map<int>
reserved_ids_by_member: map<[str]>
unchecked_subrecords: int
scripts_checked: int
summary: str
```

## `cell`

```text
mode: "worlds" | "interior" | "world" | "cell"
total: int
items: [object]
items[].form: str
items[].signature?: str | null
items[].editor_id?: str | null
items[].cells?: int
items[].child_records?: int
items[].versions: int
items[].x?: int | null
items[].y?: int | null
items[].base?: <formref> | null
items[].base_signature?: str | null
items[].base_editor_id?: str | null
items[].position?: [number | null] | null
items[].rotation?: [number | null] | null
items[].scale?: number | null
items[].persistent?: bool
items[].group?: "persistent" | "temporary"
items[].flags?: str
items[].deleted?: bool
items[].plugin?: str
items[].bounds?: object or null
items[].bounds.min: [number]
items[].bounds.max: [number]
interior_cells?: int
world?: str | null
world_editor_id?: str | null
cell?: str
cell_signature?: str | null
cell_editor_id?: str | null
moved_out?: int
next_cursor: str | null
summary: str
elapsed_ms: int
index_ms: int
bounds?: object
bounds.bases: int
bounds.bases_with_bounds: int
bounds.references_without: int
bounds.without_by_base_signature: map<int>
bounds.opaque_by_base_signature: map<int>
```
