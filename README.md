<p align="center">
  <strong>Laconic: concise, answer-first output in correct Australian English.</strong>
</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ryanduguid/laconic?style=flat" alt="Licence"></a>
</p>

Laconic is a skill for coding agents and chat assistants. It makes the model lead with the answer, mark what it has not verified, and stop when the answer is complete, without dropping a fact, step or caveat the reader needs. It instructs the model to write every original sentence to the Australian Government Style Manual, with the house overrides listed in the skill.

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

## The rules

The full text is in [SKILL.md](skills/laconic/SKILL.md).

1. Answer first, then justify.
2. Say what you know and how. Mark unverified claims; 'I cannot judge this from the evidence I have' is an acceptable answer.
3. Length follows the question. Cut words, never substance.
4. Number procedures.
5. Stay on the current task.
6. Keep state visible.
7. State errors plainly.
8. Estimate only with a basis.
9. Group long lists, without dropping relevant items.
10. No preamble, recap or closer.

It also applies Australian English (spelling, single quotation marks, commas, numbers, dates, no em dashes) and a plain writing pass against common AI writing patterns. For reports, documentation, emails and PR text, and for editing or auditing a draft, it loads the writing guidance in [references/writing.md](skills/laconic/references/writing.md).

## Always on

Laconic is off until you invoke it. To turn it on for every Claude Code session, create an empty flag file:

```bash
touch ~/.claude/.laconic-always
```

Delete the file to turn always-on off. [INSTALL.md](INSTALL.md) covers the other agents.

## Credits

Laconic is a fork of [i-have-adhd](https://github.com/ayghri/i-have-adhd) by Ayoub G. (MIT), which supplies the response rules, the platform adapters, the hooks and the evaluation harness. The writing guidance comes from No AI Slop, based on No AI Slop by Peter Yang and Stop Slop by Hardik Pandya, with material adapted from [humanizer](https://github.com/blader/humanizer), [avoid-ai-writing](https://github.com/conorbronsdon/avoid-ai-writing) and [Sepia](https://github.com/Nanako0129/sepia) (all MIT). The Australian English rules follow the [Australian Government Style Manual](https://www.stylemanual.gov.au/) (Commonwealth of Australia, [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)); the copy in `skills/laconic/references/` was scraped from the site on 13 September 2026, with navigation, release notes and feedback widgets removed.

## Licence

MIT, except the Style Manual material, which is CC BY 4.0. See [LICENSE](LICENSE).
