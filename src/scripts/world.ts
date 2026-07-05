type Destination = {
  color: string;
  description: string;
  href: string;
  label: string;
  x: number;
  y: number;
};

type Player = {
  x: number;
  y: number;
};

const tileSize = 32;
const playerSize = 32;
const movementStep = 4;
const spritePixelSize = 2;

const destinations: Destination[] = [
  {
    color: '#f7d774',
    description: 'About Brooke',
    href: '/about/',
    label: 'ABOUT',
    x: 5,
    y: 4,
  },
  {
    color: '#98e6ff',
    description: 'Field notes and posts',
    href: '/blog/',
    label: 'BLOG',
    x: 14,
    y: 4,
  },
  {
    color: '#b49cff',
    description: 'Projects and experiments',
    href: '/projects/',
    label: 'PROJECTS',
    x: 10,
    y: 9,
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
  x: canvas.width / 2,
  y: canvas.height / 2,
};

const pressedKeys = new Set<string>();
let activeDestination: Destination | null = null;

function drawBackground(): void {
  context.fillStyle = '#0b0918';
  context.fillRect(0, 0, canvas.width, canvas.height);

  context.strokeStyle = '#20183f';
  context.lineWidth = 1;

  for (let x = 0; x <= canvas.width; x += tileSize) {
    context.beginPath();
    context.moveTo(x, 0);
    context.lineTo(x, canvas.height);
    context.stroke();
  }

  for (let y = 0; y <= canvas.height; y += tileSize) {
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }
}

function drawCamp(): void {
  const campX = canvas.width / 2;
  const campY = canvas.height / 2;

  context.fillStyle = '#f07f5b';
  context.fillRect(campX - 8, campY - 8, 16, 16);

  context.fillStyle = '#f7d774';
  context.fillRect(campX - 4, campY - 14, 8, 8);
}

function drawDestinations(): void {
  for (const destination of destinations) {
    const x = destination.x * tileSize;
    const y = destination.y * tileSize;

    context.fillStyle = '#06050d';
    context.fillRect(x - 18, y - 18, 68, 44);

    context.fillStyle = destination.color;
    context.fillRect(x - 14, y - 14, 60, 36);

    context.fillStyle = '#151029';
    context.font = '700 12px Courier New, monospace';
    context.fillText(destination.label, x - 8, y + 7);
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

function drawPlayer(): void {
  const spriteWidth = raccoonSprite[0].length * spritePixelSize;
  const spriteHeight = raccoonSprite.length * spritePixelSize;
  const spriteX = player.x - spriteWidth / 2;
  const spriteY = player.y - spriteHeight / 2;

  context.fillStyle = '#06050d';
  context.fillRect(spriteX + 4, spriteY + 4, spriteWidth, spriteHeight);

  drawPixelSprite(raccoonSprite, spriteX, spriteY);
}

function getDestinationAtPlayer(): Destination | null {
  return (
    destinations.find((destination) => {
      const destinationX = destination.x * tileSize;
      const destinationY = destination.y * tileSize;
      const distanceX = Math.abs(player.x - destinationX);
      const distanceY = Math.abs(player.y - destinationY);

      return distanceX < tileSize && distanceY < tileSize;
    }) ?? null
  );
}

function isPlayerNearCamp(): boolean {
  const campX = canvas.width / 2;
  const campY = canvas.height / 2;
  const distanceX = Math.abs(player.x - campX);
  const distanceY = Math.abs(player.y - campY);

  return distanceX < tileSize && distanceY < tileSize;
}

function updateStatus(): void {
  activeDestination = getDestinationAtPlayer();

  if (activeDestination) {
    statusOutput.textContent = `${activeDestination.description}. Press Enter to enter.`;
    return;
  }

  if (isPlayerNearCamp()) {
    statusOutput.textContent = 'Standing at camp.';
    return;
  }

  statusOutput.textContent = 'Adventuring…';
}

function movePlayer(): void {
  if (pressedKeys.has('arrowup') || pressedKeys.has('w')) {
    player.y -= movementStep;
  }

  if (pressedKeys.has('arrowdown') || pressedKeys.has('s')) {
    player.y += movementStep;
  }

  if (pressedKeys.has('arrowleft') || pressedKeys.has('a')) {
    player.x -= movementStep;
  }

  if (pressedKeys.has('arrowright') || pressedKeys.has('d')) {
    player.x += movementStep;
  }

  player.x = Math.max(
    playerSize / 2,
    Math.min(canvas.width - playerSize / 2, player.x),
  );
  player.y = Math.max(
    playerSize / 2,
    Math.min(canvas.height - playerSize / 2, player.y),
  );
}

function draw(): void {
  drawBackground();
  drawCamp();
  drawDestinations();
  drawPlayer();
}

function tick(): void {
  movePlayer();
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
