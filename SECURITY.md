# Security Policy

## Reporting

Email **security@bellona.local** (placeholder until the public inbox lands) or
open a GitHub security advisory. Please include reproduction steps and the
audit-chain rows involved. We aim to acknowledge within 72h.

## Scope

The Praetorian Gate (`praetorium`) is the security boundary. In scope:
bypass paths around `custos`, policy-evaluation escapes, ledger tampering,
identity forgery, SSRF shield evasion, sandbox escape via Castra drivers,
plugin capability leakage.

## Hardening rules we enforce in-repo

- No tool executes outside `praetorium::custos` — enforced by integration tests.
- Policies are fail-closed; broken rules refuse rather than open.
- Secrets never enter transcripts or traces (redaction hooks at trace + memory put).
- Web fetches re-validate every redirect hop against private-space targets.

## Disclosure timeline

fix → release with CVE → public write-up in docs/receipts.md.
