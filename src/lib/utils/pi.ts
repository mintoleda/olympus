import type { PiCommandOption, PiSession, PiSessionMeta, PrimaryPreset } from '../types/pi';

export const timeFormatter = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit'
});

export function stripAnsi(text: string): string {
  return text ? text.replace(/\u001b\[[0-9;]*[A-Za-z]/g, '') : '';
}

export function parsePreset(text: string | undefined): PrimaryPreset | undefined {
  if (!text) return undefined;
  const cleaned = stripAnsi(text).trim().toLowerCase();
  const match = cleaned.match(/primary\s*[:=]\s*(plan|build|ask|off)/);
  if (match) return match[1] as PrimaryPreset;
  if (['plan', 'build', 'ask', 'off'].includes(cleaned)) return cleaned as PrimaryPreset;
  return undefined;
}

export function nextPreset(
  current: PrimaryPreset | undefined,
  cycle: PrimaryPreset[],
  direction: 1 | -1
): PrimaryPreset {
  const idx = current ? cycle.indexOf(current) : -1;
  const len = cycle.length;
  const nextIdx = direction === 1 ? (idx + 1 + len) % len : idx <= 0 ? len - 1 : idx - 1;
  return cycle[nextIdx];
}

export function formatTime(timestamp: number): string {
  if (!timestamp) return 'no activity';
  return timeFormatter.format(new Date(timestamp));
}

export function latestTimestamp(session: PiSession): number {
  return session.messages.reduce((max, message) => Math.max(max, message.timestamp), 0);
}

export function piImportPreview(meta: PiSessionMeta): string {
  const trimmed = meta.preview.trim();
  if (trimmed) return trimmed;
  return `${meta.message_count} message${meta.message_count === 1 ? '' : 's'}`;
}

export function relativeTime(ms: number): string {
  if (!ms) return '';
  const diff = Date.now() - ms;
  if (diff < 0) return 'just now';
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day}d ago`;
  const mo = Math.floor(day / 30);
  if (mo < 12) return `${mo}mo ago`;
  return `${Math.floor(mo / 12)}y ago`;
}

export function fuzzyScore(name: string, query: string): number {
  if (!query) return 1;
  const lower = name.toLowerCase();
  if (lower === query) return 1000;
  if (lower.startsWith(query)) return 500 - (lower.length - query.length);
  let qi = 0;
  let score = 0;
  let prevMatch = -2;
  for (let i = 0; i < lower.length && qi < query.length; i++) {
    if (lower[i] === query[qi]) {
      score += 10;
      if (i === prevMatch + 1) score += 8;
      if (i === 0 || /[^a-z0-9]/.test(lower[i - 1])) score += 5;
      prevMatch = i;
      qi++;
    }
  }
  if (qi < query.length) return -1;
  return score - (lower.length - query.length);
}

export function rankCommands(commands: PiCommandOption[], query: string): PiCommandOption[] {
  if (!query) return [...commands].sort((a, b) => a.name.localeCompare(b.name));
  const scored = commands
    .map((command) => ({ command, score: fuzzyScore(command.name, query) }))
    .filter((entry) => entry.score >= 0)
    .sort((a, b) => b.score - a.score || a.command.name.localeCompare(b.command.name));
  return scored.map((entry) => entry.command);
}
