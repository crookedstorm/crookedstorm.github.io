// World gateway renderer and input layer.
//
// The simulation lives in the Rust engine (see `src/engine/index.ts`). This
// module owns the canvas, keyboard input, and per-frame drawing. Each frame
// it gathers input, calls `engine.step(input)`, and paints from the returned
// `FrameState` plus the one-time `InitState` fetched at startup.
//
// Without JavaScript, the static links below the canvas still work; this
// module progressively enhances the page with the explorable world.

import { createEngine } from '../engine/index.js';
import type { FrameState, InitState, Input } from '../engine/index.js';

type Engine = Awaited<ReturnType<typeof createEngine>>;

const raccoonPalette: Record<string, string> = {
  B: '#06050d',
  D: '#3b324d',
  G: '#8f869f',
  L: '#d9d0e8',
  P: '#f0a6c8',
  W: '#f0e8ff',
};

const raccoonSprite = [
  '................',
  '....B......B....',
  '...BGB....BGB...',
  '..BGGGBBBBGGGB..',
  '..BGDDGGGGDDGB..',
  '.BGDDBGGGGBDDGB.',
  '.BGDBWBGGBWBDBG.',
  '.BGDDBBDDBBDDBG.',
  '..BGGGGLLGGGGB..',
  '...BGGLPPLGGB...',
  '....BGGLLGGB....',
  '...BBGGGGGGBB...',
  '..BGGBGBBGGGBB..',
  '.BGGBB....BBGGB.',
  '..BB........BB..',
  '................',
];

const spritePixelSize = 2;
const spriteWidth = raccoonSprite[0].length * spritePixelSize;
const spriteHeight = raccoonSprite.length * spritePixelSize;

const destinationColors: Record<string, string> = {
  '/about/': '#f7d774',
  '/blog/': '#98e6ff',
  '/projects/': '#b49cff',
};

const canvasElement =
  document.querySelector<HTMLCanvasElement>('#world-canvas');
const statusElement = document.querySelector<HTMLElement>('#world-status');

if (!canvasElement || !statusElement) {
  throw new Error('World gateway requires #world-canvas and #world-status.');
}

const canvas = canvasElement;
const statusOutput = statusElement;
const canvasContext = canvas.getContext('2d');

if (!canvasContext) {
  throw new Error('World gateway requires a Canvas 2D context.');
}

const context = canvasContext;

const pressedKeys = new Set<string>();

function readSeedFromUrl(): bigint {
  const params = new URLSearchParams(window.location.search);
  const raw = params.get('seed');

  if (raw === null || raw === '') {
    // Default to a fresh random seed so each visit without ?seed= is novel;
    // URLs with ?seed=N reproduce the same maze.
    return BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
  }

  // Accept decimals and hex prefixed with 0x.
  const asNumber = Number(raw);
  if (Number.isFinite(asNumber) && raw.trim() !== '') {
    return BigInt(Math.trunc(asNumber));
  }

  try {
    return BigInt(raw);
  } catch {
    return BigInt(0);
  }
}

function buildInput(): Input {
  return {
    up: pressedKeys.has('arrowup') || pressedKeys.has('w'),
    down: pressedKeys.has('arrowdown') || pressedKeys.has('s'),
    left: pressedKeys.has('arrowleft') || pressedKeys.has('a'),
    right: pressedKeys.has('arrowright') || pressedKeys.has('d'),
    enter: pressedKeys.has('enter'),
  };
}

function drawBackground(init: InitState): void {
  context.fillStyle = '#0b0918';
  context.fillRect(0, 0, canvas.width, canvas.height);

  // Open floor tiles are implicit; only walls are drawn as solid blocks.
  for (const wall of init.walls) {
    const x = wall.x * init.tileSize;
    const y = wall.y * init.tileSize;

    context.fillStyle = '#261a45';
    context.fillRect(x, y, init.tileSize, init.tileSize);

    context.fillStyle = '#3d2a6f';
    context.fillRect(x + 3, y + 3, init.tileSize - 6, init.tileSize - 6);
  }

  // Faint floor grid so open space still reads as a tile world.
  context.strokeStyle = '#20183f';
  context.lineWidth = 1;
  for (let x = 0; x <= canvas.width; x += init.tileSize) {
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
  }
  for (let y = 0; y <= canvas.height; y += init.tileSize) {
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }
}

function drawCamp(init: InitState): void {
  const campX = init.camp.x * init.tileSize + init.tileSize / 2;
  const campY = init.camp.y * init.tileSize + init.tileSize / 2;

  context.fillStyle = '#f07f5b';
  context.fillRect(campX - 8, campY - 8, 16, 16);

  context.fillStyle = '#f7d774';
  context.fillRect(campX - 4, campY - 14, 8, 8);
}

function drawDestinations(init: InitState): void {
  for (const destination of init.destinations) {
    const x = destination.x * init.tileSize;
    const y = destination.y * init.tileSize;

    context.fillStyle = '#06050d';
    context.fillRect(x - 18, y - 18, 68, 44);

    context.fillStyle = destinationColors[destination.href] ?? '#f7d774';
    context.fillRect(x - 14, y - 14, 60, 36);

    context.fillStyle = '#151029';
    context.font = '700 12px Courier New, monospace';
    // Short label: the section name, uppercased first word.
    context.fillText(
      destination.label.toUpperCase().split(' ')[0],
      x - 8,
      y + 7,
    );
  }
}

function drawTreats(init: InitState, frame: FrameState): void {
  // Drawn from the live treat list each frame so collected treats vanish.
  for (const treat of frame.treats) {
    const x = treat.x * init.tileSize;
    const y = treat.y * init.tileSize;

    context.fillStyle = '#06050d';
    context.fillRect(x + 10, y + 10, 12, 12);

    context.fillStyle = '#f7d774';
    context.fillRect(x + 11, y + 8, 10, 10);

    context.fillStyle = '#f0a6c8';
    context.fillRect(x + 14, y + 11, 4, 4);
  }
}

function drawPixelSprite(sprite: string[], x: number, y: number): void {
  for (const [rowIndex, row] of sprite.entries()) {
    for (const [columnIndex, pixel] of Array.from(row).entries()) {
      const color = raccoonPalette[pixel];
      if (!color) {
        continue;
      }
      context.fillStyle = color;
      context.fillRect(
        x + columnIndex * spritePixelSize,
        y + rowIndex * spritePixelSize,
        spritePixelSize,
        spritePixelSize,
      );
    }
  }
}

function drawPlayerShadow(
  sprite: string[],
  x: number,
  y: number,
  offsetX: number,
  offsetY: number,
): void {
  context.fillStyle = '#06050d';
  for (const [rowIndex, row] of sprite.entries()) {
    for (const [columnIndex, pixel] of Array.from(row).entries()) {
      if (!raccoonPalette[pixel]) {
        continue;
      }
      context.fillRect(
        x + columnIndex * spritePixelSize + offsetX,
        y + rowIndex * spritePixelSize + offsetY,
        spritePixelSize,
        spritePixelSize,
      );
    }
  }
}

function drawPlayer(frame: FrameState): void {
  const spriteX = frame.playerX - spriteWidth / 2;
  const spriteY = frame.playerY - spriteHeight / 2;

  drawPlayerShadow(raccoonSprite, spriteX, spriteY, 3, 3);
  drawPixelSprite(raccoonSprite, spriteX, spriteY);
}

function draw(init: InitState, frame: FrameState): void {
  drawBackground(init);
  drawTreats(init, frame);
  drawCamp(init);
  drawDestinations(init);
  drawPlayer(frame);
}

function handleNavigation(frame: FrameState): void {
  if (!frame.pendingNavigation) {
    return;
  }
  // Engine requested a page transition because Enter was pressed while
  // standing on a destination.
  window.location.href = frame.pendingNavigation;
}

async function bootstrap(): Promise<void> {
  const seed = readSeedFromUrl();
  const engine: Engine = await createEngine(seed);
  const init = engine.init_state() as unknown as InitState;

  // Size the canvas to the generated maze so the world always fits exactly.
  canvas.width = init.width * init.tileSize;
  canvas.height = init.height * init.tileSize;

  // Render an initial frame so the page is not blank before the first rAF.
  statusOutput.textContent = 'Booting world…';

  function tick(): void {
    const input = buildInput();
    const frame = engine.step(input) as unknown as FrameState;

    statusOutput.textContent = frame.status;
    draw(init, frame);
    handleNavigation(frame);

    window.requestAnimationFrame(tick);
  }

  window.requestAnimationFrame(tick);
}

window.addEventListener('keydown', (event) => {
  const key = event.key.toLowerCase();

  if (
    [
      'arrowup',
      'arrowdown',
      'arrowleft',
      'arrowright',
      'w',
      'a',
      's',
      'd',
      'enter',
    ].includes(key)
  ) {
    event.preventDefault();
    pressedKeys.add(key);
  }
});

window.addEventListener('keyup', (event) => {
  pressedKeys.delete(event.key.toLowerCase());
});

void bootstrap();
