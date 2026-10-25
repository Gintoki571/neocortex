# neocortex

Long-term memory for AI agents. neocortex stores what an agent learned —
facts, decisions and their reasons, preferences, mistakes and their fixes,
procedures, events — in one SQLite file, and gives it back compactly when it
matters. It is a memory, not a code-retrieval tool: people, places, projects
and how they changed over time.

- One binary: a CLI and an MCP stdio server.
- One SQLite file: BM25 (FTS5), vectors (sqlite-vec), an entity/relation graph
  (graphqlite) and bi-temporal facts.
- Pure Rust embeddings (UltraBERT-Embed-150M on candle, CPU). No Python, no
  server, no network, no LLM inside — the calling agent is the LLM.

## Build

```bash
cargo build --release
# target/release/neocortex
```

## Model setup

Embeddings need the `UltraBERT-Embed-150M` model directory (safetensors fp32,
`config.json`, `tokenizer.json`). Put it at the default location:

```bash
mkdir -p ~/.neocortex/models
cp -r /path/to/UltraBERT-Embed-150M ~/.neocortex/models/
```

or point at it explicitly:

```bash
export NEOCORTEX_MODEL_DIR=/path/to/UltraBERT-Embed-150M
```

The model is loaded lazily: the MCP server starts immediately and loads it on
the first `remember`/`recall`. For tests and wiring checks you can skip the
model entirely:

```bash
export NEOCORTEX_EMBEDDER=hash   # deterministic test embedder, not for real use
```

## Quick start

```bash
neocortex remember "Mom moved from Delhi to Pune on 2026-09-14." \
  --kind event --tags family \
  --entities '[{"name":"Mom","type":"person","aliases":["mother"]},
               {"name":"Pune","type":"place"}]' \
  --relations '[{"source":"Mom","predicate":"lives in","target":"Pune","exclusive":true}]'

neocortex recall "where does Mom live"
```

```
memories (1 of 1, ~90/1200 tokens):
[m1] event 2026-09-14 · Mom moved from Delhi to Pune on 2026-09-14.  (bm25#1 vec#1 graph#1)
facts:
[r1] Mom —live in→ Pune (since 2026-09-14)
entities: [e1] Mom (person; aka mother) · [e2] Pune (place)
```

`--json` on any command returns the full output struct.

## CLI

| command | what it does |
|---|---|
| `remember <content...>` | store a memory; flags below |
| `recall <query...>` | retrieve memories, facts and entities |
| `entity <name\|eID>` | profile (with its community), current/past facts and timeline of one entity |
| `invalidate <mID\|rID>` | end a memory or relation at a time, keeping it as history |
| `forget <mID>` | retire a memory; its sole-evidence relations are retracted (`--hard` deletes it with vectors and mentions) |
| `merge <keep> <merge>` | merge one entity into another (`--force` crosses types) |
| `alias <entity> <alias>` | add another name to an entity |
| `overview [query...]` | communities with labels, key predicates and date spans |
| `stats` | counts, database size, embedder, cold-tier hint |
| `reindex` | rebuild the graph projection from the relational tables |
| `validate` | report integrity problems (exit 1 when any) |
| `cypher <query>` | read-only Cypher over the graph projection |
| `import-remem <path>` | one-shot import of a ReMem v3 database |
| `export [--out FILE]` | write memories as JSON Lines (`--include-forgotten`) |
| `import <file.jsonl>` | replay a JSON Lines export through `remember` |
| `bench [--n N] [--queries N] [--db PATH]` | synthetic remember/recall latency report |
| `eval <fixtures.json> [--ablation] [--failures] [--set key=value] [--sweep key=v1,v2]` | evaluate retrieval quality |
| `mcp` | run the MCP stdio server |

`remember` flags: `--kind --tags --importance --scope --source --session --when
--until --entities JSON --relations JSON --keywords --supersedes`.
`remember -` reads a whole `RememberInput` JSON object from stdin.
`recall` flags: `--k --budget --kinds --tags --scope --source --session --since
--until --as-of --history --entities --keywords`.

Every CLI invocation loads the embedder before it answers: the tokenizer parse
plus the safetensors copies cost about half a second of CPU, so a run of
one-off commands pays it each time. An agent that stays in the store for a
session should start `neocortex mcp` once and keep that process resident; a
unix-socket `serve` mode was considered and is not provided, because the MCP
stdio server already is the resident path and a second protocol surface would
not pay for itself.

`cypher` runs read-only openCypher over the graph projection:
`MATCH ... [WHERE ...] RETURN ... [ORDER BY ...] [SKIP n] [LIMIT n]`. It refuses
`CREATE`/`MERGE`/`SET`/`DELETE`/`DETACH`/`REMOVE`/`DROP`/`CALL`. Memory nodes
carry `id`, `kind` and `name` (the first words of the text); entity nodes carry
`id`, `name` and `type`; edges are `MENTIONS` (`type(r)` = `MENTIONS`),
`SIMILAR` (with `score`) and `REL` (with `rid` and `predicate`).

Exit codes: `0` ok, `1` runtime error, `2` usage error.

### Export, import and benchmarks

`neocortex export [--out FILE] [--include-forgotten]` writes one JSON object per
memory, in id order, to `FILE` or stdout. Each line is a `RememberInput` plus
`id` and `expired`, so a single line can be replayed with `neocortex remember -`.
A line carries the memory's entities (name, type, description, aliases), the
relations it is evidence for (canonical predicate, fact, validity, polarity,
exclusive flag), validity times as ISO-8601 UTC, and `supersedes` (the ids it replaced).
Superseded rows are exported too, marked `"expired": true`, so the history
survives; forgotten rows are left out unless `--include-forgotten` is given.

`neocortex import FILE.jsonl` replays a file through `remember` in file order
and translates `m<id>` references in `supersedes` to the ids the new database
assigns, then retires the rows that were forgotten. Use it to restore a backup
or to move a database to another embedding model: export with the old model,
import with the new one (a database records the embedder that built it).

`neocortex bench [--n 10000] [--queries 200] [--db PATH]` generates a
deterministic synthetic corpus (xorshift seed, ~300 fictional people, places,
projects and tools, 20% with structured entities and relations, dates spread
over three years) and reports remember and recall p50/p95, remember throughput,
database size and peak RSS. It needs an empty database: without `--db` it uses
a temporary file and deletes it afterwards. `NEOCORTEX_EMBEDDER=hash` runs the
whole benchmark without the model.

## MCP registration

Claude Code:

```bash
claude mcp add neocortex -s user \
  -e NEOCORTEX_DB=$HOME/.neocortex/neocortex.db \
  -- /path/to/neocortex mcp
```

GitHub Copilot CLI (`~/.copilot/mcp-config.json`):

```json
{
  "mcpServers": {
    "neocortex": {
      "command": "/path/to/neocortex",
      "args": ["mcp"],
      "env": { "NEOCORTEX_DB": "/home/you/.neocortex/neocortex.db" }
    }
  }
}
```

opencode (`opencode.json`):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "neocortex": {
      "type": "local",
      "command": ["/path/to/neocortex", "mcp"],
      "enabled": true,
      "environment": { "NEOCORTEX_DB": "/home/you/.neocortex/neocortex.db" }
    }
  }
}
```

Any other MCP client (`mcp.json`):

```json
{
  "mcpServers": {
    "neocortex": {
      "command": "/path/to/neocortex",
      "args": ["mcp"],
      "env": { "NEOCORTEX_DB": "/home/you/.neocortex/neocortex.db" }
    }
  }
}
```

The server speaks JSON-RPC 2.0 over stdio, one JSON message per line, and
supports protocol versions `2025-06-18`, `2025-03-26` and `2024-11-05`.

## Tools

`remember`, `recall`, `entity`, `invalidate`, `forget`, `merge_entities`,
`overview`, `stats`. Every tool accepts `format: "text"` (default, compact) or
`"json"`. Tool errors come back as `isError: true` content with a message that
says what to fix.

`overview` runs deterministic Leiden community detection over the live graph
(the same seed always gives the same partition; `cN` ids are matched to the
previous run by member overlap). Each community is summarised without an LLM:
a label of its highest-degree entities, its size, the most common live
predicates, the date span of its memories, three central facts and a
representative memory. `entity` cards carry the same community as
`community: {id, label, size}`.

The agent-facing guide — when to remember, how to write a good memory, when to
recall — is in [`skills/neocortex/SKILL.md`](skills/neocortex/SKILL.md).

## How retrieval works

Four channels run in parallel: **BM25** over memory text and tags, **vector**
KNN over UltraBERT embeddings, **graph** personalized PageRank over entities,
memories and relations, and **temporal** overlap with a time window parsed from
the query. Their ranked lists are fused with weighted RRF, then lightly
adjusted for recency, importance and whether a fact is ended. MMR removes
near-duplicates and the result is packed into a token budget, whole items only.

Facts are bi-temporal: `valid_from`/`valid_to` say when something was true in
the world, `expired_at` says when neocortex retired it. Changed facts are kept
as history and are never presented as current unless you ask for history or an
`as_of` snapshot. A memory's `--until` or `invalidate m<id>` ends the facts only
that memory states; `forget` retracts them without an end date. Without
`--when`, one explicit date in the text (`March 5, 2025`, `2019`, `yesterday`)
becomes the start; several dates, or none, leave it at today.

The channel set is extensible: two measured experiments are built in but off by
default (`--set w_tlink=…` temporal neighbours, `--set w_causal=…` why-question
causal evidence; `--set causal_extract=1` also extracts one conservative
"because" relation when you pass no relations). Recall records which memories
it returned (totals plus a capped access history), applies a small retention
weight (0.05) to salience and recent access, and `stats` reports a cold
tier; nothing is ever evicted automatically.

Relation writes use tri-state `exclusive`: `true` ends competing values,
`false` keeps this write without conflict reporting, and omitted uses the
predicate policy or returns candidates. `false` can clear a sticky exclusive
flag, but never on the seeded predicate ontology. Negative assertions are
stored with polarity and end matching positive validity without deleting
history.

## How names and phrases resolve

- Same name, different type: two entity rows (casing folds), one per type.
- Typos, diacritic spellings and other scripts: names are compared on a
  transliteration key that is never stored ("Мюллер", "Müller", "Mueller" key
  to the same string). A near name that passes the hard blockers, is a
  typo-sized edit on the differing tokens (1 Damerau-Levenshtein edit up to 8
  chars, 2 beyond), and agrees with the entity cosine joins the existing card
  with `near` status ("Bergn" for "Bergen"). A lookalike that hits a blocker
  or is more than a typo away stays a separate card and is reported as
  `ambiguous` with the candidate, never silently merged: sibling names
  ("Bergen Airport"/"Bergen Airport South", "Novak Group North", "Jan
  Nováček"), label pairs ("Apt 3B"/"Apt 3C"), and other hard negatives
  ("Alex"/"Alexis", "Room 101"/"Room 102", "Teresa Mota"/"Teresa Moura").
- Same name + conflicting description: the match keeps `ambiguous` status with
  a ranked candidate; neocortex never pastes one description over the other.
- Same name among homonyms: a typed mention of a single match is a normal
  match; an honest pool returns `ambiguous` with the likely candidate first
  (scored by the entity card plus what the memory is about). Uncertain pools
  keep the memory on a placeholder entity so nothing is lost. `distinct_from`
  (entity tool, CLI `alias`-family) forces a homonym split; `merge <a> <b>`
  converges them again.
- Inverse and symmetric phrases: "X reports to Y" is stored once as "Y manages
  X" (or, for "married to", with both ends in storage order) and every side
  renders its own view: `entity` shows the phrase that starts with the profile.
- Predicates live in families ("resides in" and "based in" count as "lives
  in"): exclusivity and negative assertions are checked per family, and a
  brand new phrase joins a family when its vector is close enough. The seeded
  `cause` family (`causes`, `led to`, `resulted in`, with `caused by` and
  `because of` as inverses) makes why-questions follow causal evidence first
  when `w_causal` is on.

## Quality

On 84 queries, recall is **0.953** and MRR is **0.896**; on the independent
holdout set (88 queries) **0.913 / 0.798** (measured 2026-10-25; the ablation
numbers live in `neocortex eval <fixtures> --ablation`). Recall p50 with
UltraBERT on a 12-core laptop CPU is about **160 ms**; the 10k-memory
hash-embedder benchmark reports **~100 ms p50 / ~220-360 ms p95** including
embedding.
Retrieval uses four base channels, then RRF (`k=5`), modifiers, MMR and graph
context; access history is recorded but only changes ranking when
`w_retention` is non-zero.

## Environment

| variable | default | meaning |
|---|---|---|
| `NEOCORTEX_DB` | `~/.neocortex/neocortex.db` | database file (`--db` overrides) |
| `NEOCORTEX_MODEL_DIR` | `~/.neocortex/models/UltraBERT-Embed-150M` | embedding model directory |
| `NEOCORTEX_TZ` | system local offset | offset for relative dates and displayed dates, `+HH:MM` or `Z` |
| `NEOCORTEX_MAX_TOKENS` | `1024` | max tokens per embedding |
| `NEOCORTEX_EMBEDDER` | unset | `hash` selects the deterministic test embedder |

## License

MIT
