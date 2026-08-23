# Armamentarium — the skill registry spec (v0.1)

Skills are **instructions, not capabilities**. A skill pack is a directory of
markdown with YAML frontmatter — the format proven interoperable across
Hermes, Claude Code and Superpowers ecosystems. Bellona reads foreign packs
via `external_dirs` without conversion; conversion happens only for signing.

## Layout

```
<skill-name>/
├── SKILL.md        # entry: frontmatter + instructions
└── (optional refs/ # reference docs the agent may read on demand)
```

## Frontmatter

| Field       | Required | Notes                                   |
|-------------|----------|-----------------------------------------|
| name        | yes      | kebab-case, unique in namespace         |
| version     | yes      | semver                                  |
| description | yes      | 16–280 chars; shown in lazy-load index  |
| triggers    | no       | phrases that should load the skill      |
| author      | recommended |                                      |
| license     | yes      | SPDX                                    |

## Loading discipline (token economy)

- Only `name`, `description`, `triggers` enter the system prompt index.
- Full body loads when triggered or explicitly invoked (`/skill-name`).
- Nothing executes from a pack — ever. Scripts referenced by skills are
  ordinary tools and pass through the Praetorian Gate like everything else.

## Distribution & trust

- Registry manifests validate against `registry.schema.json`.
- Published packs carry Sigstore-style witness signatures.
- Unsigned packs run in *advisory* mode: visible, watermarked, never auto-loaded.

## Foreign-host compatibility

The `compat.hosts` field advertises where a pack works unchanged:
`bellona`, `hermes`, `claude-code`, `codex`, `opencode`, `goose`.
Interop is a feature, not an afterthought.
