---
name: neocortex
description: Long-term memory for AI agents. Use when you should remember a durable fact, decision, preference, mistake or outcome, or when you need to recall what was known earlier (people, projects, environment, past work, past decisions).
---

# neocortex

neocortex is a local long-term memory that survives across sessions. It stores
memories, entities, relations and time-bounded facts in one SQLite file, and
retrieves with keyword, vector, graph and temporal search. You are the only
writer: nothing is inferred beyond what you pass.

## When to remember

Store something when it will still matter next week:

- decisions and **why** they were made (rejected alternatives included)
- user preferences, constraints and working style
- stable facts about people, places, projects, tools and the environment
- mistakes and the fix that worked, so they are not repeated
- procedures that worked (build steps, deploy order, gotchas)
- outcomes: what shipped, what failed, what was measured
- dates of events and when a fact became true (`--when`; if omitted, the date
  written in the text is used and the output says so: "March 5, 2025",
  "last Tuesday", "two weeks ago", "last month", "this morning", or a written
  range "from 2026-07-06 to 2026-08-14" / "between March 1 and March 5" /
  "until 2026-10-15", which sets both ends. `--when`/`--until` always win)

## When NOT to remember

- transient chatter, intermediate reasoning, things already in the repo
- secrets, tokens, passwords, credentials, private keys
- content you can re-read from a file, issue or commit
- near-duplicates: recall first, and update instead of adding another copy

## How to write a good memory

- **Atomic**: one fact per memory. "Deploy moved to Friday" and "Friday deploy
  needs sign-off" are two memories.
- **Self-contained**: no "it" or "this". Name the subject. Absolute dates
  ("2026-09-14") stay readable forever and are preferred; a relative date
  ("last week", "two weeks ago", "last Tuesday") is resolved against the write
  clock and stored as a date, so it is safe when you do not know today's date,
  but the text will still say "last week" to a reader.
- **Consistent names**: always "Mom", never "mother" in one memory and "Mom" in
  the next. Aliases belong in `entities`.
- **Always pass relations for facts**: you are the extractor. Text alone never
  becomes a graph fact (measured: relation F1 0.07-0.20 for rules and GLiNER on
  held-out text), so a fact passed only as text gets no conflict handling, no
  history and no rendered fact. Explicit entities and relations always beat
  plain text. One relation per stated fact: `{"source": "Bob", "predicate":
  "reports to", "target": "Alice"}`; split multi-fact sentences.
- **One entity per real thing**: same name with a different type is fine (a
  person and a project both called "Zephyr" become two cards). When the
  `remember` result says `ambiguous` for an entity, the write was stored and
  the candidate list is ranked with the likely match first. Confirm with
  `same_as` (pick the candidate) or `distinct_from` (this mention is NOT that
  entity; it splits off a new card). Real different people with the same name
  and type are separate entities: do not `merge_entities` them without
  checking the facts on both cards.
- **Version spellings are one name**: "Postgres 16", "Postgres v16" and
  "Postgres 016" resolve to one entity, while "Postgres 14" and "Postgres 16"
  are different versions (and a version is not its bare product name). Text
  like "Postgres 14/16" is stored as two version entities.
- **Symbol names are names**: "C#", "C++", "F#" and "C" are distinct tool
  entities (normalised "c sharp", "c plus plus", "f sharp"); pass them exactly
  and never merge them. A database written before this rule may still hold them
  on one card, and those facts are not migrated: check `entity`, `forget` the
  corrupted memory, write the fact again, and add `distinct_from` when the name
  alone still resolves to the old card.
- Status `near` (a transliteration variant, or a typo with a corroborating
  shared person/org in the same write) needs nothing. `ambiguous` (homonym,
  sibling name, or a typo-sized near name without corroboration) is never
  auto-merged: confirm with `same_as` when it is the same person, or
  `distinct_from` when it is not.
- **Structured when known**: pass `entities` and `relations`. Mark single-valued
  relations (`lives in`, `works at`, `is married to`, `has phone number`)
  `exclusive: true` so the previous value is ended and kept as history.
  `exclusive: false` keeps competing values without a conflict hint; omit it to
  use predicate policy or return candidates. Exclusivity sticks to the predicate
  for future writes, so never mark multi-valued ones (`uses`, `knows`, `likes`,
  `depends on`) exclusive. `exclusive: false` can clear a sticky flag you set
  earlier; it never changes the built-in policy of seeded predicates.
- Predicates come in families: "resides in" or "based in" count as "lives in";
  "reports to", "accountable to" and "manage" both work and render from the
  subject's side. Ask either way: "who does Mira report to" and "who is Mira's
  manager" both find the stored "Noah manages Mira" and render it from Mira's
  side ("Mira reports to Noah"), while "who does Mira manage" still answers the
  other direction. Role nouns (manager, boss, supervisor, mentor, mentee) are
  read-side only; they are not write predicates.
- **Causes are relations too**: `{"source": "disk full", "predicate": "caused",
  "target": "deploy failed"}`. `causes`, `led to`, `resulted in`, `caused by`
  and `because of` share one seeded family, and the inverse is stored from the
  cause side. A why-question follows the cause one step out from the memory
  that answered it (the cause memory and its fact come back even when that
  memory never names the cause). The `causal_extract` text-cue extractor is an
  eval-only knob (off); a cause you do not pass is not stored.
- **Negation**: pass `negated: true` for "does not support Y"; it renders as
  `—NOT→` and ends the matching positive interval. JSON facts carry `"polarity": 0`
  next to the explicit negative `fact` text.
- **Nicknames are aliases**: `{"source": "Robert Okafor", "predicate":
  "nickname", "target": "Bob"}` stores no fact; the target becomes another name
  of the same entity (prefer `entities[].aliases`). Any relation whose two ends
  resolve to the same entity is dropped, and the reply names it in `hints`
  (`dropped self-relation: Robert Okafor nickname Bob (kept "Bob" as an alias)`).
- **Renames join the old and new name**: a statement like "Harbor was renamed
  to Lantern" (as text, or as a relation with predicate "renamed to" / "now
  called" / "formerly called") merges the two names into one entity — the old
  name stays as an alias of the surviving (new) name, old facts move to it,
  and a "Lantern formerly called Harbor" fact keeps the old name answerable.
  A hedged ("might rename", "plan to rename", "planned for Q3"), negated
  ("did not rename"), questioned ("Was X renamed?"), reverted ("then back
  to Harbor"), passive ("called by the team"), written-generic (a side
  written all-lowercase — "the repo", "is now deprecated" — or the word-list
  nouns "the branch", "the meeting", a file name), bare-first-name, or
  undated both-facts rename merges nothing, so write it as a plan instead and
  merge by hand when it happens. "X is now Y", "Y (now Z)" and "Y (renamed
  from X)" also merge; a bare "is now Y" additionally needs Y capitalised in
  the text and typed like X.
- **Text-only retraction**: a sentence with no relation ("Ivan left Acme; he no
  longer works there") ends the one live fact it names, at `valid_from`, and
  reports it as `invalidated`. It also runs on a write that supplies its own
  relations, for a *different* fact the prose names ("Alice no longer leads the
  core team; Bob took over" with only Bob's relation): what you just supplied
  is never retracted by the same sentence. Cues that end a relation: "left his
  job at X", "left X" (work at), "quit X", "resigned from X", "moved out of X"
  (lives in), "divorced" / "broke up with" (married to), "sold X" (own),
  "stopped using X", "gave up X", "no longer ..." (any predicate). A first name
  resolves through the entities you supplied and the stored people ("Maria
  divorced Tom" with Maria Duarte and Tom Duarte), and a first name shared by
  several people only returns a conflict. A pair before the cue ("Acme and
  Northwind are no longer partners") ends the pair's relation. The name must be
  the whole object of the cue: "left the Acme office", "left Acme a voicemail"
  and "Acme's parking lot" end nothing, while a trailing date ("... no longer
  reviews X since August 2026") is a time tail and still ends it. A hedge or report ("almost quit",
  "might have left", "thinks Bob left") returns a `conflicts` candidate and
  ends nothing. If several live facts match, the result is `conflicts` and
  nothing ends, so say which one you mean. A live fact recorded now is retired
  even when the retraction is dated earlier ("Maria divorced Tom in 2024").
- **Plans and routines** come from the text: "we will move next spring" gets a
  future `valid_from` (or tag `planned` when no date resolves) and is not a
  current fact. A question about the planned event itself ("when does the
  deploy freeze start", "is the v1 API deprecated") still finds it.
  "standup every Monday" keeps the rule and gets tag `recurring`.
- **Correct, don't contradict**: use `supersedes: ["m12"]` for a newer memory
  that replaces an older one, or `invalidate m<id>` when a fact stopped being
  true. Never write two live facts that disagree. When `remember` returns
  `conflicts` or `related`, decide whether to invalidate or supersede; neocortex
  never guesses and never deletes history.
- Pick a `kind`: `fact`, `event`, `decision`, `preference`, `mistake`,
  `procedure`, `observation`, `note`. Add `tags` for filtering and
  `importance` (0..1, ~0.7+ for things the user will ask about again).

## When to recall

- at the start of a task, before planning
- before a decision that may depend on past context
- when the user refers to something from before ("as we discussed", "last time")
- when asked why something happened: recall the event and its cause together;
  a stored `causes` relation is the strongest evidence
- when stuck: the answer or a past fix may already be stored

## How to read results

Recall prints `memories (N of M, ~T/B tokens):`, then `facts:`, `entities:`
and `next:` when present.

- `[m42]` memory, `[r9]` relation, `[e3]` entity. Pass ids back with or without
  the prefix.
- `facts:` lines are relations: `[r9] Mom —live in→ Pune (since 2026-09-14)`.
  `—NOT→` is a negation. Current facts show `since <date>` or no end.
- `[1 of N current]` appears only on a single-valued predicate (seeded: lives
  in, works at, married to, reports to, ...; or one you flagged `exclusive:
  true`) when more than one value is still live. It means two facts disagree:
  invalidate the stale one. Multi-valued predicates (uses, depends on, likes)
  never carry it; their several current values are ordinary. `owns` and `leads`
  are keyed on the object instead: a named repo has one owner, two people can
  own a laptop. JSON has the same count in `current_values`.
- `[planned]` / `[recurring]` mark a future plan or a rule; neither is a current
  fact. A range with `ended` is **history**: true then, not now. `retracted`
  means the source was forgotten, not that the fact stopped being true.
- Planned and future-dated memories are left out unless the question asks about
  plans ("upcoming", "planned", "scheduled"), names a future window, or asks
  about that event ("when does the deploy freeze start", "is the v1 API
  deprecated").
- A month, year or date range in the question is a window when the question
  scopes itself to it ("in August 2026", "during 2025", "on 2026-08-01"):
  memories whose stored dates fall outside it are dropped, while memories with
  no stated date still appear (their `valid_from` is only when they were
  written), so check the date on each line before answering "what happened in
  August". An event whose text states a range ("from 2026-10-26 to 2026-11-06")
  counts as spanning it. A month that names the subject ("the December 2026
  conference") is not a window: the question is about that event, not about
  December.
- A memory-lapse phrasing ("I don't remember ...", "I forgot ...", "remind me
  ...") is a request, not a negation: pass the question itself to recall.
- An exclusion question ("which service is not in Rust", "what database does
  Lumen not use", "everything except Rust") asks which item lacks a property:
  the memories of the items that lack it come first, and the facts that give
  the property to the other items are kept last. When no stored entity type or
  property matches, it is answered like an ordinary question.
- A status question ("what bug is still open", "which issues are unresolved")
  ranks the defect reports no later memory fixes first; a bug a later memory
  fixes is still shown, after them. A report that says it is not fixed yet
  ("not fixed yet", "no fix yet", "needs a fix") is an open report. When the
  question names one defect ("is the login bug still open"), only that
  defect's report is re-ranked and it keeps its place against unrelated bugs.
  The question has to name a defect ("is the API still open to the public" and
  "which PRs are outstanding" are ordinary questions), and "open question" is
  a pending decision, not an open defect.
- A why-question also returns the cause: after the first pass, the stored
  cause of the memory that answered it (a `cause` relation that ends at an
  entity of the question, or whose memory shares a specific entity with the
  answer) is added to the memories and its fact leads `facts:`. A cause that
  shares only a broad entity, such as a team every note mentions, is ignored.
- History is automatic for past-tense wording ("used to", "before",
  "previously", "changed", "did ... use", "no longer"), so ended facts appear
  without `include_history`. Use `as_of` for "what was true then". A yes/no
  status question that names both ends ("is npm still used in atlas") also
  shows the ended fact between them with its end date, so the answer is "no,
  ended <date>" rather than silence.
- Ordering questions ("what happened first", "in order") return items sorted by
  start date. Duration questions ("how long did Alice work at Acme") add the
  computed span from the stored interval: "3 years 2 months", "3 days", or
  "ongoing since 2023-04-01"; the memory that answers the question stays first.
- `(bm25#2 vec#1 graph#1)` lists the matching channels; more agreeing channels
  means a stronger hit.
- "no memories matched" means broaden the query or drop filters, not that the
  memory does not exist.
- `next:` means recall stopped one hop short, e.g.
  `next: follow e151 Mira Lindqvist for where live — chain ends at a person`.
  Call `follow` with that `entity` and `want`, then answer from what it returns.
  Do not guess the missing name.
- `remember` prints `saved m<id>` (or `duplicate of`), its `entities:` and
  `relations:` with statuses, `invalidated:`, `superseded:`, `conflict:`,
  `related:`, `possible_supersedes:`, and `hint:` lines. A `hint:` names the next action (invalidate or
  re-send with `exclusive=true`, pass `supersedes`, or pass relations).
  `possible_supersedes` (only when the server runs with `NEOCORTEX_CORRECTION_HINTS=1`)
  lists older memories this text may correct; nothing is ended, so decide with
  `supersedes` or `invalidate`. A recalled memory with `[maybe superseded by m<id>]`
  has a later correction.

## Tools (MCP) and CLI equivalents

Prefer the MCP tools when the `neocortex` server is connected. Otherwise use the
CLI; the installed binary is `~/.neocortex/bin/neocortex` (DB
`~/.neocortex/neocortex.db`). Every MCP tool takes `format: "text"` (default) or
`"json"`.

| MCP tool | CLI | Use |
|---|---|---|
| `remember` | `neocortex remember <text>` | store a memory |
| `recall` | `neocortex recall <query>` | retrieve (defaults: k 8, budget 1200 tokens) |
| `follow` | — (MCP only) | walk relations hop by hop, from one entity (optionally to another) |
| `aggregate` | — (MCP only) | count and list the entities one relation reaches |
| `entity` | `neocortex entity <name\|eID>` | profile of one entity; a first name several people share lists the others as `candidates` |
| `invalidate` | `neocortex invalidate <mID\|rID> --at <date>` | end a fact, keep history |
| `forget` | `neocortex forget <mID> [--hard]` | retire a memory (soft) or delete it (hard) |
| `merge_entities` | `neocortex merge <keep> <merge> [--force]` / `alias <entity> <alias>` | fix names; `distinct_from` on a `remember` entity splits a homonym |
| `overview` | `neocortex overview [query]` | communities with labels, predicates and date spans |
| `stats` | `neocortex stats` | counts, model status and the cold-tier hint |

When to use each:

- `recall`: first call for any question; add filters only when the answer's
  shape is known.
- `follow`: only when a `next:` hint names it, or recall found the neighbourhood
  but not the answer. Optional `to`, `predicate`, `depth` (1-3).
- `aggregate`: how-many, which-ones and list questions over one relation ("who
  works at Acme", "how many pets"). `source` or `target` fixes one end. Negated
  facts come back under `negative`, not in the count.
- `entity`: everything known about one person, place or project. A bare name
  shared by several homonyms (people with that first name, plus any entity
  named exactly that) returns the best card plus `candidates` (an `also:` or
  `ambiguous:` line in text output) with `status: "ambiguous"`, listing up to
  10 of them and counting the rest in `candidates_more`: ask again with the
  full name before answering about that person.
- `overview`: orienting in an unfamiliar knowledge base, or `query` to see one
  topic's neighbourhood. Not a source of individual facts.
- `invalidate`: a fact stopped being true at a time (`at`, default now). Prefer
  it over writing a contradicting memory. Accepts `m<id>` or `r<id>`.
- `forget`: wrong, private or unwanted memories. Not for facts that merely
  changed; use `invalidate`.
- `merge_entities`: one thing written under two names (merge, or add an alias).
  Not for two real people who share a name.
- `stats`: reachability check and size (`cold` is a hint; nothing is evicted).

## Small machines

- Default is f32 (`model.safetensors`; cold recall peaks near 610 MiB). When
  free RAM is under about 1 GB, use q8: `neocortex quantize-model <f32 model
  dir> <out dir>` writes `model.q8_0.gguf` plus config and tokenizer, from the
  f32 dir. Point `NEOCORTEX_MODEL_DIR` at the out dir (a dir without
  `model.safetensors` is picked up as q8; `NEOCORTEX_EMBED_DTYPE=f32|q8|q4k`
  forces one). q8 peaks near 237 MiB at +15-45% warm CPU, and shares the f32
  identity, so existing databases keep working.
- Vectors: `NEOCORTEX_VEC=int8` makes a **new** database store one byte per
  element (world-300 DB 21.3 MB -> 8.9 MB). An existing database converts once,
  one way: `neocortex migrate-vectors --to int8`. Holdout MRR drops 0.794 ->
  0.792, so keep f32 unless disk is the limit.

## Opt-in features (off by default)

- Reranker: `NEOCORTEX_RERANK=<N>` reranks the top N candidates with a
  cross-encoder; it needs the `ms-marco-MiniLM-L-6-v2` model dir
  (`NEOCORTEX_RERANK_MODEL`). Without the model, recall is unchanged.
- Prose fact lines (a relation-less memory rendered as a fact in `facts:`) are
  off: `prose_margin` is infinity, and only `eval --set prose_margin=0` enables it.
- The relation-fact seed prefilter is off: `eval --set rel_prefilter=on` reads
  only the relations incident to the first query anchors instead of the global
  relation-vector KNN. Keep it off unless a large database needs the latency.

## Examples

Store a decision with structure:

```json
{"content": "Chose SQLite over Postgres for neocortex because it ships as one file with no server.",
 "kind": "decision", "tags": ["architecture", "storage"], "importance": 0.8,
 "entities": [{"name": "neocortex", "type": "project"},
              {"name": "SQLite", "type": "tool"}, {"name": "Postgres", "type": "tool"}],
 "relations": [{"source": "neocortex", "predicate": "uses", "target": "SQLite"}],
 "keywords": ["storage choice"]}
```

Record a change instead of a contradiction:

```json
{"content": "Mom moved from Delhi to Pune on 2026-09-14.",
 "kind": "event", "tags": ["family"], "valid_from": "2026-09-14",
 "entities": [{"name": "Mom", "type": "person", "aliases": ["mother"]},
              {"name": "Pune", "type": "place"}],
 "relations": [{"source": "Mom", "predicate": "lives in", "target": "Pune", "exclusive": true}]}
```

Recall before deciding: `neocortex recall "what did we decide about storage" --history`. Past-state phrasing (for example, "who used to lead", "who formerly owned", or "the project I led") retrieves ended facts with their end dates; current-state questions continue to prefer live facts.

Write absolute dates; neocortex resolves relative dates in the machine's local
timezone (`NEOCORTEX_TZ` overrides it).
