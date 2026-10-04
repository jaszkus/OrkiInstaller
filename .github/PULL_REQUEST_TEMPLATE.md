## Summary

## Checklist

- [ ] `cargo xtask ci` green locally
- [ ] Conventional Commits message
- [ ] No C/C++ dependencies introduced (`cargo deny check bans`)
- [ ] Stub size within budget (`cargo xtask size-budget`)
- [ ] Waiver entry added to `docs/dependency-waivers.md` if an exception was needed
- [ ] No secrets or personal data in logs, examples, or fixtures
