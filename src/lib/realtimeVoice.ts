// Cliente de /v1/realtime do llama.cpp (LFM3-Audio/Omni). Uma conexão faz
// ouvir e falar: manda o áudio da fala (PCM16 16 kHz), e recebe a resposta em
// texto e em áudio (PCM16 24 kHz) que toca conforme chega, sem esperar o fim.
//
// Não há detecção de fala no servidor (turn_detection fica desligado): quem
// decide quando a fala acabou é o `useVoiceConversation`, e aqui só se
// envia `input_audio_buffer.commit` + `response.create`.

export interface RealtimeHandlers {
  /** Texto final da resposta (o mesmo que foi falado). */
  onReply(text: string): void;
  /** A resposta terminou e o último trecho já tocou. */
  onDone(): void;
  onError(message: string): void;
}

const OUTPUT_RATE = 24000;
/** Bytes por envio: número par, pra cada envio conter amostras inteiras. */
const APPEND_CHUNK = 32000;

export class RealtimeVoice {
  private ws: WebSocket | null = null;
  private context: AudioContext | null = null;
  private nextStart = 0;
  private replyText = "";
  private playbackTimer: ReturnType<typeof setInterval> | null = null;

  constructor(
    private readonly port: number,
    private readonly model: string,
    private readonly instructions: string,
    private readonly handlers: RealtimeHandlers,
  ) {}

  /** Abre a conexão e resolve quando o servidor confirma a sessão. */
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(`ws://127.0.0.1:${this.port}/v1/realtime?model=${encodeURIComponent(this.model)}`);
      this.ws = ws;
      let configured = false;
      ws.onerror = () => reject(new Error("não foi possível abrir /v1/realtime (o servidor do LFM3 está rodando?)"));
      ws.onclose = () => {
        if (!configured) reject(new Error("conexão de tempo real encerrada antes de começar"));
        this.ws = null;
      };
      ws.onmessage = (e) => {
        const event = JSON.parse(String(e.data));
        if (event.type === "session.created") {
          ws.send(JSON.stringify({
            type: "session.update",
            session: { type: "realtime", instructions: this.instructions, output_modalities: ["audio", "text"] },
          }));
        } else if (event.type === "session.updated" && !configured) {
          configured = true;
          resolve();
        } else {
          this.onEvent(event);
        }
      };
    });
  }

  /** Envia uma fala (WAV 16 kHz mono em data URI) e pede a resposta. */
  sendUtterance(wavDataUri: string) {
    const ws = this.ws;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      this.handlers.onError("conexão de tempo real fechada");
      return;
    }
    this.replyText = "";
    const pcm = wavToPcm16(wavDataUri);
    for (let i = 0; i < pcm.length; i += APPEND_CHUNK) {
      ws.send(JSON.stringify({
        type: "input_audio_buffer.append",
        audio: bytesToBase64(pcm.subarray(i, i + APPEND_CHUNK)),
      }));
    }
    ws.send(JSON.stringify({ type: "input_audio_buffer.commit" }));
    ws.send(JSON.stringify({ type: "response.create" }));
  }

  /** Para a reprodução e fecha a conexão. */
  close() {
    if (this.playbackTimer) clearInterval(this.playbackTimer);
    this.playbackTimer = null;
    const ws = this.ws;
    this.ws = null;
    ws?.close();
    void this.context?.close();
    this.context = null;
    this.nextStart = 0;
  }

  private onEvent(event: { type: string; delta?: string; [key: string]: unknown }) {
    switch (event.type) {
      case "response.output_audio.delta":
        if (event.delta) this.playChunk(event.delta);
        break;
      case "response.output_text.delta":
        this.replyText += event.delta ?? "";
        break;
      case "response.done":
        this.waitForPlayback();
        break;
      case "error": {
        const message = (event as { error?: { message?: string } }).error?.message ?? "erro no tempo real";
        this.handlers.onError(message);
        break;
      }
      default:
        break;
    }
  }

  private playChunk(base64: string) {
    if (!this.context) this.context = new AudioContext({ sampleRate: OUTPUT_RATE });
    const bytes = base64ToBytes(base64);
    const samples = Math.floor(bytes.length / 2);
    if (samples === 0) return;
    const buffer = this.context.createBuffer(1, samples, OUTPUT_RATE);
    const channel = buffer.getChannelData(0);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    for (let i = 0; i < samples; i++) channel[i] = view.getInt16(i * 2, true) / 32768;
    const source = this.context.createBufferSource();
    source.buffer = buffer;
    source.connect(this.context.destination);
    const startAt = Math.max(this.nextStart, this.context.currentTime + 0.02);
    source.start(startAt);
    this.nextStart = startAt + buffer.duration;
  }

  /** Avisa quando o último trecho de áudio terminar de tocar. */
  private waitForPlayback() {
    const reply = this.replyText.trim();
    if (reply) this.handlers.onReply(reply);
    const check = () => {
      const now = this.context?.currentTime ?? 0;
      if (now >= this.nextStart) {
        if (this.playbackTimer) clearInterval(this.playbackTimer);
        this.playbackTimer = null;
        this.nextStart = 0;
        this.handlers.onDone();
      }
    };
    if (!this.context) {
      this.handlers.onDone();
      return;
    }
    if (this.playbackTimer) clearInterval(this.playbackTimer);
    this.playbackTimer = setInterval(check, 100);
    check();
  }
}

/** Bytes PCM16 de um WAV mono 16 bits (o que `blobToWavDataUri` produz). */
export function wavToPcm16(dataUri: string): Uint8Array {
  const base64 = dataUri.split("base64,")[1] ?? "";
  const wav = base64ToBytes(base64);
  // Procura o bloco "data" em vez de assumir 44 bytes de cabeçalho.
  for (let i = 12; i + 8 <= wav.length; i++) {
    if (wav[i] === 0x64 && wav[i + 1] === 0x61 && wav[i + 2] === 0x74 && wav[i + 3] === 0x61) {
      const size = wav[i + 4] | (wav[i + 5] << 8) | (wav[i + 6] << 16) | (wav[i + 7] << 24);
      return wav.subarray(i + 8, Math.min(wav.length, i + 8 + size));
    }
  }
  throw new Error("WAV sem bloco data");
}

function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}
