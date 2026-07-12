// wasm-pack generates `Engine` as a class with `init_state()` and `step()`
// methods. We re-init the wasm module lazily on first use so callers do not
// need to await a separate bootstrap step.
import init, {
  Engine,
  version as engineVersionRaw,
} from '../../engine/pkg/engine.js';

export type Direction = 'up' | 'down' | 'left' | 'right';

export type Input = {
  up: boolean;
  down: boolean;
  left: boolean;
  right: boolean;
  preferredDirection: Direction | null;
  enter: boolean;
};

export type TilePos = { x: number; y: number };

export type TreatKind =
  | 'cheeseburger'
  | 'snail'
  | 'frog'
  | 'banana'
  | 'cherries'
  | 'berries'
  | 'apple';

export type TreatInfo = TilePos & { kind: TreatKind };

export type DestinationInfo = {
  x: number;
  y: number;
  href: string;
  label: string;
};

export type InitState = {
  protocolVersion: number;
  width: number;
  height: number;
  tileSize: number;
  walls: TilePos[];
  camp: TilePos;
  playerStart: TilePos;
  destinations: DestinationInfo[];
  treats: TreatInfo[];
};

export type FrameState = {
  protocolVersion: number;
  playerX: number;
  playerY: number;
  playerVx: number;
  playerVy: number;
  score: number;
  /** Live treat positions and varieties, excluding collected treats. */
  treats: TreatInfo[];
  status: string;
  activeDestinationHref: string | null;
  pendingNavigation: string | null;
  justCollectedTreat: boolean;
};

let initialised = false;

async function ensureInitialised(): Promise<void> {
  if (initialised) {
    return;
  }
  await init();
  initialised = true;
}

export async function engineVersion(): Promise<string> {
  await ensureInitialised();
  return engineVersionRaw();
}

export async function createEngine(seed: number | bigint): Promise<Engine> {
  await ensureInitialised();
  return new Engine(BigInt(seed));
}