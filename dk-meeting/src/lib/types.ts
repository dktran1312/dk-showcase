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
  segments: Segment[];
  minutes: MinutesRecord | null;
  actionItems: ActionItem[];
  processing: boolean;
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
  stt: SttStatus | null;
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
  sttEnabled: boolean;
  whisperModel: string;
  translateEnabled: boolean;
  ollamaUrl: string;
  ollamaModel: string;
  onboarded: boolean;
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

// ───────────── M3: nhận giọng nói ─────────────

export interface Segment {
  id: number;
  meetingId: string;
  track: Track;
  startMs: number;
  endMs: number;
  lang: string;
  text: string;
  translation: string | null;
  edited: boolean;
}

export type SttStatus =
  | { state: "loading"; model: string }
  | { state: "ready"; model: string; gpu: boolean }
  | { state: "unavailable"; reason: string }
  | { state: "error"; message: string };

export interface WhisperModel {
  id: string;
  label: string;
  file: string;
  sizeMb: number;
  minMemGb: number;
  description: string;
  installed: boolean;
}

export interface DownloadProgress {
  id: string;
  downloaded: number;
  total: number;
  done: boolean;
  error: string | null;
}

export interface GpuInfo {
  name: string;
  vramGb: number | null;
  nvidia: boolean;
}

export interface Hardware {
  os: string;
  cpu: string;
  cores: number;
  ramGb: number;
  gpus: GpuInfo[];
}

export interface OllamaModel {
  name: string;
  size: number;
}

export interface OllamaStatus {
  url: string;
  running: boolean;
  version: string | null;
  models: OllamaModel[];
  configuredModel: string;
  hasModel: boolean;
  error: string | null;
}

export interface SystemCheck {
  hardware: Hardware;
  sttBackend: string;
  whisperModels: WhisperModel[];
  vadInstalled: boolean;
  recommendedModel: string;
  ollama: OllamaStatus;
  settings: Settings;
}

// ───────────── M4: biên bản ─────────────

export interface TechItem {
  requirements: string[];
  numbers: string[];
}

export interface Minutes {
  general: { title: string; date: string; duration: string; participants: string[]; client: string };
  summary: string[];
  brief: {
    objective: string;
    concept: string;
    key_message: string;
    audience: string;
    show_date: string;
    venue: string;
    scale: string;
  };
  technical: {
    led_visual: TechItem;
    sound: TechItem;
    lighting: TechItem;
    sfx: TechItem;
    stage: TechItem;
    artist_mc: TechItem;
    other: TechItem;
  };
  rundown: { time: string; content: string }[];
  budget: { item: string; amount: string; note: string }[];
  decisions: { content: string; source_time: string }[];
  action_items: { task: string; owner: string; deadline: string; source_time: string }[];
  confirm_with_client: string[];
  risks: string[];
}

export interface MinutesRecord {
  data: Minutes;
  model: string;
  edited: boolean;
  updatedAt: string;
}

export type ActionStatus = "todo" | "doing" | "done";

export interface ActionItem {
  id: number;
  task: string;
  owner: string;
  deadline: string;
  status: ActionStatus;
  sourceMs: number | null;
}

export interface MinutesProgress {
  meetingId: string;
  step: number;
  total: number;
  message: string;
}

export const ACTION_STATUS_LABEL: Record<ActionStatus, string> = {
  todo: "Chưa làm",
  doing: "Đang làm",
  done: "Xong",
};

export const LANG_NAME: Record<string, string> = {
  vi: "VI",
  en: "EN",
  ko: "KO",
  ja: "JA",
  zh: "ZH",
  th: "TH",
  fr: "FR",
};
