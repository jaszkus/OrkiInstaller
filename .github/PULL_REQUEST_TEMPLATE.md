## Summary

## Checklist

- [ ] `cargo xtask ci` green locally
- [ ] Conventional Commits message
- [ ] All repository artifacts (PR description, commit message, code, docs, CHANGELOG) written in English
- [ ] No comments in code files
- [ ] No C/C++ dependencies introduced (`cargo deny check bans`)
- [ ] Stub size within budget (`cargo xtask size-budget`)
- [ ] Waiver entry added to `docs/dependency-waivers.md` if an exception was needed
- [ ] No secrets or personal data in logs, examples, or fixtures
