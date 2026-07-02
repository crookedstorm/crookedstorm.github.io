# AGENTS.md

## Validation

Run the checks related to files changed during a turn before reporting completion.

For Astro, TypeScript, CSS, or content changes under `src/`, run:

```bash
npm run format:check
npm run lint
npm run build
```

For dependency or tool configuration changes, run the same full set.

For changes limited to planning or documentation, run formatting checks when practical:

```bash
npm run format:check
```

If a check cannot be run, report why and include the command that should be run later.

## Project notes

- `public/` is source input for Astro and should be committed.
- `dist/`, `.astro/`, and `node_modules/` are generated and should not be committed.
- The previous generated site is preserved in `archive/legacy-static-site/`.
- The browsable temporary copy is in `public/legacy/` and is intentionally excluded from formatting and linting.
- Keep `CNAME` at the repository root for GitHub Pages. Keep `public/CNAME` so Astro copies it into `dist/`.
