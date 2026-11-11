// Khớp với các struct Rust (serde camelCase) trong src-tauri/src/storage/models.rs & commands.rs

export type Track = "mic" | "system";
export type Language = "auto" | "vi" | "en";
export type MeetingStatus = "recording" | "paused" | "done" | "interrupted";

export interface Meeting {
  id: string;
  title: string;
  project: string;
  client: string;
  language: Language;
  status: MeetingStatus;
  createdAt: string;
  startedAt: string;
  endedAt: string | null;
  durationMs: number;
  notes: string;
  bookmarkCount: number;
}

export interface AudioChunk {
  track: Track;
  idx: number;
  fileName: string;
  path: string;
  sampleRate: number;
  startMs: number;
  durationMs: number;
}

export interface Bookmark {
  id: number;
  meetingId: string;
  atMs: number;
  label: string;
}

export interface MeetingDetail {
  meeting: Meeting;
  chunks: AudioChunk[];
  bookmarks: Bookmark[];
  folder: string;
}

export interface MeetingPatch {
  title?: string;
  project?: string;
  client?: string;
  notes?: string;
}

export interface InputDevice {
  name: string;
  isDefault: boolean;
}

export type SourceKind =
  | { type: "mic"; device: string | null }
  | { type: "systemLoopback" }
  | { type: "processLoopback"; pid: number; name: string };

export interface StartMeetingRequest {
  title: string;
  project: string;
  client: string;
  language: Language;
  micDevice: string | null;
}

export interface RecordingStatus {
  meeting: Meeting;
  paused: boolean;
  elapsedMs: number;
  sources: SourceKind[];
}

export interface TrackLevel {
  track: Track;
  rms: number;
  peak: number;
}

export interface LevelsEvent {
  meetingId: string;
  elapsedMs: number;
  paused: boolean;
  levels: TrackLevel[];
}

export interface Settings {
  micDevice: string | null;
  language: Language;
  theme: "dark";
}

export interface SystemInfo {
  version: string;
  os: string;
  arch: string;
  dataDir: string;
  recordingsDir: string;
  dbPath: string;
}

export const TRACK_LABEL: Record<Track, string> = {
  mic: "Phía mình",
  system: "Phía online",
};

export const LANGUAGE_LABEL: Record<Language, string> = {
  auto: "Tự nhận diện",
  vi: "Tiếng Việt",
  en: "English",
};

export const STATUS_LABEL: Record<MeetingStatus, string> = {
  recording: "Đang ghi",
  paused: "Tạm dừng",
  done: "Hoàn tất",
  interrupted: "Bị gián đoạn",
};
