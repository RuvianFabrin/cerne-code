import { onScopeDispose, ref, watch, type Ref } from "vue";
import { api } from "../api";

const POLL_INTERVAL_MS = 4000;

/**
 * Polls the real /health endpoint (not just "did we spawn a process") for
 * whichever fork id the given ref points at. `isUp` is `null` until the
 * first check lands, then `true`/`false` — the dot should show "unknown"
 * (grey) rather than guessing green or red before that.
 */
export function useLlamaHealth(forkId: Ref<string | null | undefined>) {
  const isUp = ref<boolean | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;

  async function check() {
    const id = forkId.value;
    if (!id) {
      isUp.value = null;
      return;
    }
    try {
      isUp.value = await api.llamaServerHealth(id);
    } catch {
      isUp.value = false;
    }
  }

  function start() {
    stop();
    check();
    timer = setInterval(check, POLL_INTERVAL_MS);
  }

  function stop() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  watch(forkId, () => start(), { immediate: true });
  onScopeDispose(stop);

  return { isUp, refresh: check };
}
