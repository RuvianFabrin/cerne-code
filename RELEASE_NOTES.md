<!--
  Texto das novidades da PRÓXIMA release, nas 4 línguas do projeto.
  Edite este arquivo, faça commit, e SÓ DEPOIS crie a tag (ex.: v0.1.3) e
  dê push nela. O workflow "Release" (.github/workflows/release.yml) lê
  este arquivo no momento do push da tag e usa o conteúdo abaixo como
  texto da release no GitHub.

  Não precisa apagar este comentário - comentário HTML não aparece na
  release publicada, só no markdown-fonte.
-->

## 🇧🇷 Português

**Novidade grande:** o Cerne Code agora roda em **Linux e macOS**, além do Windows.

- 🐧 **Linux e 🍎 macOS**: port completo do agente. Linux ganhou duas variantes de instalador — `-full` (com captura de tela/automação de UI) e `-compat` (sem esse recurso, mas roda em mais distros, inclusive mais antigas). **Ainda não testados numa máquina real** — só passaram pelo build automático.
- ⚙️ **Esteira de build automática**: instaladores gerados sozinhos a cada atualização, pros três sistemas operacionais.
- 🎨 Configurações redesenhadas em dois painéis (navegação + conteúdo).
- 🖼️ Imagem anexada é redimensionada automaticamente antes de mandar pro modelo.
- 🔤 Fontes embutidas no app + zoom/tamanho de texto ajustável.
- 🐛 Correções: sessão não trava mais quando fica um `tool_call` órfão no histórico; medidor de contexto agora usa a contagem real de tokens do provedor; acento quebrado e cores ANSI na saída de comandos corrigidos; cor do balão de mensagem do usuário ajustada.

## 🇺🇸 English

**Big news:** Cerne Code now runs on **Linux and macOS**, in addition to Windows.

- 🐧 **Linux and 🍎 macOS**: full agent port. Linux ships two installer variants — `-full` (with screen capture/UI automation) and `-compat` (without it, but runs on more distros, including older ones). **Not tested on real hardware yet** — only passed the automated build.
- ⚙️ **Automated build pipeline**: installers generated automatically on every update, for all three operating systems.
- 🎨 Redesigned settings screen with two panels (navigation + content).
- 🖼️ Attached images are automatically resized before being sent to the model.
- 🔤 Bundled fonts + adjustable text size/zoom.
- 🐛 Fixes: sessions no longer hang on an orphaned `tool_call` in history; context meter now uses the provider's real token count; broken accents and ANSI colors in command output fixed; user message bubble color adjusted.

## 🇪🇸 Español

**Gran novedad:** Cerne Code ahora funciona en **Linux y macOS**, además de Windows.

- 🐧 **Linux y 🍎 macOS**: port completo del agente. Linux tiene dos variantes de instalador — `-full` (con captura de pantalla/automatización de UI) y `-compat` (sin eso, pero funciona en más distros, incluso más antiguas). **Todavía no probados en una máquina real** — solo pasaron el build automático.
- ⚙️ **Pipeline de build automático**: instaladores generados solos en cada actualización, para los tres sistemas operativos.
- 🎨 Pantalla de configuración rediseñada en dos paneles (navegación + contenido).
- 🖼️ Las imágenes adjuntas se redimensionan automáticamente antes de enviarlas al modelo.
- 🔤 Fuentes integradas + tamaño de texto/zoom ajustable.
- 🐛 Correcciones: las sesiones ya no se traban con un `tool_call` huérfano en el historial; el medidor de contexto ahora usa el conteo real de tokens del proveedor; acentos rotos y colores ANSI en la salida de comandos corregidos; color de la burbuja de mensaje del usuario ajustado.

## 🇨🇳 中文

**重大更新：** Cerne Code 现已支持 **Linux 和 macOS**，不再局限于 Windows。

- 🐧 **Linux 和 🍎 macOS**：完整移植。Linux 提供两种安装包 —— `-full`（含屏幕截图/UI 自动化）和 `-compat`（不含该功能，但兼容更多发行版，包括较旧的）。**尚未在真实设备上测试**，目前只通过了自动构建。
- ⚙️ **自动构建流水线**：每次更新都会自动为三个操作系统生成安装包。
- 🎨 重新设计的设置界面，采用双栏布局（导航 + 内容）。
- 🖼️ 附加的图片会在发送给模型前自动调整大小。
- 🔤 内置字体，支持调整文字大小/缩放。
- 🐛 修复：历史记录中出现孤立的 `tool_call` 时会话不再卡死；上下文计量改用服务商返回的真实 token 数；修复命令输出中的乱码和 ANSI 颜色问题；调整了用户消息气泡的颜色。
