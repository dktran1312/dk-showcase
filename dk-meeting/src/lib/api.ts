// Wrapper có kiểu cho mọi lệnh gọi xuống Rust. UI chỉ gọi qua file này.
import { invoke, convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type {
  ActionItem,
  DownloadProgress,
  MinutesProgress,
  MinutesRecord,
  OllamaStatus,
  Segment,
  SttStatus,
  SystemCheck,
  WhisperModel,
  Bookmark,
  InputDevice,
  LevelsEvent,
  Meeting,
  MeetingDetail,
  MeetingPatch,
  RecordingStatus,
  Settings,
  StartMeetingRequest,
  SystemInfo,
} from "./types";

export const api = {
  systemInfo: () => invoke<SystemInfo>("get_system_info"),

  listMeetings: (query?: string) => invoke<Meeting[]>("list_meetings", { query: query || null }),
  getMeeting: (id: string) => invoke<MeetingDetail>("get_meeting", { id }),
  updateMeeting: (id: string, patch: MeetingPatch) => invoke<Meeting>("update_meeting", { id, patch }),
  deleteMeeting: (id: string) => invoke<void>("delete_meeting", { id }),

  listInputDevices: () => invoke<InputDevice[]>("list_input_devices"),
  startMeeting: (req: StartMeetingRequest) => invoke<Meeting>("start_meeting", { req }),
  recordingStatus: () => invoke<RecordingStatus | null>("get_recording_status"),
  pauseMeeting: () => invoke<void>("pause_meeting"),
  resumeMeeting: () => invoke<void>("resume_meeting"),
  stopMeeting: () => invoke<Meeting>("stop_meeting"),

  addBookmark: (label?: string) => invoke<Bookmark>("add_bookmark", { label: label ?? null }),
  updateBookmark: (id: number, label: string) => invoke<void>("update_bookmark", { id, label }),
  deleteBookmark: (id: number) => invoke<void>("delete_bookmark", { id }),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),

  // M3–M4
  systemCheck: () => invoke<SystemCheck>("get_system_check"),
  listWhisperModels: () => invoke<WhisperModel[]>("list_whisper_models"),
  downloadWhisperModel: (id: string) => invoke<void>("download_whisper_model", { id }),
  cancelDownload: (id: string) => invoke<void>("cancel_download", { id }),
  deleteWhisperModel: (id: string) => invoke<void>("delete_whisper_model", { id }),
  ollamaStatus: () => invoke<OllamaStatus>("ollama_status"),
  ollamaPull: (model: string) => invoke<void>("ollama_pull", { model }),
  quickSummary: () => invoke<string>("quick_summary"),
  generateMinutes: (id: string) =>
    invoke<{ minutes: MinutesRecord | null; actionItems: ActionItem[] }>("generate_minutes", { id }),
  saveMinutes: (id: string, data: unknown) => invoke<MinutesRecord | null>("save_minutes", { id, data }),
  saveActionItems: (id: string, items: ActionItem[]) => invoke<ActionItem[]>("save_action_items", { id, items }),
  transcribeMeeting: (id: string) => invoke<void>("transcribe_meeting", { id }),

  revealInFolder: (path: string) => revealItemInDir(path),
  fileSrc: (path: string) => convertFileSrc(path),
};

export const events = {
  onLevels: (cb: (e: LevelsEvent) => void): Promise<UnlistenFn> =>
    listen<LevelsEvent>("recording://levels", (e) => cb(e.payload)),
  onRecordingError: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("recording://error", (e) => cb(e.payload)),
  onSegment: (cb: (s: Segment) => void) => listen<Segment>("stt://segment", (e) => cb(e.payload)),
  onTranslation: (cb: (e: { meetingId: string; segmentId: number; translation: string }) => void) =>
    listen<{ meetingId: string; segmentId: number; translation: string }>("stt://translation", (e) => cb(e.payload)),
  onSttStatus: (cb: (e: { meetingId: string; status: SttStatus }) => void) =>
    listen<{ meetingId: string; status: SttStatus }>("stt://status", (e) => cb(e.payload)),
  onLlmError: (cb: (e: { meetingId: string; message: string }) => void) =>
    listen<{ meetingId: string; message: string }>("llm://error", (e) => cb(e.payload)),
  onProcessed: (cb: (e: { meetingId: string }) => void) =>
    listen<{ meetingId: string }>("meeting://processed", (e) => cb(e.payload)),
  onModelProgress: (cb: (p: DownloadProgress) => void) =>
    listen<DownloadProgress>("models://progress", (e) => cb(e.payload)),
  onOllamaPull: (cb: (p: { model: string; status: string; completed: number; total: number }) => void) =>
    listen<{ model: string; status: string; completed: number; total: number }>("ollama://pull", (e) => cb(e.payload)),
  onMinutesProgress: (cb: (p: MinutesProgress) => void) =>
    listen<MinutesProgress>("minutes://progress", (e) => cb(e.payload)),
  onTranscribeProgress: (cb: (p: { meetingId: string; doneMs: number; totalMs: number }) => void) =>
    listen<{ meetingId: string; doneMs: number; totalMs: number }>("transcribe://progress", (e) => cb(e.payload)),
};

/** Đăng ký listener trong useEffect: `useEffect(() => subscribe(events.onX(cb)), [])`. */
export function subscribe(p: Promise<UnlistenFn>): () => void {
  return () => {
    p.then((f) => f());
  };
}

export const inTauri = isTauri;

/** Lỗi từ Rust trả về là chuỗi tiếng Việt; lỗi JS thì lấy message. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Đã có lỗi xảy ra";
}
