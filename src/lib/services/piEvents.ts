import { listen } from '@tauri-apps/api/event';
import type {
  EditorTextEvent,
  ExtensionUiRequest,
  NotifyEvent,
  SessionEvent,
  SessionUpdateEvent,
  StatusEvent,
  TitleEvent,
  SessionClosedEvent,
  WidgetEvent
} from '../types/pi';

export type PiEventHandlers = {
  onMessage: (event: SessionEvent) => void;
  onSessionUpdate: (event: SessionUpdateEvent) => void;
  onExtensionRequest: (event: ExtensionUiRequest) => void;
  onStatus: (event: StatusEvent) => void;
  onWidget: (event: WidgetEvent) => void;
  onNotify: (event: NotifyEvent) => void;
  onEditorText: (event: EditorTextEvent) => void;
  onTitle: (event: TitleEvent) => void;
  onSessionClosed: (event: SessionClosedEvent) => void;
};

export async function attachPiEventListeners(handlers: PiEventHandlers): Promise<Array<() => void>> {
  const messageCleanup = await listen<SessionEvent>('pi://message', (event) => {
    handlers.onMessage(event.payload);
  });

  const sessionCleanup = await listen<SessionUpdateEvent>('pi://session', (event) => {
    handlers.onSessionUpdate(event.payload);
  });

  const extensionCleanup = await listen<ExtensionUiRequest>('pi://extension-ui-request', (event) => {
    handlers.onExtensionRequest(event.payload);
  });

  const statusCleanup = await listen<StatusEvent>('pi://status', (event) => {
    handlers.onStatus(event.payload);
  });

  const widgetCleanup = await listen<WidgetEvent>('pi://widget', (event) => {
    handlers.onWidget(event.payload);
  });

  const notifyCleanup = await listen<NotifyEvent>('pi://notify', (event) => {
    handlers.onNotify(event.payload);
  });

  const editorTextCleanup = await listen<EditorTextEvent>('pi://editor-text', (event) => {
    handlers.onEditorText(event.payload);
  });

  const titleCleanup = await listen<TitleEvent>('pi://title', (event) => {
    handlers.onTitle(event.payload);
  });

  const sessionClosedCleanup = await listen<SessionClosedEvent>('pi://session-closed', (event) => {
    handlers.onSessionClosed(event.payload);
  });

  return [
    messageCleanup,
    sessionCleanup,
    extensionCleanup,
    statusCleanup,
    widgetCleanup,
    notifyCleanup,
    editorTextCleanup,
    titleCleanup,
    sessionClosedCleanup
  ];
}
