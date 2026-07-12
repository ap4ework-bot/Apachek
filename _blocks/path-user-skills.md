---
type: atom
kind: path
name: user-skills
template: ~/.claude/skills
expand_at: render
---

# Path atom — user-skills

Resolves to the user's `~/.claude/skills/` directory (skill definitions and their reference material, e.g. `architecture-rules/references/antipatterns.md`).

Used by agent manifests (`_manifests/*.toml`) to reference skill reference files without leaking the absolute path (with the maintainer's home `/Users/<user>/...`) into public artefacts under `_generated/`.

**Usage in manifests:**
```toml
[references]
extra = [
  "path:user-skills/architecture-rules/references/antipatterns.md",
  "path:user-skills/architecture-rules/references/duplication.md",
]
```

**Resolution:** the assembler detects the `path:user-skills/` prefix, looks up this atom in the registry, and emits an opaque DNA reference into the rendered `_generated/<agent>.md`. Same content-addressing semantics as `path-user-memory`, `path-user-rules`, and `path-user-hooks` — published artefact has DNA hashes, not paths. A reader with a local kit + registry resolves the DNA back to the file; a reader without the kit sees only the opaque hash.

**Expand timing:** `render` — substitution happens at `_assembler` time, before the `_generated/` markdown is written.

**Constructor Pattern:** one cube, one path. No code, no logic. Body bytes + frontmatter ARE the atom. Hash → DNA via standard registry pipeline.
