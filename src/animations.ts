import { animate, createScope, createTimeline, stagger } from 'animejs';
import type { Scope } from 'animejs';

const MOTION_QUERY = '(prefers-reduced-motion: reduce)';
const FAST = 150;
const NORMAL = 220;

type CancellableAnimation = { cancel: () => unknown };
let inspectorAnimation: CancellableAnimation | undefined;
let metricAnimation: CancellableAnimation | undefined;

function cancel(animation: CancellableAnimation | undefined) {
  try {
    animation?.cancel();
  } catch {
    // Some animation objects may not expose cancel in every runtime.
  }
}

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

export function animateVoidEnter(root: HTMLElement, scope?: Scope) {
  if (reducedMotion()) return;

  const tl = createTimeline({ defaults: { duration: 300, ease: 'out(3)' } });
  tl.add(targets(root, '.void-chrome > *'), {
    opacity: [0, 1],
    y: [-8, 0],
    delay: stagger(40)
  }, 0)
  .add(targets(root, '.tool-card'), {
    opacity: [0, 1],
    y: [12, 0],
    delay: stagger(80)
  }, 0)
  .add(targets(root, '.command-dock'), {
    opacity: [0, 1],
    y: [10, 0]
  }, 200);

  if (scope) (scope as { register: (item: unknown) => void }).register(tl);
}

export function animatePaneChange(root: HTMLElement) {
  if (reducedMotion()) return;
  const card = one(root, '.tool-card');
  if (!card) return;

  const tl = createTimeline({ defaults: { duration: FAST, ease: 'out(3)' } });
  tl.add(card, {
    opacity: [0.72, 1],
    y: [8, 0],
    scale: [0.996, 1]
  }, 0)
  .add(targets(root, '.transcript-head, .placeholder-copy, .home-body'), {
    opacity: [0, 1],
    y: [8, 0]
  }, 45);

  animateHomeStagger(root);
  animateInspectorRefresh(root);
}

export function animateHomeStagger(root: HTMLElement) {
  if (reducedMotion()) return;
  const homeItems = targets(root, '.launch-card > *, .home-metrics article, .recent-list button');
  if (!homeItems.length) return;

  animate(homeItems, {
    opacity: [0, 1],
    x: [-10, 0],
    y: [4, 0],
    duration: 260,
    delay: stagger(30),
    ease: 'out(3)'
  });
}

export function animateLatestMessage(root: HTMLElement) {
  if (reducedMotion()) return;
  const messages = targets(root, '.chat-log .message');
  const latest = messages.at(-1);
  if (!latest) return;

  const tl = createTimeline({ defaults: { ease: 'out(3)' } });
  tl.add(latest, {
    opacity: [0, 1],
    y: [8, 0],
    duration: 180
  }, 0)
  .add(latest, {
    borderColor: ['#c6f36d', '#37352e'],
    duration: 520
  }, 80);
}

export function animateChatHistory(root: HTMLElement) {
  if (reducedMotion()) return;
  const messages = targets(root, '.chat-log .message');
  if (!messages.length) return;

  animate(messages.slice(-10), {
    opacity: [0, 1],
    y: [6, 0],
    delay: stagger(22),
    duration: 190,
    ease: 'out(3)'
  });
}

export function animateStreamingStatus(root: HTMLElement) {
  if (reducedMotion()) return;
  const live = targets(root, '.pi-state.streaming, .status-pill.streaming');
  if (!live.length) return;

  animate(live, {
    scale: [1, 1.045, 1],
    color: ['#c6f36d', '#e3ff9b', '#c6f36d'],
    duration: 620,
    ease: 'inOut(2)'
  });
}

export function animateInspectorRefresh(root: HTMLElement) {
  if (reducedMotion()) return;
  const cards = targets(root, '.meter-button');
  if (!cards.length) return;

  cancel(inspectorAnimation);
  inspectorAnimation = animate(cards, {
    opacity: [0.78, 1],
    x: [4, 0],
    delay: stagger(18),
    duration: 160,
    ease: 'out(3)'
  }) as unknown as CancellableAnimation;
}

export function animateMetricTick(root: HTMLElement) {
  if (reducedMotion()) return;
  const values = targets(root, '.home-metrics strong, .meters strong');
  if (!values.length) return;

  cancel(metricAnimation);
  metricAnimation = animate(values, {
    scale: [1, 1.045, 1],
    color: ['#eee8db', '#c6f36d', '#eee8db'],
    delay: stagger(24),
    duration: 260,
    ease: 'out(3)'
  }) as unknown as CancellableAnimation;
}

export function animateCommandFocus(root: HTMLElement, focused: boolean) {
  if (reducedMotion()) return;
  const dock = one(root, '.command-dock');
  if (!dock) return;

  animate(dock, {
    y: focused ? -3 : 0,
    scale: focused ? 1.006 : 1,
    duration: 180,
    ease: 'out(3)'
  });
}

export function animateButtonPress(button: HTMLElement) {
  if (reducedMotion()) return;

  animate(button, {
    scale: [1, 0.975, 1],
    duration: 180,
    ease: 'out(3)'
  });
}

export function attachInteractionAnimations(root: HTMLElement) {
  const onFocusIn = (event: FocusEvent) => {
    if ((event.target as HTMLElement | null)?.closest('.command-dock')) animateCommandFocus(root, true);
  };
  const onFocusOut = (event: FocusEvent) => {
    if ((event.target as HTMLElement | null)?.closest('.command-dock')) animateCommandFocus(root, false);
  };
  const onPointerDown = (event: PointerEvent) => {
    const button = (event.target as HTMLElement | null)?.closest('button');
    if (button instanceof HTMLElement) animateButtonPress(button);
  };
  const onPointerEnter = (event: PointerEvent) => {
    const row = (event.target as HTMLElement | null)?.closest('.recent-list button, .meter-button, .menu-row');
    if (row instanceof HTMLElement && !reducedMotion()) {
      animate(row, { x: [0, 3], duration: 120, ease: 'out(3)' });
    }
  };
  const onPointerLeave = (event: PointerEvent) => {
    const row = (event.target as HTMLElement | null)?.closest('.recent-list button, .meter-button, .menu-row');
    if (row instanceof HTMLElement && !reducedMotion()) {
      animate(row, { x: 0, duration: 120, ease: 'out(3)' });
    }
  };

  root.addEventListener('focusin', onFocusIn);
  root.addEventListener('focusout', onFocusOut);
  root.addEventListener('pointerdown', onPointerDown);
  root.addEventListener('pointerover', onPointerEnter);
  root.addEventListener('pointerout', onPointerLeave);

  return () => {
    root.removeEventListener('focusin', onFocusIn);
    root.removeEventListener('focusout', onFocusOut);
    root.removeEventListener('pointerdown', onPointerDown);
    root.removeEventListener('pointerover', onPointerEnter);
    root.removeEventListener('pointerout', onPointerLeave);
  };
}
