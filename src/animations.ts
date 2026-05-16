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

  const tl = createTimeline({ defaults: { duration: 320, ease: 'out(3)' } });
  tl.add(targets(root, '.nav-rail, .topbar, .main-panel, .inspector'), {
    opacity: [0, 1],
    y: [12, 0],
    filter: ['blur(4px)', 'blur(0px)'],
    delay: stagger(48)
  }, 0)
    .add(targets(root, '.product-mark span, .pane-tabs button, .rail-status'), {
      opacity: [0, 1],
      x: [-10, 0],
      delay: stagger(24)
    }, 90)
    .add(targets(root, '.meters article, .context-card'), {
      opacity: [0, 1],
      x: [8, 0],
      delay: stagger(36)
    }, 140)
    .add(targets(root, '.command-dock'), {
      opacity: [0, 1],
      y: [10, 0]
    }, 190);

  if (scope) (scope as { register: (item: unknown) => void }).register(tl);
}

export function animatePaneChange(root: HTMLElement) {
  if (reducedMotion()) return;
  const panel = one(root, '.main-panel');
  if (!panel) return;

  const tl = createTimeline({ defaults: { duration: FAST, ease: 'out(3)' } });
  tl.add(panel, {
    opacity: [0.62, 1],
    y: [10, 0],
    filter: ['blur(3px)', 'blur(0px)']
  }, 0)
    .add(targets(root, '.transcript-head, .placeholder-copy, .home-surface'), {
      opacity: [0, 1],
      y: [8, 0]
    }, 45);

  animateActiveNav(root);
  animateHomeStagger(root);
  animateInspectorRefresh(root);
}

export function animateActiveNav(root: HTMLElement) {
  if (reducedMotion()) return;
  const active = one(root, '.pane-tabs button.active');
  if (!active) return;

  animate(active, {
    x: [0, 3, 0],
    duration: 260,
    ease: 'out(3)'
  });

  const key = active.querySelector<HTMLElement>('b');
  if (key) {
    animate(key, {
      scale: [0.9, 1.08, 1],
      duration: 320,
      ease: 'out(4)'
    });
  }
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

export function animateSessionRail(root: HTMLElement, collapsed: boolean) {
  if (reducedMotion()) return;
  const rail = one(root, collapsed ? '.sessions-peek' : '.session-tree');
  if (!rail) return;

  const tl = createTimeline({ defaults: { duration: 220, ease: 'out(3)' } });
  tl.add(rail, {
    opacity: [0, 1],
    x: collapsed ? [-9, 0] : [-12, 0],
    filter: ['blur(3px)', 'blur(0px)']
  }, 0);

  if (collapsed) {
    tl.add(targets(root, '.sessions-peek span, .sessions-peek strong'), {
      opacity: [0, 1],
      y: [-5, 0],
      delay: stagger(42)
    }, 80);
  } else {
    tl.add(targets(root, '.session-tree .panel-head, .project-label, .session-tree > button'), {
      opacity: [0, 1],
      x: [-8, 0],
      delay: stagger(18)
    }, 50);
  }
}

export function animateSessionStack(root: HTMLElement) {
  if (reducedMotion()) return;
  const rows = targets(root, '.session-tree > button');
  if (!rows.length) return;

  animate(rows, {
    opacity: [0.55, 1],
    x: [-5, 0],
    delay: stagger(15),
    duration: 170,
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
    filter: ['blur(2px)', 'blur(0px)'],
    duration: 220
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
  const live = targets(root, '.pi-state.streaming, .transcript-head span.streaming');
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
  const cards = targets(root, '.meters article, .context-card');
  if (!cards.length) return;

  animate(cards, {
    opacity: [0.68, 1],
    x: [6, 0],
    delay: stagger(28),
    duration: 210,
    ease: 'out(3)'
  });
}

export function animateMetricTick(root: HTMLElement) {
  if (reducedMotion()) return;
  const values = targets(root, '.home-metrics strong, .meters strong');
  if (!values.length) return;

  animate(values, {
    scale: [1, 1.06, 1],
    color: ['#eee8db', '#c6f36d', '#eee8db'],
    delay: stagger(35),
    duration: 360,
    ease: 'out(3)'
  });
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
    const row = (event.target as HTMLElement | null)?.closest('.recent-list button, .session-tree > button, .meters article');
    if (row instanceof HTMLElement && !reducedMotion()) {
      animate(row, { x: [0, 3], duration: 140, ease: 'out(3)' });
    }
  };
  const onPointerLeave = (event: PointerEvent) => {
    const row = (event.target as HTMLElement | null)?.closest('.recent-list button, .session-tree > button, .meters article');
    if (row instanceof HTMLElement && !reducedMotion()) {
      animate(row, { x: 0, duration: 150, ease: 'out(3)' });
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
