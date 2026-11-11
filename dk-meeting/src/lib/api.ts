// Wrapper có kiểu cho mọi lệnh gọi xuống Rust. UI chỉ gọi qua file này.
import { invoke, convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type {
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

  revealInFolder: (path: string) => revealItemInDir(path),
  fileSrc: (path: string) => convertFileSrc(path),
};

export const events = {
  onLevels: (cb: (e: LevelsEvent) => void): Promise<UnlistenFn> =>
    listen<LevelsEvent>("recording://levels", (e) => cb(e.payload)),
  onRecordingError: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("recording://error", (e) => cb(e.payload)),
};

export const inTauri = isTauri;

/** Lỗi từ Rust trả về là chuỗi tiếng Việt; lỗi JS thì lấy message. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Đã có lỗi xảy ra";
}
