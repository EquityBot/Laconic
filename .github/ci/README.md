# CI tool dependencies

These private packages lock the tools used by the Claude Code and Pi load checks,
including their resolved dependencies and integrity hashes. Workflows use `npm ci
--ignore-scripts`. Claude's Linux binary is installed directly because its wrapper
normally exposes that binary through a postinstall script.

Update manifests and regenerate the corresponding lockfile on Linux with
`npm install --package-lock-only --ignore-scripts --prefix .github/ci/claude-code`
or `npm install --package-lock-only --ignore-scripts --prefix .github/ci/pi`.
Check `npm audit` and run the isolated load checks before changing the pins.

Pi's Undici override excludes the vulnerable 8.0.0 through 8.12.1 range, and its
minimatch override excludes versions before 11.0.1 (GHSA-g3hv-75wg-m249). Pi pins
minimatch 10.2.6 but calls only `minimatch()`, which 11 keeps. Remove each override
once Pi itself requires a patched version, after repeating those checks.
