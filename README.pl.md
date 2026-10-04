# OrkiInstaller (orki)

Installer builder napisany w czystym Ruście z UI iced + wgpu — alternatywa dla NSIS, projektowana najpierw pod Tauri v2.

## Status

Faza bootstrap (faza 0 roadmapy). Workspace buduje się i testuje na zielono; silnik pakowania, UI i system shaderów są w aktywnej budowie.

- [x] Szkielet workspace, pinowany toolchain, CI
- [x] `orki-core`: typowany manifest (`orki.toml`), sanityzacja ścieżek, model operacji/transakcji, kody wyjścia MSI
- [x] `orki-pack`: format kontenera `.orkipack` (footer, manifest, integralność chunków BLAKE3 + CRC32)
- [x] `orki-stub`: runtime z trybami silent/passive/uninstall i flagami zgodnymi z NSIS/Tauri
- [x] CLI `orki`: `wrap`, `init` (także `--tauri`), `check`, `schema`, `doctor`
- [ ] UI iced + wgpu i system shaderów (spike fazy 0)
- [ ] Silnik transakcyjny: plan → walidacja → wykonanie → rollback (faza 1)
- [ ] Aktualizacje delta, skrypty Rhai/WASM (faza 3)

## Wymagania

- Rust stable (pinowana wersja w `rust-toolchain.toml`)
- Windows 10/11 (x64 lub ARM64) — MSVC Build Tools z Windows SDK
- `cargo-deny` i `cargo-nextest` (lokalnie opcjonalne, w CI wymagane)

## Szybki start

```sh
cargo xtask check      # fmt + check + clippy (szybka pętla)
cargo xtask ci         # wszystko co robi CI
cargo run -p orki-cli -- wrap my-app.exe -o Setup.exe
cargo run -p orki-cli -- init --tauri src-tauri/tauri.conf.json
cargo run -p orki-cli -- check
cargo run -p orki-cli -- schema > orki.schema.json
```

## Warianty

`orki-stub` budowany jest z jedną z trzech wariantów feature:

| Wariant | Zawartość |
| --- | --- |
| `full` | iced + wgpu + shadery + Rhai + WASM (domyślny) |
| `lite` | iced + renderer programowy tiny-skia, bez shaderów/WASM |
| `headless` | tylko silnik, bez UI |

```sh
cargo xtask build --variant full --profile ci
cargo xtask size-budget --variant full --profile ci
cargo xtask audit-imports --variant full --profile ci
```

## Polityka zależności

100% czysty Rust: żadnych crate'ów `*-sys` kompilujących C/C++, żadnych `cc`/`cmake`/`bindgen`/`pkg-config` w drzewie (jedyny wyjątek: FFI do API systemu przez `windows`/`windows-sys`). Wszystkie zależności na najnowszych stabilnych wydaniach; wyjątki wymagają datowanego wpisu w `docs/dependency-waivers.md`. Egzekwowane w CI przez `cargo deny check` oraz audyt tabeli importów każdego zbudowanego stubu.

## CI

GitHub Actions (`.github/workflows/ci.yml`): lint, testy na `windows-latest` + `windows-11-arm`, buildy wszystkich trzech wariantów stubu z budżetami rozmiaru i audytem importów oraz e2e smoke. `nightly` pilnuje świeżości zależności; `release` buduje przy tagach `v*`.

## Licencja

Podwójna licencja MIT lub Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).
