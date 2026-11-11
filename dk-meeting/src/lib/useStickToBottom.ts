import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Tự cuộn xuống khi có nội dung mới, nhưng DỪNG khi người dùng kéo lên đọc lại.
 * Cuộn xuống đáy lại thì tự cuộn tiếp.
 */
export function useStickToBottom<T extends HTMLElement>(dep: unknown) {
  const ref = useRef<T>(null);
  const [stuck, setStuck] = useState(true);

  const onScroll = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    setStuck(el.scrollHeight - el.scrollTop - el.clientHeight < 40);
  }, []);

  const scrollToBottom = useCallback(() => {
    const el = ref.current;
    if (el) el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
    setStuck(true);
  }, []);

  useEffect(() => {
    const el = ref.current;
    if (el && stuck) el.scrollTop = el.scrollHeight;
  }, [dep, stuck]);

  return { ref, onScroll, stuck, scrollToBottom };
}
