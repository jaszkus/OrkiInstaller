# Contributing to OrkiInstaller

Thanks for your interest in contributing. This document describes the rules that CI and
maintainers enforce on every change.

## Language policy

All repository artifacts are written in **English only**:

- pull request titles and descriptions
- commit messages (Conventional Commits, for example `feat(pack): add FastCDC chunking`)
- code identifiers and strings
- documentation (`README.md`, `docs/*`, `CHANGELOG.md`)
- issue reports and code review comments

Polish, and any other language, is reserved for project chat and discussions outside
the repository.

## No comments in code

Code files must not contain comments. This includes `//`, `/* */`, `///` doc comments,
`#![doc]` attributes, and comments in WGSL, TOML, YAML, or Markdown embedded in code.
Names, types, and tests are the documentation. A change that seems to need a comment
should instead rename things, split a function, or add a test.

## Commit style

- [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`.
- Common types: `feat`, `fix`, `refactor`, `docs`, `chore`, `build`, `test`.
- Squash merges are used on `main`; the squash commit message should follow the same
  format.
- **No bot footers.** Commit messages must not contain `Generated with ...` lines or
  `Co-authored-by` trailers pointing at bots (Codebuff, Copilot, Claude, and similar).
  The `commit-msg` hook rejects them and `cargo xtask check-history` fails CI when one
  slips through. Contributors are humans; Dependabot is the only exception.

## Development loop

```sh
cargo xtask fmt      # formatting
cargo xtask check    # fast workspace check
cargo xtask lint     # clippy with -D warnings
cargo xtask test     # workspace tests
cargo xtask build --variant full   # one of: full, lite, headless
cargo xtask ci       # everything CI runs, locally
```

`cargo xtask ci` is the gate: run it before opening a pull request. CI on GitHub runs
the same steps on `windows-latest` and `windows-11-arm`.

## Dependency policy

- 100% pure Rust. Any dependency that compiles C/C++ sources is rejected by
  `cargo deny check bans`.
- New advisories or license issues introduced by a dependency need a waiver entry in
  `docs/dependency-waivers.md` with a reason and expiry date.
- Stub binaries must stay within the size budgets in `xtask/budgets.toml` and import
  only allowlisted system DLLs (enforced by `xtask audit-imports`).

## Pull requests

- Use the pull request template and keep every checkbox honest.
- Linear history is enforced; merges are squash-only.
- Do not include secrets, personal data, or local paths in logs, examples, or fixtures.

## Reporting issues

Open a GitHub issue in English. Include the command you ran, the exact output, and the
commit you were on. Security issues go through `SECURITY.md`, not public issues.
