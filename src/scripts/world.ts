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
import type {
  Direction,
  FrameState,
  InitState,
  Input,
  TreatKind,
} from '../engine/index.js';

type Engine = Awaited<ReturnType<typeof createEngine>>;
type TreatImages = Record<TreatKind, HTMLImageElement>;
type DestinationImages = Partial<Record<string, HTMLImageElement>>;

type Camera = {
  x: number;
  y: number;
  width: number;
  height: number;
};

const minViewportTilesWide = 11;
const minViewportTilesHigh = 9;
const maxViewportTilesWide = 23;
const maxViewportTilesHigh = 15;
const viewportHeightRatio = 0.55;

type Facing = 'down' | 'left' | 'right' | 'up';

const raccoonSpriteSheetUrl = '/assets/world/raccoon.png';
const raccoonFrameWidth = 24;
const raccoonFrameHeight = 24;
const raccoonFramesPerDirection = 3;
const raccoonScale = 1.5;
const raccoonDrawWidth = raccoonFrameWidth * raccoonScale;
const raccoonDrawHeight = raccoonFrameHeight * raccoonScale;
const raccoonAnimationFrameDuration = 5;

const campfireSpriteSheetUrl = '/assets/world/campfire.png';
const campfireFrameWidth = 24;
const campfireFrameHeight = 24;
const campfireFrameCount = 4;
const campfireScale = 1.5;
const campfireDrawWidth = campfireFrameWidth * campfireScale;
const campfireDrawHeight = campfireFrameHeight * campfireScale;
const campfireFrameDurationMs = 100;

const destinationSpriteUrls: Record<string, string> = {
  '/about/': '/assets/world/about.png',
  '/blog/': '/assets/world/field.png',
  '/projects/': '/assets/world/projects.png',
};

const treatSpriteUrls: Record<TreatKind, string> = {
  cheeseburger: '/assets/world/cheeseburger.png',
  snail: '/assets/world/snail.png',
  frog: '/assets/world/frog.png',
  banana: '/assets/world/banana.png',
  cherries: '/assets/world/cherries.png',
  berries: '/assets/world/berries.png',
  apple: '/assets/world/apple.png',
};

const raccoonRows: Record<Exclude<Facing, 'left' | 'right'> | 'side', number> =
  {
    down: 0,
    side: 1,
    up: 2,
  };

let lastFacing: Facing = 'down';
let raccoonAnimationTicks = 0;

const destinationColors: Record<string, string> = {
  '/about/': '#f7d774',
  '/blog/': '#98e6ff',
  '/projects/': '#b49cff',
};

const canvasElement =
  document.querySelector<HTMLCanvasElement>('#world-canvas');
const statusMessageElement = document.querySelector<HTMLElement>(
  '#world-status-message',
);
const scoreElement = document.querySelector<HTMLElement>('#world-score');

if (!canvasElement || !statusMessageElement || !scoreElement) {
  throw new Error(
    'World gateway requires #world-canvas, #world-status-message, and #world-score.',
  );
}

const canvas = canvasElement;
const statusOutput = statusMessageElement;
const scoreOutput = scoreElement;
const canvasContext = canvas.getContext('2d');

if (!canvasContext) {
  throw new Error('World gateway requires a Canvas 2D context.');
}

const context = canvasContext;
context.imageSmoothingEnabled = false;

const pressedKeys = new Set<string>();
const directionPressOrder: Direction[] = [];

function keyToDirection(key: string): Direction | null {
  if (key === 'arrowup' || key === 'w') {
    return 'up';
  }
  if (key === 'arrowdown' || key === 's') {
    return 'down';
  }
  if (key === 'arrowleft' || key === 'a') {
    return 'left';
  }
  if (key === 'arrowright' || key === 'd') {
    return 'right';
  }
  return null;
}

function rememberDirectionPress(direction: Direction): void {
  removeDirectionPress(direction);
  directionPressOrder.push(direction);
}

function removeDirectionPress(direction: Direction): void {
  const index = directionPressOrder.indexOf(direction);
  if (index === -1) {
    return;
  }
  directionPressOrder.splice(index, 1);
}

function clearPressedKeys(): void {
  pressedKeys.clear();
  directionPressOrder.length = 0;
}

function isDirectionHeld(direction: Direction): boolean {
  if (direction === 'up') {
    return pressedKeys.has('arrowup') || pressedKeys.has('w');
  }
  if (direction === 'down') {
    return pressedKeys.has('arrowdown') || pressedKeys.has('s');
  }
  if (direction === 'left') {
    return pressedKeys.has('arrowleft') || pressedKeys.has('a');
  }
  return pressedKeys.has('arrowright') || pressedKeys.has('d');
}

function currentPreferredDirection(): Direction | null {
  for (let index = directionPressOrder.length - 1; index >= 0; index -= 1) {
    const direction = directionPressOrder[index];
    if (isDirectionHeld(direction)) {
      return direction;
    }
  }
  return null;
}

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
  const input: Input = {
    up: pressedKeys.has('arrowup') || pressedKeys.has('w'),
    down: pressedKeys.has('arrowdown') || pressedKeys.has('s'),
    left: pressedKeys.has('arrowleft') || pressedKeys.has('a'),
    right: pressedKeys.has('arrowright') || pressedKeys.has('d'),
    preferredDirection: currentPreferredDirection(),
  };

  return input;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

function clampOddTileCount(
  value: number,
  minimum: number,
  maximum: number,
): number {
  const clampedValue = clamp(value, minimum, maximum);

  if (clampedValue % 2 === 1) {
    return clampedValue;
  }
  if (clampedValue === minimum) {
    return Math.min(clampedValue + 1, maximum);
  }
  return clampedValue - 1;
}

function resizeViewport(init: InitState): void {
  const parentWidth = canvas.parentElement?.clientWidth ?? canvas.clientWidth;
  const widthBudget = Math.floor(parentWidth / init.tileSize);
  const heightBudget = Math.floor(
    (window.innerHeight * viewportHeightRatio) / init.tileSize,
  );

  const tilesWide = clampOddTileCount(
    widthBudget,
    minViewportTilesWide,
    Math.min(maxViewportTilesWide, init.width),
  );
  const tilesHigh = clampOddTileCount(
    heightBudget,
    minViewportTilesHigh,
    Math.min(maxViewportTilesHigh, init.height),
  );

  canvas.width = tilesWide * init.tileSize;
  canvas.height = tilesHigh * init.tileSize;
  context.imageSmoothingEnabled = false;
}

function buildCamera(init: InitState, frame: FrameState): Camera {
  const worldWidth = init.width * init.tileSize;
  const worldHeight = init.height * init.tileSize;
  const maxX = Math.max(0, worldWidth - canvas.width);
  const maxY = Math.max(0, worldHeight - canvas.height);

  return {
    x: clamp(frame.playerX - canvas.width / 2, 0, maxX),
    y: clamp(frame.playerY - canvas.height / 2, 0, maxY),
    width: canvas.width,
    height: canvas.height,
  };
}

function toScreenX(worldX: number, camera: Camera): number {
  return worldX - camera.x;
}

function toScreenY(worldY: number, camera: Camera): number {
  return worldY - camera.y;
}

function drawBackground(init: InitState, camera: Camera): void {
  context.fillStyle = '#0b0918';
  context.fillRect(0, 0, canvas.width, canvas.height);

  // Open floor tiles are implicit; only walls are drawn as solid blocks.
  for (const wall of init.walls) {
    const worldX = wall.x * init.tileSize;
    const worldY = wall.y * init.tileSize;
    const x = toScreenX(worldX, camera);
    const y = toScreenY(worldY, camera);

    if (
      x + init.tileSize < 0 ||
      y + init.tileSize < 0 ||
      x > camera.width ||
      y > camera.height
    ) {
      continue;
    }

    context.fillStyle = '#261a45';
    context.fillRect(x, y, init.tileSize, init.tileSize);

    context.fillStyle = '#3d2a6f';
    context.fillRect(x + 3, y + 3, init.tileSize - 6, init.tileSize - 6);
  }

  // Faint floor grid so open space still reads as a tile world.
  context.strokeStyle = '#20183f';
  context.lineWidth = 1;

  const firstGridX = Math.floor(camera.x / init.tileSize) * init.tileSize;
  for (
    let worldX = firstGridX;
    worldX <= camera.x + camera.width;
    worldX += init.tileSize
  ) {
    const x = toScreenX(worldX, camera);
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
  }

  const firstGridY = Math.floor(camera.y / init.tileSize) * init.tileSize;
  for (
    let worldY = firstGridY;
    worldY <= camera.y + camera.height;
    worldY += init.tileSize
  ) {
    const y = toScreenY(worldY, camera);
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }
}

function drawCamp(
  init: InitState,
  camera: Camera,
  campfireImage: HTMLImageElement,
  elapsedTimeMs: number,
): void {
  const campX = toScreenX(
    init.camp.x * init.tileSize + init.tileSize / 2,
    camera,
  );
  const campY = toScreenY(
    init.camp.y * init.tileSize + init.tileSize / 2,
    camera,
  );
  const frame =
    Math.floor(elapsedTimeMs / campfireFrameDurationMs) % campfireFrameCount;
  const sourceX = frame * campfireFrameWidth;
  const drawX = campX - campfireDrawWidth / 2;
  const drawY = campY - campfireDrawHeight / 2;

  context.drawImage(
    campfireImage,
    sourceX,
    0,
    campfireFrameWidth,
    campfireFrameHeight,
    drawX,
    drawY,
    campfireDrawWidth,
    campfireDrawHeight,
  );
}

function drawDestinationPlaque(
  centerX: number,
  bottomY: number,
  label: string,
): void {
  const plaqueLabel = label.toUpperCase().split(' ')[0];

  context.font = '700 12px Courier New, monospace';
  const textWidth = context.measureText(plaqueLabel).width;
  const plaqueWidth = textWidth + 12;
  const plaqueHeight = 18;
  const plaqueX = centerX - plaqueWidth / 2;
  const plaqueY = bottomY - plaqueHeight;

  context.fillStyle = '#06050d';
  context.fillRect(plaqueX, plaqueY, plaqueWidth, plaqueHeight);

  context.fillStyle = '#f7d774';
  context.fillText(plaqueLabel, plaqueX + 6, plaqueY + 13);
}

function drawDestinations(
  init: InitState,
  camera: Camera,
  destinationImages: DestinationImages,
): void {
  for (const destination of init.destinations) {
    const x = toScreenX(destination.x * init.tileSize, camera);
    const y = toScreenY(destination.y * init.tileSize, camera);
    const image = destinationImages[destination.href];

    if (image) {
      const buildingX = x + init.tileSize / 2 - image.naturalWidth / 2;
      const buildingY = y + init.tileSize - image.naturalHeight;

      context.drawImage(image, buildingX, buildingY);
      drawDestinationPlaque(
        x + init.tileSize / 2,
        buildingY - 2,
        destination.label,
      );
      continue;
    }

    context.fillStyle = '#06050d';
    context.fillRect(x - 18, y - 18, 68, 44);

    context.fillStyle = destinationColors[destination.href] ?? '#f7d774';
    context.fillRect(x - 14, y - 14, 60, 36);

    context.fillStyle = '#151029';
    context.font = '700 12px Courier New, monospace';
    context.fillText(
      destination.label.toUpperCase().split(' ')[0],
      x - 8,
      y + 7,
    );
  }
}

function drawTreats(
  init: InitState,
  frame: FrameState,
  camera: Camera,
  treatImages: TreatImages,
): void {
  // Drawn from the live treat list each frame so collected treats vanish.
  for (const treat of frame.treats) {
    const image = treatImages[treat.kind];
    const centerX = toScreenX(
      treat.x * init.tileSize + init.tileSize / 2,
      camera,
    );
    const centerY = toScreenY(
      treat.y * init.tileSize + init.tileSize / 2,
      camera,
    );
    const drawX = centerX - image.naturalWidth / 2;
    const drawY = centerY - image.naturalHeight / 2;

    context.fillStyle = '#06050d';
    context.fillRect(centerX - 8, centerY + 9, 16, 4);
    context.drawImage(image, drawX, drawY);
  }
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`Unable to load image: ${url}`));
    image.src = url;
  });
}

async function loadDestinationImages(): Promise<DestinationImages> {
  const entries = await Promise.all(
    Object.entries(destinationSpriteUrls).map(async ([href, url]) => {
      const image = await loadImage(url);
      return [href, image] as const;
    }),
  );

  return Object.fromEntries(entries) as DestinationImages;
}

async function loadTreatImages(): Promise<TreatImages> {
  const entries = await Promise.all(
    Object.entries(treatSpriteUrls).map(async ([kind, url]) => {
      const image = await loadImage(url);
      return [kind, image] as const;
    }),
  );

  return Object.fromEntries(entries) as TreatImages;
}

function facingFromFrame(frame: FrameState): Facing {
  if (frame.playerVx < 0) {
    return 'left';
  }
  if (frame.playerVx > 0) {
    return 'right';
  }
  if (frame.playerVy < 0) {
    return 'up';
  }
  if (frame.playerVy > 0) {
    return 'down';
  }
  return lastFacing;
}

function isPlayerMoving(frame: FrameState): boolean {
  return frame.playerVx !== 0 || frame.playerVy !== 0;
}

function drawPlayerShadow(x: number, y: number): void {
  context.fillStyle = '#06050d';
  context.fillRect(x + 4, y + raccoonDrawHeight - 8, raccoonDrawWidth - 8, 8);
}

function drawPlayer(
  frame: FrameState,
  camera: Camera,
  raccoonImage: HTMLImageElement,
): void {
  const facing = facingFromFrame(frame);
  const moving = isPlayerMoving(frame);

  if (moving) {
    lastFacing = facing;
    raccoonAnimationTicks += 1;
  } else {
    raccoonAnimationTicks = 0;
  }

  const row =
    facing === 'up'
      ? raccoonRows.up
      : facing === 'down'
        ? raccoonRows.down
        : raccoonRows.side;
  const column = moving
    ? Math.floor(raccoonAnimationTicks / raccoonAnimationFrameDuration) %
      raccoonFramesPerDirection
    : 1;
  const sourceX = column * raccoonFrameWidth;
  const sourceY = row * raccoonFrameHeight;
  const spriteX = toScreenX(frame.playerX, camera) - raccoonDrawWidth / 2;
  const spriteY = toScreenY(frame.playerY, camera) - raccoonDrawHeight / 2;

  drawPlayerShadow(spriteX, spriteY);

  if (facing === 'left') {
    context.save();
    context.translate(spriteX + raccoonDrawWidth, spriteY);
    context.scale(-1, 1);
    context.drawImage(
      raccoonImage,
      sourceX,
      sourceY,
      raccoonFrameWidth,
      raccoonFrameHeight,
      0,
      0,
      raccoonDrawWidth,
      raccoonDrawHeight,
    );
    context.restore();
    return;
  }

  context.drawImage(
    raccoonImage,
    sourceX,
    sourceY,
    raccoonFrameWidth,
    raccoonFrameHeight,
    spriteX,
    spriteY,
    raccoonDrawWidth,
    raccoonDrawHeight,
  );
}

function draw(
  init: InitState,
  frame: FrameState,
  raccoonImage: HTMLImageElement,
  campfireImage: HTMLImageElement,
  destinationImages: DestinationImages,
  treatImages: TreatImages,
  elapsedTimeMs: number,
): void {
  const camera = buildCamera(init, frame);

  drawBackground(init, camera);
  drawTreats(init, frame, camera, treatImages);
  drawCamp(init, camera, campfireImage, elapsedTimeMs);
  drawDestinations(init, camera, destinationImages);
  drawPlayer(frame, camera, raccoonImage);
}

function handleNavigation(frame: FrameState): void {
  if (!frame.pendingNavigation) {
    return;
  }
  // Engine requested a page transition when the player arrived at a door.
  window.location.href = frame.pendingNavigation;
}

async function bootstrap(): Promise<void> {
  const seed = readSeedFromUrl();
  const [engine, raccoonImage, campfireImage, destinationImages, treatImages]: [
    Engine,
    HTMLImageElement,
    HTMLImageElement,
    DestinationImages,
    TreatImages,
  ] = await Promise.all([
    createEngine(seed),
    loadImage(raccoonSpriteSheetUrl),
    loadImage(campfireSpriteSheetUrl),
    loadDestinationImages(),
    loadTreatImages(),
  ]);
  const init = engine.init_state() as unknown as InitState;

  // Size the canvas as a responsive camera viewport into the generated maze.
  resizeViewport(init);
  window.addEventListener('resize', () => resizeViewport(init));

  // Render an initial frame so the page is not blank before the first rAF.
  statusOutput.textContent = 'Booting world…';
  scoreOutput.textContent = 'Score: 0';

  function tick(elapsedTimeMs: number): void {
    const input = buildInput();
    const frame = engine.step(input) as unknown as FrameState;

    statusOutput.textContent = frame.status;
    scoreOutput.textContent = `Score: ${frame.score}`;
    draw(
      init,
      frame,
      raccoonImage,
      campfireImage,
      destinationImages,
      treatImages,
      elapsedTimeMs,
    );
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
    ].includes(key)
  ) {
    event.preventDefault();

    pressedKeys.add(key);

    const direction = keyToDirection(key);
    if (direction) {
      rememberDirectionPress(direction);
    }
  }
});

window.addEventListener('keyup', (event) => {
  const key = event.key.toLowerCase();
  pressedKeys.delete(key);

  const direction = keyToDirection(key);
  if (!direction) {
    return;
  }
  if (!isDirectionHeld(direction)) {
    removeDirectionPress(direction);
  }
});

// A keyup can be lost while the browser is navigating away. Clear state before
// page caching so history restoration cannot resume movement from a stale key.
window.addEventListener('blur', clearPressedKeys);
window.addEventListener('pagehide', clearPressedKeys);

void bootstrap();
