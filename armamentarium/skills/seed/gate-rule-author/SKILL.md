---
name: gate-rule-author
version: 1.0.0
description: Draft fail-closed CEL rules for the Lex policy engine.
triggers: ["write policy", "new rule", "cel rule"]
author: bellona-works
license: MIT
---

# Authoring Lex rules

Attributes available under `attr.*`: `tool.name`, `effect.kind`,
`agent.id`, `owner.id`, `intent`, `target.uri`, `resource.kind`,
`page.url`, `page.host`, `file.path`, `cmd.line`, `mcp.server`.

Rules of engagement:

1. Default posture is deny — never write a catch-all allow.
2. Deny rules exist for *specific hazards*; broad denies starve the loop.
3. Irreversible effects (`file_write`, `shell_exec`, `mcp_call`) get
   `require_approval` unless scoped to an exact target prefix.
4. Test every rule against: matching request, near-miss request, malformed
   attributes (must refuse).
