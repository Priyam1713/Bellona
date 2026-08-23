---
name: doctrine-review
version: 1.0.0
description: Review a change against Bellona's Seven Laws before approval.
triggers: ["review", "law check", "doctrine"]
author: bellona-works
license: MIT
---

# Doctrine review

1. Read `BELLONA.md` and hold the Seven Laws in working memory.
2. For each Law, find the file:line in the diff that satisfies it.
3. Flag violations with severity:
   - **critical** — any bypass of `praetorium::custos`, any allow-by-default
     policy path, unsigned skill installation, audit-after-execution.
   - **major** — new vendor-cloud requirement (Law III), missing lineage on
     compaction (Law VI), benchmark claims without harness-controlled runs (Law VII).
   - **minor** — kernel growth without justification (Law I).
4. Output a verdict table: Law | Status | Evidence.
