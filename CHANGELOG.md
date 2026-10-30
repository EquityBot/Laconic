# Changelog

This file follows the shape of [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and is built from `git log`. The plugin and `laconic-check` are versioned separately. Dates are commit dates. Numbers in brackets are pull requests.

The manifests still declare plugin 0.5.0 and `Cargo.toml` still declares `laconic-check` 0.2.0, so a build from `main` carries those versions plus the Unreleased changes. Only `v0.4.1` and `laconic-check-v0.1.0` are tagged; neither plugin 0.5.0 nor `laconic-check` 0.2.0 has a tag or release.

## [Unreleased]

### Plugin: added

- Warn when always-on mode cannot locate the plugin (#18).
- Run the full Python test suite and eval validation in CI (#19).
- Run eval model calls in parallel with `--jobs` (#11).
- Benchmark Laconic against `humanizer` and `avoid-ai-writing` (#12).
- A revisit trigger for each shortcut marker (#24).
- A README badge row, CodeQL, `CITATION.cff` and `llms.txt` (#37).

### Plugin: changed

- Make Laconic a full replacement for stop-slop and No AI Slop.
- Forbid typical-range estimates in place of evidence.
- Move the compact rule's mechanics into a before-sending checklist (#34).
- Relicense under FSL-1.1-ALv2 (#25), replacing the Apache-2.0 licence adopted the same day (#22).
- Point repository links at EquityBot (#31).

### Plugin: fixed

- Validate eval inputs and record requested model metadata (#20).
- Make hook trimming linear (#29).

### Plugin: security

- Pin CI agent installs and disable lifecycle scripts (#28).
- Keep scenario prompts out of Claude CLI arguments and reject model values that could become options (#30).

### laconic-check: added

- Flag unnecessary hyphens (#13).
- Report protected items an edit added, failing on them only with `--strict` (#26).
- Report dropped quantifiers, modals and bounds in `preserve` (#35).

### laconic-check: fixed

- Catch prose patterns across wrapped Markdown lines (#16).
- Stop date checks from joining separate clauses (#17).
- Make dictionary parsing linear (#29).
- Stop flagging commas that join 2 clauses as serial commas (#36).

## [laconic-check 0.2.0] - 2026-10-20

### Added

- Flag staged contrasts and reveal questions (#10).

## [laconic-check 0.1.0] - 2026-10-20

### Added

- The `lint` and `preserve` commands (#2).
- The MCP server and seeded-fault measurement (#4).
- Stock-phrase linting (#5).
- Flags for mismatched weekdays (#6).
- Prebuilt binaries published on a release tag (#8).

### Changed

- Make the serial-comma rule precise enough to act on (#7).

## [plugin 0.5.0] - 2026-10-19

### Changed

- Turn the i-have-adhd fork into Laconic: concise, answer-first output in correct Australian English, with the No AI Slop prose skill merged into the response rules.
- Rename every adapter, hook, flag, manifest, test and doc to `laconic`.

### Removed

- The stale translations, upstream eval results and logo.

For versions before 0.5.0, see the history from tag `v0.4.1`.

[Unreleased]: https://github.com/EquityBot/laconic/commits/main
[laconic-check 0.2.0]: https://github.com/EquityBot/laconic/commit/b633217
[laconic-check 0.1.0]: https://github.com/EquityBot/laconic/releases/tag/laconic-check-v0.1.0
[plugin 0.5.0]: https://github.com/EquityBot/laconic/commit/3eb0a03
