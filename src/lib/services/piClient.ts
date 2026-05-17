import { invoke } from '@tauri-apps/api/core';
import type {
  PiCommandOption,
  PiModelOption,
  PiSession,
  PiSessionMeta
} from '../types/pi';

export const piClient = {
  createSession(projectPath: string | null) {
    return invoke<PiSession>('create_session', { projectPath });
  },
  listSessions() {
    return invoke<PiSession[]>('list_sessions');
  },
  listPiImports() {
    return invoke<PiSessionMeta[]>('list_pi_imports', { projectPath: null });
  },
  importPiSession(sessionFile: string) {
    return invoke<PiSession>('import_pi_session', { sessionFile });
  },
  switchSession(id: string) {
    return invoke('switch_session', { id });
  },
  closeSession(id: string) {
    return invoke('close_session', { id });
  },
  sendMessage(id: string, content: string) {
    return invoke('send_message', { id, content });
  },
  sendPiCommand(id: string, content: string) {
    return invoke('send_pi_command', { id, content });
  },
  resetPiSession(id: string) {
    return invoke('reset_pi_session', { id });
  },
  compactSession(id: string, customInstructions: string | null) {
    return invoke('compact_session', { id, customInstructions });
  },
  renamePiSession(id: string, name: string) {
    return invoke('rename_pi_session', { id, name });
  },
  stopSession(id: string) {
    return invoke('stop_session', { id });
  },
  listPiCommands(id: string) {
    return invoke<PiCommandOption[]>('list_pi_commands', { id });
  },
  respondExtensionUi(id: string, requestId: string, response: Record<string, any>) {
    return invoke('respond_extension_ui', { id, requestId, response });
  },
  listPiModels(id: string) {
    return invoke<PiModelOption[]>('list_pi_models', { id });
  },
  setPiModel(id: string, provider: string, modelId: string) {
    return invoke('set_pi_model', { id, provider, modelId });
  },
  setPiThinkingLevel(id: string, level: string) {
    return invoke('set_pi_thinking_level', { id, level });
  }
};
