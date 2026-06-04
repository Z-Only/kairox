---
name: skill-creator
description: Use when a user wants to create, update, validate, or package a Kairox skill based on SKILL.md.
version: 0.1.0
kairox:
  activation:
    mode: suggest
    keywords:
      - skill
      - skills
      - SKILL.md
      - create skill
      - update skill
      - custom skill
---

# Skill Creator

Use this skill to help users create or revise Kairox skills. A Kairox skill is a directory with a required `SKILL.md` file and optional supporting resources.

## Workflow

1. Determine the target scope:
   - User skill: `~/.config/kairox/skills/<skill-name>/SKILL.md`
   - Workspace skill: `<workspace>/.kairox/skills/<skill-name>/SKILL.md`
2. Pick a stable kebab-case directory name and use the same value for the frontmatter `name` unless the user has a strong reason to differ.
3. Write concise frontmatter with `name` and `description`. Add `version` and `kairox.activation` only when they help discovery.
4. Keep `SKILL.md` focused on the core procedure. Move long references, examples, schemas, or provider-specific details into `references/`.
5. Add `scripts/` only for deterministic helper code that the agent can run deliberately. Skills never execute bundled scripts automatically.
6. Declare tool permissions under `kairox.permissions` only as an informational summary; normal Kairox tool approval and sandbox policy still apply.
7. Validate the skill by rediscovering/listing skills and loading the detail view. For behavior-heavy skills, test with a realistic prompt that should trigger or use it.

## Minimal Template

```markdown
---
name: my-skill
description: Use when the user needs a precise description of the task this skill handles.
version: 0.1.0
kairox:
  activation:
    mode: suggest
    keywords:
      - keyword
  permissions:
    tools:
      - fs.read
    can_request_tools:
      - patch.apply
---

# My Skill

## Workflow

1. Do the first task-specific step.
2. Load only the relevant reference files when needed.
3. Validate the result with the smallest reliable check.
```

## Quality Bar

- The description must say when to use the skill, not just what the skill is.
- Prefer specific procedures over broad advice.
- Avoid restating general programming knowledge.
- Keep examples short and directly reusable.
- Do not add extra README, changelog, or installation documents unless the user explicitly asks for them.
