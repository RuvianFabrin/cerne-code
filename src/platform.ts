/**
 * Detecção de plataforma — PONTO ÚNICO DE VERDADE.
 *
 * Regra do projeto (Tarefa 4.0 do PLANOS/port_linux_macos.md): nenhum
 * componente faz regex de userAgent sozinho; todos importam daqui. Se um dia
 * a detecção mudar (ex.: comando Tauri `get_platform`), muda só este arquivo.
 */

export type Platform = "windows" | "macos" | "linux" | "unknown";

let cached: Platform | null = null;

export function getPlatform(): Platform {
  if (cached) return cached;
  const ua =
    navigator.userAgent ??
    (navigator as unknown as { userAgentData?: { platform?: string } })
      .userAgentData?.platform ??
    "";
  if (/Windows/i.test(ua)) cached = "windows";
  else if (/Macintosh|Mac OS X|MacIntel/i.test(ua)) cached = "macos";
  else if (/Linux|X11|CrOS/i.test(ua)) cached = "linux";
  else cached = "unknown";
  return cached;
}

export function isWindows(): boolean {
  return getPlatform() === "windows";
}

export function isMacos(): boolean {
  return getPlatform() === "macos";
}

/** Unix-like: Linux + macOS (o WebView do Tauri reporta Linux no Android também). */
export function isUnixLike(): boolean {
  const p = getPlatform();
  return p === "linux" || p === "macos";
}

/**
 * Extensões pro filtro do file picker de executáveis. No Windows filtramos
 * `.exe` (conveniência real); fora dele não há extensão de executável —
 * retornamos null pra NÃO passar filtro nenhum ao dialog (aceita qualquer
 * arquivo). Ver Settings.vue (browse de fork do llama.cpp).
 */
export function executablePickerExtensions(): string[] | null {
  return isWindows() ? ["exe"] : null;
}

/** Placeholder neutro pro nome do binário do llama-server por SO. */
export function llamaServerPlaceholder(): string {
  return isWindows() ? "llama-server.exe..." : "llama-server...";
}
