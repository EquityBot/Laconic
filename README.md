<p align="center">
  <strong>Laconic: concise, answer-first output in correct Australian English.</strong>
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ryanduguid/laconic?style=flat" alt="Licence"></a>
</p>

Laconic is a skill for coding agents and chat assistants. It makes the model lead with the answer, mark what it has not verified and stop when the answer is complete, without dropping a fact, step or caveat the reader needs. It instructs the model to write every original sentence to the Australian Government Style Manual, with the house overrides listed in the skill.

## Install

Paste this into your agent:

```text
Install the laconic skill/plugin from https://github.com/ryanduguid/laconic, following the repository's AGENTS.md.
```

Or follow the [installation instructions](INSTALL.md) for your agent. For Claude Code:

```bash
claude plugin marketplace add ryanduguid/laconic
claude plugin install laconic@laconic
```

Restart the agent, then type `/laconic`. It stays on for the session until you say 'stop laconic'.

## What changes

<table>
<tr>
<td width="50%">

### Before

> Great question! Let me take a look. Your auth flow has a few moving pieces: the middleware, the token verification and the cookie handling. It's likely a caching issue, which typically resolves itself within an hour or so. Hope this helps! Let me know if you want to dig deeper.

</td>
<td width="50%">

### After

> The 401 comes from the missing `Authorization` header: the endpoint requires a bearer token and the request sends none.
>
> 1. Add `Authorization: Bearer <token>` at `src/api.ts:42`
> 2. Run `npm test -- api.spec.ts`
>
> Not verified: whether the token expires mid-session. The logs do not show it.

</td>
</tr>
</table>

## What it includes

| Piece | Path | When it applies |
|---|---|---|
| Session mode | `skills/laconic/SKILL.md` | After you type `/laconic`, for the rest of the session, until 'stop laconic' |
| Prose skill | `skills/laconic-prose/SKILL.md` | Loaded by the model for drafting, editing and prose audits; affects that task only |
| Compact rules | `rules/laconic.md` | Always loaded, if you add it to your agent's rules (about 200 words) |

The session rules, in full in [SKILL.md](skills/laconic/SKILL.md):

1. Answer first, then justify.
2. Say what you know and how. Mark unverified claims; 'I cannot judge this from the evidence I have' is an acceptable answer.
3. Length follows what the reader must decide, do or understand. Cut words, never substance.
4. Number procedures.
5. Stay on the current task.
6. Keep state visible.
7. State errors plainly: the error, the known cause and the fix or next check.
8. Estimate only with a basis.
9. Group long lists, without dropping relevant items.
10. No preamble, recap or closer.

Every piece applies Australian English (spelling, single quotation marks, commas, numbers, dates, no em dashes), falling back to British English, never American, where Australian sources do not settle a form. House rule: whilst and amongst, where the Style Manual prefers 'while' and 'among'. The prose skill adds the full writing, editing and audit workflow: a quick pass against common AI writing patterns, meaning and voice preservation, venue guides, Style Manual mechanics and AGLC4 citation notes.

## Evaluation

Measured on 19 October 2026 with the harness in `evals/`: 30 cases, 2 trials each, generated and blind-judged by Claude Opus 5.5. Each setup had its skills, rules and references installed and readable. The comparison setup was the No AI Slop skill with its compact stop-slop rules, which Laconic replaces.

| Measure | Laconic | No AI Slop setup | Difference (95% interval) |
|---|---|---|---|
| Correctness (1 to 5) | 4.68 | 3.98 | +0.70 (+0.33 to +1.07) |
| Concision (1 to 5) | 4.67 | 4.12 | +0.55 (+0.30 to +0.80) |
| Safety (1 to 5) | 4.90 | 4.85 | +0.05 (−0.10 to +0.20) |
| Weighted score | 4.74 | 4.39 | +0.35 (+0.18 to +0.54) |
| Blocking findings | 1 | 7 | |
| Mean answer length (characters) | 1,360 | 1,663 | |

A second, blind judge from another model family (GPT-6 Pro) agreed on the direction with a smaller margin: weighted score 4.63 against 4.46 (difference +0.17, 95% interval +0.05 to +0.30) and correctness +0.33 (+0.12 to +0.58). It applied the house mechanics far more strictly and found serial commas and numbers written as words in many answers from both setups.

`tools/laconic-check` counts those slips without a judge. Across the same 60 answers per setup:

| Setup | Answers with no findings | Serial comma candidates | Numbers as words | Double quotation marks |
|---|---|---|---|---|
| No AI Slop setup | 30 | 32 | 17 | 67 |
| Laconic compact rules | 40 | 13 | 15 | 1 |
| Laconic session mode (`/laconic`) | 51 | 12 | 3 | 0 |

A follow-up run on 20 October 2026 (30 cases, 1 trial) measured the later rule changes. With the numeral rule added to the compact rules, numbers written as words fell from 5 to 0 and answers with no findings rose from 18 to 23 of 30. Session mode scored 4.71 weighted with no blocking findings, against 4.59 for the compact rules alone. A shortened session skill (849 words instead of 1,153) scored lower (4.61) with 3 blockers, so the full wording stays.

Further checks on 20 October 2026:

- **Drafts checked before sending:** session mode with the `laconic-check` self-check scored 4.82 weighted against 4.71 without it on 18 prose and accounting cases (4.74 against 4.49 on the 10 accounting cases), with no blocking findings against 1.
- **Real use:** in the author's own Claude Code sessions on the day Laconic replaced the old setup, assistant messages with no `laconic-check` findings rose from 26% to 42%, and numbers written as words fell from 5.7 to 3.6 per 1,000 words.
- **Agent cost:** on 5 small repository fixes (`evals/agent_bench.py`, 2 trials), every setup passed every task. Session mode cost about 27% more per task than the compact rules, mostly because of its larger always-loaded prompt; a separate agent-guidance section saved nothing and was removed.

Limits: 18 to 60 answers per setup from one generating model; the cases are in `evals/cases.jsonl` and the rubric in `evals/rubric.md`. Serial commas remain the most common slip.

## Checker

`tools/laconic-check` is a small Rust CLI with no dependencies. `lint` flags candidates in Markdown prose: em dashes, US spellings, 'while' and 'among' as conjunction and preposition, double quotation marks, possible serial commas, numbers written as words, dates and times in non-Style Manual forms, Latin abbreviations and chatbot closers, skipping code, URLs and quoted text. It also flags chat-tool residue, such as zero-width characters, citation markers and ChatGPT tracking tags, including inside links. `preserve` reports numbers, inline code, URLs and words such as 'not', 'must' and 'only' that an edit dropped. A finding is a reason to look, not proof of a defect.

```bash
cargo run --manifest-path tools/laconic-check/Cargo.toml -- lint README.md
cargo run --manifest-path tools/laconic-check/Cargo.toml -- preserve original.md edited.md
```

`lint -` reads standard input, so an agent can check a draft without saving it. When `laconic-check` is on the PATH, both skills run it on written deliverables before sending. Its US-to-Australian word list combines a hand-written list with about 2,400 pairs generated from the [English Speller Database](https://github.com/en-wl/wordlist) and reviewed against the Style Manual (`tools/laconic-check/gen_au_words.py`). Seeded-fault tests plant one known fault per rule into clean prose and require the checker to catch each one exactly once, with no findings on the clean text.

`tools/laconic-check/mcp/server.mjs` exposes `lint` and `preserve` as MCP tools, with no dependencies beyond Node:

```bash
claude mcp add --scope user laconic-check -- node /path/to/laconic/tools/laconic-check/mcp/server.mjs
```

## Always on

To load the compact rules in every Claude Code session, copy them into your rules folder:

```bash
mkdir -p ~/.claude/rules && cp rules/laconic.md ~/.claude/rules/laconic.md
```

To start every Claude Code session in full session mode instead, create an empty flag file:

```bash
touch ~/.claude/.laconic-always
```

Delete the file to turn session mode off. Choose one or the other: session mode already contains the compact rules, so loading both repeats about 250 words in every session. [INSTALL.md](INSTALL.md) covers the other agents.

## Credits

Laconic is a fork of [i-have-adhd](https://github.com/ayghri/i-have-adhd) by Ayoub G. (MIT), which supplies the response rules, the platform adapters, the hooks and the evaluation harness. The writing guidance comes from No AI Slop, based on No AI Slop by Peter Yang and Stop Slop by Hardik Pandya, with material adapted from [humanizer](https://github.com/blader/humanizer), [avoid-ai-writing](https://github.com/conorbronsdon/avoid-ai-writing) and [Sepia](https://github.com/Nanako0129/sepia) (all MIT). The Australian English rules follow the [Australian Government Style Manual](https://www.stylemanual.gov.au/) (Commonwealth of Australia, [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)); the copy in `skills/laconic/references/` was scraped from the site on 13 September 2026, with navigation, release notes and feedback widgets removed.

## Licence

MIT, except the Style Manual material, which is CC BY 4.0. See [LICENSE](LICENSE).
