import { animate, createScope, createTimeline, stagger } from 'animejs';
import type { Scope } from 'animejs';

const MOTION_QUERY = '(prefers-reduced-motion: reduce)';
const FAST = 180;
const NORMAL = 260;

function reducedMotion() {
  return typeof window !== 'undefined' && window.matchMedia(MOTION_QUERY).matches;
}

function targets(root: HTMLElement, selector: string) {
  return Array.from(root.querySelectorAll<HTMLElement>(selector));
}

function one(root: HTMLElement, selector: string) {
  return root.querySelector<HTMLElement>(selector);
}

export function createAppAnimationScope(root: HTMLElement) {
  return createScope({
    root,
    mediaQueries: { reduceMotion: MOTION_QUERY },
    defaults: { duration: NORMAL, ease: 'out(3)' }
  });
}

export function animateShellEnter(root: HTMLElement, scope?: Scope) {
  if (reducedMotion()) return;

  const tl = createTimeline({ defaults: { duration: 300, ease: 'out(3)' } });
  tl.add(targets(root, '.nav-rail, .topbar, .main-panel, .inspector'), {
    opacity: [0, 1],
    y: [10, 0],
    delay: stagger(42)
  }, 0)
    .add(targets(root, '.pane-tabs button'), {
      opacity: [0, 1],
      x: [-8, 0],
      delay: stagger(24)
    }, 70);

  if (scope) (scope as { register: (item: unknown) => void }).register(tl);
}

export function animatePaneChange(root: HTMLElement) {
  if (reducedMotion()) return;
  const panel = one(root, '.main-panel');
  if (!panel) return;

  animate(panel, {
    opacity: [0.72, 1],
    y: [8, 0],
    duration: FAST,
    ease: 'out(3)'
  });

  animateHomeStagger(root);
}

export function animateHomeStagger(root: HTMLElement) {
  if (reducedMotion()) return;
  const homeItems = targets(root, '.launch-card > *, .home-metrics article, .recent-list button');
  if (!homeItems.length) return;

  animate(homeItems, {
    opacity: [0, 1],
    x: [-8, 0],
    duration: 240,
    delay: stagger(28),
    ease: 'out(3)'
  });
}

export function animateSessionRail(root: HTMLElement, collapsed: boolean) {
  if (reducedMotion()) return;
  const rail = one(root, collapsed ? '.sessions-peek' : '.session-tree');
  if (!rail) return;

  animate(rail, {
    opacity: [0, 1],
    x: collapsed ? [-7, 0] : [-10, 0],
    duration: 210,
    ease: 'out(3)'
  });

  if (!collapsed) {
    animate(targets(root, '.session-tree > button'), {
      opacity: [0, 1],
      x: [-6, 0],
      delay: stagger(18),
      duration: 180,
      ease: 'out(3)'
    });
  }
}

export function animateLatestMessage(root: HTMLElement) {
  if (reducedMotion()) return;
  const messages = targets(root, '.chat-log .message');
  const latest = messages.at(-1);
  if (!latest) return;

  animate(latest, {
    opacity: [0, 1],
    y: [6, 0],
    duration: 210,
    ease: 'out(3)'
  });
}

export function animateStreamingStatus(root: HTMLElement) {
  if (reducedMotion()) return;
  const live = targets(root, '.pi-state.streaming, .transcript-head span.streaming');
  if (!live.length) return;

  animate(live, {
    scale: [1, 1.035, 1],
    color: ['#c6f36d', '#e3ff9b', '#c6f36d'],
    duration: 620,
    ease: 'inOut(2)'
  });
}
