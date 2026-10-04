# Security Policy

## Supported versions

Only the latest `main` and the most recent tagged release receive security fixes.

## Reporting a vulnerability

Do not open a public issue for security problems.

Use GitHub private vulnerability reporting:
https://github.com/jaszkus/OrkiInstaller/security/advisories/new

Include a description, affected component (`orki-core`, `orki-pack`, `orki-stub`, ...), steps or a manifest to reproduce, and the expected vs actual behavior.

You can expect an initial response within 7 days.

## Scope notes

The trust model treats the installer author as trusted and all external input (network, CLI arguments, answer files, user paths) as untrusted. Payload integrity is enforced with BLAKE3 chunk hashes plus Ed25519 signatures; path handling rejects traversal, ADS streams, and reserved device names.
