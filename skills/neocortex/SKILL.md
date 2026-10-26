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
- dates of events and when a fact became true (`--when`; if omitted, one
  explicit date in the text such as "March 5, 2025" is used, and the output says so)

## When NOT to remember

- transient chatter, intermediate reasoning, things already in the repo
- secrets, tokens, passwords, credentials, private keys
- content you can re-read from a file, issue or commit
- near-duplicates: recall first, and update instead of adding another copy

## How to write a good memory

- **Atomic**: one fact per memory. "Deploy moved to Friday" and "Friday deploy
  needs sign-off" are two memories.
- **Self-contained**: no "it", "this", "yesterday". Name the subject and use
  absolute dates ("2026-09-14", not "last week").
- **Consistent names**: always "Mom", never "mother" in one memory and "Mom" in
  the next. Aliases belong in `entities`.
- **Always pass relations for facts**: you are the extractor. neocortex cannot
  reliably pull facts out of free text (measured: relation F1 0.07-0.20 for
  rules and GLiNER on held-out text), so a fact you do not pass as a relation
  never gets conflict handling, history or a rendered fact; multi-hop recall
  can only follow it weakly through the names it mentions. One relation per
  stated fact: `{"source": "Bob", "predicate":
  "reports to", "target": "Alice"}`; split multi-fact sentences.
- **One entity per real thing**: same name with a different type is fine (a
  person and a project both called "Zephyr" become two cards). When the
  `remember` result says `ambiguous` for an entity, the write was stored and
  the candidate list is ranked with the likely match first — confirm it with
  `same_as` (pick the candidate) or `distinct_from` (this mention is NOT that
  entity; it splits off a new card). Real different people with the same name
  and type are separate entities: do not `merge_entities` them without
  checking the facts on both cards.
- Status `near` means a typo'd or transliterated spelling ("Bergn", "Мюллер")
  already joined the right card — nothing to do. When `ambiguous` names a
  candidate, the mention is either a homonym or a sibling name
  ("Bergen Airport South" vs "Bergen Airport") whose added words carry
  meaning; the name layer refuses to auto-merge either shape. Confirm with
  `same_as` (it really is that thing) or `distinct_from` (it is not).
- **Structured when known**: pass `entities` and `relations`. Mark single-valued
  relations (`lives in`, `works at`, `is married to`, `has phone number`)
  `exclusive: true` so the previous value is ended and kept as history.
  Use `exclusive: false` to keep competing values without a conflict hint;
  omit it to use predicate policy or return candidates. Exclusivity sticks to
  the predicate for future writes, so never mark multi-valued ones (`uses`,
  `knows`, `likes`, `depends on`) exclusive. Note: `exclusive: false` can
  clear a sticky flag you set earlier, but never changes the built-in policy
  of seeded predicates ("lives in", "works at", "reports to", ...).
- Predicates come in families: "resides in" or "based in" automatically count
  as "lives in"; "reports to" and "manage" both work and render from the
  subject's side ("X reports to Y" shows as "X —reports to→ Y" on X's card and
  "Y —manages→ X" on Y's). Use whichever phrase is natural.
- **Causes are relations too**: state them explicitly (`{"source": "disk full",
  "predicate": "caused", "target": "deploy failed"}`); `causes`, `led to`,
  `resulted in`, `caused by` and `because of` share one seeded family, and an
  inverse write is stored from the cause side. Why-questions then surface the
  cause evidence first. When you pass no relations, `causal_extract=1` can pull
  one conservative "A because B" / "A due to B" relation, but do not rely on it:
  you are the extractor.
- Pass `negated: true` for retractions such as "no longer likes" or "X does not
  support Y"; negative relations render as `—NOT→` and end the matching
  positive interval of the same fact (reasserting the positive later is normal
  history). A retraction sentence with no relation ("Ivan left Acme; he no
  longer works there") still ends the one live fact of that kind it names at
  the memory's `valid_from`, reported in `invalidated`; if several facts match
  it returns `conflicts` and ends nothing, so say which one you mean. A live
  fact recorded now is retired even when you date the retraction earlier
  ("Maria divorced Tom in 2024"). The name must be the whole object of the
  cue: "left the Acme office" and "left Acme a voicemail" end nothing, while
  "left his job at Acme" ends the work fact, and a hedged or reported sentence
  ("almost quit", "thinks Bob left") returns a conflict candidate instead. When
  `remember` returns `conflicts` or `related`, decide whether
  to invalidate or supersede the candidate; neocortex never guesses and never
  deletes history.
- Plans and routines are read from the text: "we will move next spring" gets a
  future start (or the tag `planned` when no date resolves) and is never
  current, so a planned move does not answer "where does X live"; "standup
  every Monday" keeps the rule and gets the tag `recurring`. Ask for plans
  explicitly ("upcoming", "planned", "what is scheduled") to see them.
- **Correct, don't contradict**: use `supersedes: ["m12"]` for a newer memory
  that replaces an older one, or `invalidate m<id>` when a fact stopped being
  true (it also ends the facts that memory alone states).
  Never write two live facts that disagree.
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

- `[m42]` memory, `[r9]` relation, `[e3]` entity. Pass ids back with or without
  the prefix.
- `[planned]` / `[recurring]` mark a future plan or a rule; neither is a current
  fact. A date range with `[ended]` or "ended" is **history**: true then, not now.
  `retracted` means the source was forgotten, not that it stopped being true.
  Current facts have `since <date>` or no end.
- Ordering questions ("what happened first") return items sorted by start date;
  duration questions ("how long did Alice work at Acme") add the computed span,
  e.g. "3 years 2 months" or "ongoing since 2023-04-01". Ask for history when
  the answer is about the past.
- `(bm25#2 vec#1 graph#1)` says which channels matched; more agreeing channels
  means a stronger hit. `tlink` (temporal neighbours) and `causal` (why-answer
  evidence) appear only when those optional channels are enabled.
- "no memories matched" means broaden the query or drop filters, not that the
  memory does not exist.
- A `next:` line means recall stopped one hop short, e.g.
  `next: follow e151 Mira Lindqvist for where live — chain ends at a person`.
  Call `follow` with that `entity` and `want`, then answer from what it returns.
  Do not guess the missing name.

## Tools (MCP) and CLI equivalents

Prefer the MCP tools when the `neocortex` server is connected. Otherwise use the CLI;
the installed binary is `~/.neocortex/bin/neocortex` (DB `~/.neocortex/neocortex.db`).

| MCP tool | CLI | Use |
|---|---|---|
| `remember` | `neocortex remember <text>` | store a memory |
| `recall` | `neocortex recall <query>` | retrieve |
| `follow` | — (MCP only) | walk relations hop by hop (also the `next:` hint) |
| `entity` | `neocortex entity <name\|eID>` | profile of one entity |
| `invalidate` | `neocortex invalidate <mID\|rID> --at <date>` | end a fact |
| `forget` | `neocortex forget <mID> [--hard]` | retire/delete (facts it alone stated are retracted) |
| `merge_entities` | `neocortex merge <keep> <merge> [--force]` / `alias <entity> <alias>` | fix names (+ `distinct_from` on the `remember` entities array to split a homonym) |
| `overview` | `neocortex overview [query]` | communities with labels, predicates and date spans |
| `stats` | `neocortex stats` | counts, model status and the cold-tier hint |

Every tool takes `format: "text"` (default) or `"json"`.
Use `overview` for a compact global view of related entities, facts and a
representative memory: each community carries a label of its top-degree
entities, its size, key predicates and date span. Use `entity` for one
entity's facts and timeline; its card names the community it belongs to.

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

Recall before deciding:

```
neocortex recall "how do we deploy" --tags deploy
neocortex recall "what did we decide about storage" --history
```

CLI, same memory:

```bash
neocortex remember "Chose SQLite over Postgres for neocortex: one file, no server." \
  --kind decision --tags architecture,storage --importance 0.8
neocortex recall "storage choice" --k 5 --json
```

Write absolute dates; neocortex resolves relative dates in the machine's local
timezone.
