# Security policy

Laconic runs code on your machine: its hooks and plugins start with your coding agent, and its MCP server launches a local binary. Please report suspected vulnerabilities privately.

## Report a vulnerability

Use GitHub's private vulnerability reporting: open the [Security tab](https://github.com/EquityBot/laconic/security) and choose 'Report a vulnerability'. Do not open a public issue or PR for a suspected vulnerability.

Include the affected file or command, the commit or release, the steps to reproduce and the impact you expect.

Laconic has one maintainer, so handling is best effort and no response or fix time is guaranteed. A confirmed vulnerability is fixed on `main` as soon as the maintainer can manage and disclosed in a GitHub security advisory. Tell us in your report if you want credit.

## Supported versions

Only `main` and the latest `laconic-check` [release](https://github.com/EquityBot/laconic/releases) are supported. Earlier commits and releases receive no fixes.

## In scope

- Hooks in `hooks/`, which run when an agent session starts.
- Plugins and extensions in `.opencode/plugins/` and `extensions/`.
- `tools/laconic-check`: the CLI, `mcp/server.mjs` and the binaries on GitHub Releases.
- Install, update and uninstall commands in `INSTALL.md` that do more than they state.
- `scripts/` and `.github/workflows/`, where untrusted input could run code or expose a secret.
- Skill and rule text that tells an agent to do anything the safety list in `CONTRIBUTING.md` forbids, such as reading credentials or bypassing confirmation.

## Out of scope

- Vulnerabilities in the agent runtimes, models or third-party packages themselves. Report those to their vendors.
- A model that ignores or misapplies the rules.
- Findings `laconic-check` misses or flags wrongly, which are ordinary bugs unless they run code or expose data.
- Upstream projects Laconic derives from, including i-have-adhd.
