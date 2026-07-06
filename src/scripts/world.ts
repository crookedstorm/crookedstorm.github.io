type Destination = {
  color: string;
  description: string;
  href: string;
  label: string;
  x: number;
  y: number;
};

type Player = {
  velocityX: number;
  velocityY: number;
  x: number;
  y: number;
};

type Treat = {
  collected: boolean;
  x: number;
  y: number;
};

const tileSize = 32;
const playerSize = 32;
const movementStep = 4;
const spritePixelSize = 2;
const acceleration = 0.6;
const maxSpeed = 4;
const friction = 0.85;
const treatCount = 8;
const treatValue = 50;
const campPosition = {
  x: 15 * tileSize,
  y: 4 * tileSize,
};

const mazeRows = [
  '##############################',
  '#............##..............#',
  '#............##..............#',
  '#............................#',
  '#............................#',
  '#....######........######....#',
  '#.........#........#.........#',
  '#.........#........#.........#',
  '#............................#',
  '#..####.................####.#',
  '#.....#.................#....#',
  '#.....#.....######......#....#',
  '#...........#....#...........#',
  '#...........#....#...........#',
  '#............................#',
  '#....######..........######..#',
  '#............................#',
  '#............................#',
  '#.......####......####.......#',
  '#............................#',
  '#..............##............#',
  '#..............##............#',
  '##############################',
];

const destinations: Destination[] = [
  {
    color: '#f7d774',
    description: 'About Brooke',
    href: '/about/',
    label: 'ABOUT',
    x: 7,
    y: 17,
  },
  {
    color: '#98e6ff',
    description: 'Field notes and posts',
    href: '/blog/',
    label: 'BLOG',
    x: 23,
    y: 17,
  },
  {
    color: '#b49cff',
    description: 'Projects and experiments',
    href: '/projects/',
    label: 'PROJECTS',
    x: 15,
    y: 19,
  },
];

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
const player: Player = {
  velocityX: 0,
  velocityY: 0,
  x: campPosition.x,
  y: campPosition.y,
};

const pressedKeys = new Set<string>();
let activeDestination: Destination | null = null;
let score = 0;
let lastTreatMessageFrames = 0;

function tileToPixel(tileCoordinate: number): number {
  return tileCoordinate * tileSize;
}

function pixelToTile(pixelCoordinate: number): number {
  return Math.floor(pixelCoordinate / tileSize);
}

function isWallTile(tileX: number, tileY: number): boolean {
  const row = mazeRows[tileY];

  if (!row) {
    return true;
  }

  return row[tileX] === '#';
}

function isNearTile(
  tileX: number,
  tileY: number,
  targetX: number,
  targetY: number,
): boolean {
  return Math.abs(tileX - targetX) <= 1 && Math.abs(tileY - targetY) <= 1;
}

function isBlockedForTreat(
  tileX: number,
  tileY: number,
  treats: Treat[],
): boolean {
  if (isWallTile(tileX, tileY)) {
    return true;
  }

  const campTileX = pixelToTile(campPosition.x);
  const campTileY = pixelToTile(campPosition.y);

  if (isNearTile(tileX, tileY, campTileX, campTileY)) {
    return true;
  }

  const isNearDestination = destinations.some((destination) =>
    isNearTile(tileX, tileY, destination.x, destination.y),
  );

  if (isNearDestination) {
    return true;
  }

  return treats.some((treat) => isNearTile(tileX, tileY, treat.x, treat.y));
}

function createRandomTreats(): Treat[] {
  const randomTreats: Treat[] = [];
  const candidateTiles: Treat[] = [];

  mazeRows.forEach((row, rowIndex) => {
    Array.from(row).forEach((tile, columnIndex) => {
      if (tile === '.') {
        candidateTiles.push({ collected: false, x: columnIndex, y: rowIndex });
      }
    });
  });

  while (randomTreats.length < treatCount && candidateTiles.length > 0) {
    const candidateIndex = Math.floor(Math.random() * candidateTiles.length);
    const [candidate] = candidateTiles.splice(candidateIndex, 1);

    if (
      !candidate ||
      isBlockedForTreat(candidate.x, candidate.y, randomTreats)
    ) {
      continue;
    }

    randomTreats.push(candidate);
  }

  return randomTreats;
}

const treats = createRandomTreats();

function wouldCollideWithWall(x: number, y: number): boolean {
  const halfPlayer = playerSize / 2 - 4;
  const left = pixelToTile(x - halfPlayer);
  const right = pixelToTile(x + halfPlayer);
  const top = pixelToTile(y - halfPlayer);
  const bottom = pixelToTile(y + halfPlayer);

  return (
    isWallTile(left, top) ||
    isWallTile(right, top) ||
    isWallTile(left, bottom) ||
    isWallTile(right, bottom)
  );
}

function drawBackground(): void {
  context.fillStyle = '#0b0918';
  context.fillRect(0, 0, canvas.width, canvas.height);

  mazeRows.forEach((row, rowIndex) => {
    Array.from(row).forEach((tile, columnIndex) => {
      const x = columnIndex * tileSize;
      const y = rowIndex * tileSize;

      if (tile === '#') {
        context.fillStyle = '#261a45';
        context.fillRect(x, y, tileSize, tileSize);

        context.fillStyle = '#3d2a6f';
        context.fillRect(x + 3, y + 3, tileSize - 6, tileSize - 6);
        return;
      }

      context.fillStyle = '#100d20';
      context.fillRect(x, y, tileSize, tileSize);

      context.strokeStyle = '#20183f';
      context.strokeRect(x, y, tileSize, tileSize);
    });
  });
}

function drawCamp(): void {
  const campX = campPosition.x;
  const campY = campPosition.y;

  context.fillStyle = '#f07f5b';
  context.fillRect(campX - 8, campY - 8, 16, 16);

  context.fillStyle = '#f7d774';
  context.fillRect(campX - 4, campY - 14, 8, 8);
}

function drawDestinations(): void {
  for (const destination of destinations) {
    const x = tileToPixel(destination.x);
    const y = tileToPixel(destination.y);

    context.fillStyle = '#06050d';
    context.fillRect(x - 18, y - 18, 68, 44);

    context.fillStyle = destination.color;
    context.fillRect(x - 14, y - 14, 60, 36);

    context.fillStyle = '#151029';
    context.font = '700 12px Courier New, monospace';
    context.fillText(destination.label, x - 8, y + 7);
  }
}

function drawTreats(): void {
  for (const treat of treats) {
    if (treat.collected) {
      continue;
    }

    const x = tileToPixel(treat.x);
    const y = tileToPixel(treat.y);

    context.fillStyle = '#06050d';
    context.fillRect(x + 10, y + 10, 12, 12);

    context.fillStyle = '#f7d774';
    context.fillRect(x + 11, y + 8, 10, 10);

    context.fillStyle = '#f0a6c8';
    context.fillRect(x + 14, y + 11, 4, 4);
  }
}

function drawPixelSprite(sprite: string[], x: number, y: number): void {
  sprite.forEach((row, rowIndex) => {
    Array.from(row).forEach((pixel, columnIndex) => {
      const color = raccoonPalette[pixel];

      if (!color) {
        return;
      }

      context.fillStyle = color;
      context.fillRect(
        x + columnIndex * spritePixelSize,
        y + rowIndex * spritePixelSize,
        spritePixelSize,
        spritePixelSize,
      );
    });
  });
}

function drawPlayerShadow(
  sprite: string[],
  x: number,
  y: number,
  offsetX: number,
  offsetY: number,
): void {
  context.fillStyle = '#06050d';

  sprite.forEach((row, rowIndex) => {
    Array.from(row).forEach((pixel, columnIndex) => {
      if (!raccoonPalette[pixel]) {
        return;
      }

      context.fillRect(
        x + columnIndex * spritePixelSize + offsetX,
        y + rowIndex * spritePixelSize + offsetY,
        spritePixelSize,
        spritePixelSize,
      );
    });
  });
}

function drawPlayer(): void {
  const spriteWidth = raccoonSprite[0].length * spritePixelSize;
  const spriteHeight = raccoonSprite.length * spritePixelSize;
  const spriteX = player.x - spriteWidth / 2;
  const spriteY = player.y - spriteHeight / 2;

  drawPlayerShadow(raccoonSprite, spriteX, spriteY, 3, 3);
  drawPixelSprite(raccoonSprite, spriteX, spriteY);
}

function getDestinationAtPlayer(): Destination | null {
  return (
    destinations.find((destination) => {
      const destinationX = tileToPixel(destination.x);
      const destinationY = tileToPixel(destination.y);
      const distanceX = Math.abs(player.x - destinationX);
      const distanceY = Math.abs(player.y - destinationY);

      return distanceX < tileSize && distanceY < tileSize;
    }) ?? null
  );
}

function isPlayerNearCamp(): boolean {
  const distanceX = Math.abs(player.x - campPosition.x);
  const distanceY = Math.abs(player.y - campPosition.y);

  return distanceX < tileSize && distanceY < tileSize;
}

function collectTreats(): void {
  for (const treat of treats) {
    if (treat.collected) {
      continue;
    }

    const treatX = tileToPixel(treat.x) + tileSize / 2;
    const treatY = tileToPixel(treat.y) + tileSize / 2;
    const distanceX = Math.abs(player.x - treatX);
    const distanceY = Math.abs(player.y - treatY);

    if (distanceX < tileSize / 2 && distanceY < tileSize / 2) {
      treat.collected = true;
      score += treatValue;
      lastTreatMessageFrames = 90;
    }
  }
}

function updateStatus(): void {
  activeDestination = getDestinationAtPlayer();

  if (lastTreatMessageFrames > 0) {
    statusOutput.textContent = `Treat acquired. Score: ${score}`;
    lastTreatMessageFrames -= 1;
    return;
  }

  if (activeDestination) {
    statusOutput.textContent = `${activeDestination.description}. Press Enter to enter. Score: ${score}`;
    return;
  }

  if (isPlayerNearCamp()) {
    statusOutput.textContent = `Standing at camp. Score: ${score}`;
    return;
  }

  statusOutput.textContent = `Adventuring… Score: ${score}`;
}

function applyMovementIntent(): void {
  let intentX = 0;
  let intentY = 0;

  if (pressedKeys.has('arrowup') || pressedKeys.has('w')) {
    intentY -= 1;
  }

  if (pressedKeys.has('arrowdown') || pressedKeys.has('s')) {
    intentY += 1;
  }

  if (pressedKeys.has('arrowleft') || pressedKeys.has('a')) {
    intentX -= 1;
  }

  if (pressedKeys.has('arrowright') || pressedKeys.has('d')) {
    intentX += 1;
  }

  if (intentX !== 0 && intentY !== 0) {
    const diagonalScale = Math.SQRT1_2;
    intentX *= diagonalScale;
    intentY *= diagonalScale;
  }

  player.velocityX += intentX * acceleration;
  player.velocityY += intentY * acceleration;

  const speed = Math.hypot(player.velocityX, player.velocityY);

  if (speed > maxSpeed) {
    const scale = maxSpeed / speed;
    player.velocityX *= scale;
    player.velocityY *= scale;
  }

  if (intentX === 0) {
    player.velocityX *= friction;
  }

  if (intentY === 0) {
    player.velocityY *= friction;
  }

  if (Math.abs(player.velocityX) < 0.1) {
    player.velocityX = 0;
  }

  if (Math.abs(player.velocityY) < 0.1) {
    player.velocityY = 0;
  }
}

function tryMovePlayer(deltaX: number, deltaY: number): void {
  const nextX = player.x + deltaX;
  const nextY = player.y + deltaY;

  if (!wouldCollideWithWall(nextX, player.y)) {
    player.x = nextX;
  } else {
    player.velocityX = 0;
  }

  if (!wouldCollideWithWall(player.x, nextY)) {
    player.y = nextY;
  } else {
    player.velocityY = 0;
  }
}

function movePlayer(): void {
  applyMovementIntent();
  tryMovePlayer(player.velocityX, player.velocityY);
}

function draw(): void {
  drawBackground();
  drawTreats();
  drawCamp();
  drawDestinations();
  drawPlayer();
}

function tick(): void {
  movePlayer();
  collectTreats();
  updateStatus();
  draw();
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
  }

  if (key === 'enter' && activeDestination) {
    window.location.href = activeDestination.href;
  }
});

window.addEventListener('keyup', (event) => {
  pressedKeys.delete(event.key.toLowerCase());
});

draw();
tick();
