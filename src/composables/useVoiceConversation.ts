// Conversa por voz contínua: escuta o microfone, detecta o silêncio que
// encerra cada fala, entrega a fala pronta (WAV) ao chat e fica em pausa
// enquanto o modelo responde e a resposta toca. Depois volta a ouvir.
//
// Estados:
// - off: sem conversa ativa.
// - listening: microfone aberto, esperando a pessoa falar.
// - recording: a pessoa está falando; grava até o silêncio.
// - busy: pausado (aguardando a resposta ou tocando a fala). Não escuta,
//   então o próprio áudio da resposta não volta como entrada.

import { computed, ref } from "vue";
import { blobToWavDataUri } from "../lib/wav";

export type VoiceState = "off" | "listening" | "recording" | "busy";

/** Idiomas que o modelo de fala do LFM3 aceita (ver `lfm3::LANGUAGES`). */
export const VOICE_LANGUAGES = ["pt", "en", "es", "fr", "de", "it", "zh", "ja", "ko", "ru", "ar", "nl"] as const;

/** Nível de RMS (0..1) acima do qual conta como fala. */
const SPEECH_LEVEL = 0.02;
/** Silêncio contínuo que encerra uma fala. */
const SILENCE_MS = 900;
/** Fala mais curta que isso é clique, não pergunta. */
const MIN_UTTERANCE_MS = 400;
/** Limite de uma fala só, pra uma pessoa que não para de falar não travar a conversa. */
const MAX_UTTERANCE_MS = 30_000;
const TICK_MS = 50;

export function useVoiceConversation() {
  const state = ref<VoiceState>("off");
  const active = computed(() => state.value !== "off");

  let stream: MediaStream | null = null;
  let context: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let samples: Float32Array | null = null;
  let timer: ReturnType<typeof setInterval> | null = null;
  let recorder: MediaRecorder | null = null;
  let chunks: Blob[] = [];
  let segmentStart = 0;
  let lastVoiceAt = 0;
  let onUtterance: ((dataUri: string) => void) | null = null;

  function level(): number {
    if (!analyser || !samples) return 0;
    analyser.getFloatTimeDomainData(samples);
    let sum = 0;
    for (const v of samples) sum += v * v;
    return Math.sqrt(sum / samples.length);
  }

  function beginSegment(now: number) {
    if (!stream) return;
    chunks = [];
    recorder = new MediaRecorder(stream);
    recorder.ondataavailable = (e) => {
      if (e.data.size > 0) chunks.push(e.data);
    };
    recorder.start();
    segmentStart = now;
    lastVoiceAt = now;
    state.value = "recording";
  }

  /** Volta a escutar sem mexer em uma conversa que já foi encerrada. */
  function listenAgain() {
    if (state.value !== "off") state.value = "listening";
  }

  function endSegment() {
    const current = recorder;
    recorder = null;
    if (!current) return;
    const durationMs = performance.now() - segmentStart;
    current.onstop = async () => {
      if (durationMs < MIN_UTTERANCE_MS || chunks.length === 0) {
        listenAgain();
        return;
      }
      state.value = "busy";
      try {
        const dataUri = await blobToWavDataUri(new Blob(chunks, { type: current.mimeType }));
        onUtterance?.(dataUri);
      } catch {
        listenAgain();
      }
    };
    current.stop();
  }

  function tick() {
    if (state.value !== "listening" && state.value !== "recording") return;
    const now = performance.now();
    if (state.value === "listening") {
      if (level() > SPEECH_LEVEL) beginSegment(now);
      return;
    }
    if (level() > SPEECH_LEVEL) lastVoiceAt = now;
    if (now - lastVoiceAt > SILENCE_MS || now - segmentStart > MAX_UTTERANCE_MS) endSegment();
  }

  /** Abre o microfone e começa a escutar. `handler` recebe cada fala como WAV em data URI. */
  async function start(handler: (dataUri: string) => void) {
    if (active.value) return;
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    context = new AudioContext();
    analyser = context.createAnalyser();
    analyser.fftSize = 2048;
    context.createMediaStreamSource(stream).connect(analyser);
    samples = new Float32Array(analyser.fftSize);
    onUtterance = handler;
    state.value = "listening";
    timer = setInterval(tick, TICK_MS);
  }

  /** Pausa a escuta (o chat está respondendo ou a resposta está tocando). */
  function pause() {
    if (state.value === "recording") endSegment();
    if (state.value !== "off") state.value = "busy";
  }

  /** Volta a escutar depois de uma resposta. */
  function resume() {
    if (state.value === "busy") state.value = "listening";
  }

  /** Encerra a conversa e libera o microfone. */
  function stop() {
    if (timer) clearInterval(timer);
    timer = null;
    if (recorder) {
      recorder.onstop = null;
      recorder.stop();
    }
    recorder = null;
    stream?.getTracks().forEach((t) => t.stop());
    stream = null;
    void context?.close();
    context = null;
    analyser = null;
    samples = null;
    onUtterance = null;
    state.value = "off";
  }

  return { state, active, start, pause, resume, stop };
}

/** Texto que faz sentido ser lido em voz alta: sem blocos de código, links ou marcação. */
export function speakableText(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\s*[-*+]\s+/gm, "")
    .replace(/[*_~>|]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 1200);
}
