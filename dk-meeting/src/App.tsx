import { useCallback, useEffect, useState } from "react";
import { api, errorText, events, inTauri } from "./lib/api";
import type { LevelsEvent, Meeting, SystemInfo } from "./lib/types";
import { Splash } from "./components/Splash";
import { ConfirmHost } from "./components/ConfirmDialog";
import { TitleBar, type NavKey } from "./components/TitleBar";
import { Home } from "./pages/Home";
import { NewMeeting } from "./pages/NewMeeting";
import { Live } from "./pages/Live";
import { MeetingDetailPage } from "./pages/MeetingDetail";
import { SettingsPage } from "./pages/Settings";

export type Route =
  | { name: "home" }
  | { name: "new" }
  | { name: "live" }
  | { name: "meeting"; id: string }
  | { name: "settings" };

export interface ActiveRecording {
  meeting: Meeting;
  paused: boolean;
  elapsedMs: number;
}

const MIN_SPLASH_MS = 900;

export default function App() {
  const [booting, setBooting] = useState(true);
  const [bootError, setBootError] = useState<string>();
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [route, setRoute] = useState<Route>({ name: "home" });
  const [recording, setRecording] = useState<ActiveRecording | null>(null);
  const [levels, setLevels] = useState<LevelsEvent | null>(null);

  // Khởi động: màn hình chờ + lấy thông tin hệ thống + khôi phục phiên đang ghi (nếu có).
  useEffect(() => {
    if (!inTauri()) {
      setBootError("Hãy mở app bằng lệnh `npm run tauri dev` (không chạy trong trình duyệt thường).");
      return;
    }
    const started = Date.now();
    Promise.all([api.systemInfo(), api.recordingStatus()])
      .then(([sys, status]) => {
        setInfo(sys);
        if (status) {
          setRecording({ meeting: status.meeting, paused: status.paused, elapsedMs: status.elapsedMs });
          setRoute({ name: "live" });
        }
        setTimeout(() => setBooting(false), Math.max(0, MIN_SPLASH_MS - (Date.now() - started)));
      })
      .catch((e) => setBootError(errorText(e)));
  }, []);

  // Đồng hồ + mức âm từ Rust (~12 lần/giây).
  useEffect(() => {
    if (!inTauri()) return;
    const un = events.onLevels((e) => {
      setLevels(e);
      setRecording((r) =>
        r && r.meeting.id === e.meetingId ? { ...r, elapsedMs: e.elapsedMs, paused: e.paused } : r,
      );
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const navigate = useCallback((r: Route) => setRoute(r), []);

  if (booting) return <Splash message="Đang khởi động…" error={bootError} />;

  const navKey: NavKey | null = route.name === "settings" ? "settings" : route.name === "home" ? "home" : null;

  return (
    <div className="flex h-full flex-col">
      <TitleBar
        active={navKey}
        onNavigate={(k) => navigate({ name: k })}
        recording={recording && route.name !== "live" ? recording : null}
        onOpenLive={() => navigate({ name: "live" })}
      />
      <main className="min-h-0 flex-1">
        {route.name === "home" && (
          <Home
            recording={recording}
            onNew={() => navigate(recording ? { name: "live" } : { name: "new" })}
            onOpen={(id) => navigate(recording?.meeting.id === id ? { name: "live" } : { name: "meeting", id })}
          />
        )}
        {route.name === "new" && (
          <NewMeeting
            onCancel={() => navigate({ name: "home" })}
            onStarted={(meeting) => {
              setLevels(null);
              setRecording({ meeting, paused: false, elapsedMs: 0 });
              navigate({ name: "live" });
            }}
          />
        )}
        {route.name === "live" && recording && (
          <Live
            recording={recording}
            levels={levels?.meetingId === recording.meeting.id ? levels : null}
            onPausedChange={(paused) => setRecording((r) => (r ? { ...r, paused } : r))}
            onStopped={(m) => {
              setRecording(null);
              setLevels(null);
              navigate({ name: "meeting", id: m.id });
            }}
          />
        )}
        {route.name === "live" && !recording && (
          <Home recording={null} onNew={() => navigate({ name: "new" })} onOpen={(id) => navigate({ name: "meeting", id })} />
        )}
        {route.name === "meeting" && (
          <MeetingDetailPage id={route.id} onBack={() => navigate({ name: "home" })} />
        )}
        {route.name === "settings" && info && <SettingsPage info={info} />}
      </main>
      <ConfirmHost />
    </div>
  );
}
