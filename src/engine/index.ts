import init, { step, version } from '../../engine/pkg/engine.js';

export type FrameState = {
  protocolVersion: number;
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
  return version();
}

export async function engineStep(): Promise<FrameState> {
  await ensureInitialised();
  return step() as unknown as FrameState;
}