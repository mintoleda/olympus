export type PaneId = 'home' | 'chat' | 'search' | 'settings';

export type ChatMessage = {
  id: string;
  role: 'user' | 'assistant' | 'status' | 'system';
  content: string;
  timestamp: number;
  type?: string;
};

export type PiSession = {
  id: string;
  name: string;
  project_path: string;
  status: string;
  messages: ChatMessage[];
  pi_session_id?: string | null;
  pi_session_file?: string | null;
  model?: string | null;
  model_id?: string | null;
  provider?: string | null;
  thinking_level?: string | null;
};

export type PiSessionMeta = {
  session_id: string;
  session_file: string;
  project_path: string;
  started_at: string;
  last_activity_ms: number;
  message_count: number;
  preview: string;
  provider?: string | null;
  model_id?: string | null;
  thinking_level?: string | null;
};

export type SessionEvent = { session_id: string; message: ChatMessage };
export type SessionUpdateEvent = { session: PiSession };

export type PiModelOption = {
  provider: string;
  id: string;
  context: string;
  max_output: string;
  reasoning: boolean;
  images: boolean;
};

export type PiCommandOption = {
  name: string;
  description: string;
  source: string;
  location?: string;
  path?: string;
};

export type ExtensionUiRequest = { session_id: string; request: Record<string, any> };

export type StatusEntry = { key: string; text: string };
export type StatusEvent = { session_id: string; statuses: StatusEntry[] };

export type WidgetEntry = { key: string; lines: string[]; placement: string };
export type WidgetEvent = { session_id: string; widgets: WidgetEntry[] };

export type NotifyEvent = { session_id: string; message: string; level: string };
export type EditorTextEvent = { session_id: string; text: string };

export type PrimaryPreset = 'plan' | 'build' | 'ask' | 'off';
export type ConfigChooser = 'provider' | 'model' | 'thinking' | null;

export const PRIMARY_CYCLE: PrimaryPreset[] = ['plan', 'build', 'ask', 'off'];

export const thinkingLevels = ['off', 'minimal', 'low', 'medium', 'high', 'xhigh'];

export const panes: Array<{ id: PaneId; label: string; key: string; description: string }> = [
  {
    id: 'home',
    label: 'Home',
    key: 'HM',
    description: 'Resume recent work, open a folder, or start a clean Pi context.'
  },
  {
    id: 'chat',
    label: 'Chat',
    key: 'CH',
    description: 'Project-bound Pi conversations with model/provider controls.'
  },
  {
    id: 'search',
    label: 'Find',
    key: 'FD',
    description: 'A planned index for sessions, files, commands, and Pi context.'
  },
  {
    id: 'settings',
    label: 'Settings',
    key: 'ST',
    description: 'Preferences, permissions, theme, layout, and platform details.'
  }
];
