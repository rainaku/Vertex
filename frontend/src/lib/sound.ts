import { writable, get } from 'svelte/store';

export const SOUND_KEY = 'vertex.sound-enabled.v1';

export type SoundEffect =
  | 'wheel_open'
  | 'wheel_close'
  | 'petal_hover'
  | 'page_turn'
  | 'convert_start'
  | 'convert_done'
  | 'convert_error';

function loadSoundPreference(): boolean {
  try {
    const raw = localStorage.getItem(SOUND_KEY);
    if (raw === 'false') return false;
    if (raw === 'true') return true;
  } catch {}
  return true;
}

export const soundEnabled = writable<boolean>(loadSoundPreference());

export function isSoundEnabled(): boolean {
  return get(soundEnabled);
}

export function setSoundEnabled(enabled: boolean): void {
  soundEnabled.set(enabled);
  try {
    localStorage.setItem(SOUND_KEY, String(enabled));
  } catch {}
}

let audioCtx: AudioContext | null = null;
let lastHoverTime = 0;

function getAudioContext(): AudioContext | null {
  if (typeof window === 'undefined') return null;
  if (audioCtx && audioCtx.state !== 'closed') {
    if (audioCtx.state === 'suspended') {
      void audioCtx.resume().catch(() => {});
    }
    return audioCtx;
  }
  const Ctor =
    window.AudioContext ||
    (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctor) return null;
  try {
    audioCtx = new Ctor();
    return audioCtx;
  } catch {
    return null;
  }
}

function scheduleTone(
  ctx: AudioContext,
  opts: {
    type?: OscillatorType;
    startFreq: number;
    endFreq?: number;
    startTime: number;
    duration: number;
    gain: number;
    attack?: number;
  }
) {
  const osc = ctx.createOscillator();
  const gainNode = ctx.createGain();
  osc.type = opts.type ?? 'sine';
  osc.frequency.setValueAtTime(opts.startFreq, opts.startTime);
  if (opts.endFreq && opts.endFreq !== opts.startFreq) {
    osc.frequency.exponentialRampToValueAtTime(
      Math.max(20, opts.endFreq),
      opts.startTime + opts.duration
    );
  }

  const attack = opts.attack ?? Math.min(0.008, opts.duration * 0.25);
  gainNode.gain.setValueAtTime(0.0001, opts.startTime);
  gainNode.gain.linearRampToValueAtTime(opts.gain, opts.startTime + attack);
  gainNode.gain.exponentialRampToValueAtTime(0.0001, opts.startTime + opts.duration);

  osc.connect(gainNode);
  gainNode.connect(ctx.destination);
  osc.start(opts.startTime);
  osc.stop(opts.startTime + opts.duration + 0.005);
}

export function playSound(effect: SoundEffect): void {
  if (!get(soundEnabled)) return;

  if (effect === 'petal_hover') {
    const nowMs = performance.now();
    if (nowMs - lastHoverTime < 28) return;
    lastHoverTime = nowMs;
  }

  const ctx = getAudioContext();
  if (!ctx) return;

  const t0 = ctx.currentTime;

  switch (effect) {
    case 'wheel_open':
      scheduleTone(ctx, {
        type: 'sine',
        startFreq: 340,
        endFreq: 680,
        startTime: t0,
        duration: 0.085,
        gain: 0.055,
      });
      break;

    case 'wheel_close':
      scheduleTone(ctx, {
        type: 'sine',
        startFreq: 520,
        endFreq: 260,
        startTime: t0,
        duration: 0.075,
        gain: 0.04,
      });
      break;

    case 'petal_hover':
      scheduleTone(ctx, {
        type: 'triangle',
        startFreq: 960,
        endFreq: 760,
        startTime: t0,
        duration: 0.022,
        gain: 0.032,
        attack: 0.002,
      });
      break;

    case 'page_turn':
      scheduleTone(ctx, {
        type: 'sine',
        startFreq: 540,
        endFreq: 720,
        startTime: t0,
        duration: 0.045,
        gain: 0.045,
      });
      break;

    case 'convert_start':
      scheduleTone(ctx, {
        type: 'triangle',
        startFreq: 480,
        endFreq: 640,
        startTime: t0,
        duration: 0.06,
        gain: 0.055,
      });
      break;

    case 'convert_done':
      scheduleTone(ctx, {
        type: 'sine',
        startFreq: 587.33, // D5
        startTime: t0,
        duration: 0.11,
        gain: 0.065,
      });
      scheduleTone(ctx, {
        type: 'sine',
        startFreq: 880, // A5
        startTime: t0 + 0.065,
        duration: 0.16,
        gain: 0.075,
      });
      break;

    case 'convert_error':
      scheduleTone(ctx, {
        type: 'triangle',
        startFreq: 300,
        endFreq: 240,
        startTime: t0,
        duration: 0.09,
        gain: 0.065,
      });
      scheduleTone(ctx, {
        type: 'triangle',
        startFreq: 220,
        endFreq: 185,
        startTime: t0 + 0.085,
        duration: 0.13,
        gain: 0.065,
      });
      break;
  }
}

