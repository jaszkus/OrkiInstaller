# Architektura — skrót

Pełny dokument projektowy: `OrkiInstaller — dokument projektowy.md` (w katalogu głównym workspace'u).

## Komponenty

| Komponent | Rola |
| --- | --- |
| `orki` (CLI) | budowanie (`wrap`, `pack`), walidacja (`check`), schemat, inspekcja |
| `orki-stub` | runtime instalatora: GUI/silent/passive/uninstall/repair/modify |
| `orki-core` | manifest, planer, operacje, journal/rollback, wyrażenia, ścieżki |
| `orki-pack` | format `.orkipack`: sekcje, chunki, hashe BLAKE3, CRC, footer |
| `orki-ui` | aplikacja iced: strony, layouty, motywy |
| `orki-gfx` | ABI uniformów shaderów, presety, detekcja adaptera programowego |
| `orki-platform` | trait'y platformy + implementacja Windows |
| `orki-net` | pobieranie z wznawianiem (Windows: WinHTTP) |
| `orki-tauri` | import `tauri.conf.json`, generator `latest.json` |

## Format payloadu

`Setup.exe` = stub PE + dołączony overlay `.orkipack` (sekcje CHUNKS → MANIFEST → FOOTER). Footer o stałym rozmiarze 64 B z magic `ORKIPACK` pozwala czytać manifest od tyłu pliku. Każdy chunk ma BLAKE3 i CRC32; manifest ma własne CRC.

## Warianty stubu

`full` (iced+wgpu+shadery+Rhai+WASM), `lite` (iced+tiny-skia), `headless` (bez UI). Wybór feature flagami tego samego crate'a `bins/orki-stub`.

## Polityka zależności

R1: zero C/C++ (ban w `deny.toml`, audyt importów PE w `xtask`). R2: najnowsze wersje zależności. R3: wyjątki tylko w `docs/dependency-waivers.md`.
