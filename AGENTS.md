# AGENTS.md

## Validation

Run the checks related to files changed during a turn before reporting completion.

For Astro, TypeScript, CSS, or content changes under `src/`, run:

```bash
npm run format:check
npm run lint
npm run build
```

For Rust engine changes under `engine/`, run:

```bash
npm run engine:fmt
npm run engine:clippy
npm run engine:test
```

If the change crosses the Rust↔TypeScript boundary — for example `engine/src/`, `src/engine/`, or the `/world/` integration in `src/scripts/world.ts` — run the engine checks and the full site checks:

```bash
npm run engine:fmt
npm run engine:clippy
npm run engine:test
npm run format:check
npm run lint
npm run build
```

For dependency or tool configuration changes, run the same full set.

For GitHub Actions workflow changes under `.github/workflows/`, run:

```bash
actionlint
```

For changes limited to planning or documentation, run formatting checks when practical:

```bash
npm run format:check
```

If a check cannot be run, report why and include the command that should be run later.

## Project notes

- `public/` is source input for Astro and should be committed.
- `dist/`, `.astro/`, `node_modules/`, and `engine/pkg/` are generated and should not be committed.
- The previous generated site is preserved in `archive/legacy-static-site/`.
- The browsable temporary copy is in `public/legacy/` and is intentionally excluded from formatting and linting.
- Keep `CNAME` at the repository root for GitHub Pages. Keep `public/CNAME` so Astro copies it into `dist/`.
- For `/world/`, Rust owns simulation state and procedural generation. TypeScript owns canvas rendering, DOM updates, keyboard listeners, and browser navigation.
- Keep the Rust↔TypeScript world contract in sync across `engine/src/`, `src/engine/index.ts`, and `src/scripts/world.ts`.
- Runtime world artwork lives under `public/assets/world/` and should be committed. Keep gameplay meaning, values, placement, and collision in Rust; keep asset loading, animation timing, scaling, and drawing in TypeScript.
- Preserve nearest-neighbor rendering for pixel art by keeping canvas image smoothing disabled, including after canvas resize.
- Destination buildings use `96×80` transparent PNGs: the lower `96×64` maps to a `3×2` tile collision footprint and the upper 16 pixels allow roof or chimney overhang. Doors are centered on the bottom edge, entrance tiles sit directly below them, and TypeScript draws readable plaques above the roof rather than baking text into artwork.
- Building entrance navigation should trigger once when the player finishes stepping onto the entrance tile. It must not retrigger continuously while the player remains there or when browser history restores the page.
