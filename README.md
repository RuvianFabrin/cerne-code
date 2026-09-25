# 🪵 Cerne Code

> **Seu agente de código local, livre e sem frescura.**
>
> Cansado de pagar por agente de código que cobra por uso, te prende num ecossistema ou simplesmente não funciona com modelos locais? O **Cerne Code** é um agente de código desktop completo, de graça, que roda **100% local** com qualquer modelo (OpenAI, Claude, Ollama, llama.cpp, LM Studio, Qwen, qualquer um que fale API compatível com OpenAI).

---

### 💾 Como instalar

> ⚠️ **Linux e macOS ainda não foram testados numa máquina real.** O pipeline de build (CI) já gera os instaladores das três plataformas a cada push, mas só o Windows passou por uso de verdade até agora. Trate `.dmg`/`.deb`/`.rpm`/`.AppImage` como "deve funcionar, mas ainda não confirmado".

**Windows**
1. Baixe o instalador `.exe` (NSIS) na seção [📥 Baixar](#-baixar) abaixo.
2. Execute e siga o assistente. Se o SmartScreen do Windows avisar "O Windows protegeu seu PC" (normal em app sem certificado pago), clique em **Mais informações → Executar assim mesmo**.

**macOS** *(não testado)*
1. Baixe o `.dmg` e arraste o Cerne Code pra pasta **Aplicativos**.
2. Como o app não é assinado/notarizado pela Apple, o Gatekeeper vai bloquear a primeira abertura. Clique com o botão **direito** no app → **Abrir** → confirme no aviso. Se continuar bloqueando, rode no Terminal:
   ```bash
   xattr -cr "/Applications/Cerne Code.app"
   ```

**Linux** *(não testado)*

Cada arquivo Linux existe em **duas variantes** — escolha pelo sufixo do nome:

- **`-full`**: tem tudo, incluindo `computer_use` (captura de tela e automação de UI — clicar, digitar, ler a tela). Exige distro mais nova (a lib de PipeWire que esse recurso usa só existe em versões recentes).
- **`-compat`**: roda em mais distros (inclusive mais antigas), mas **sem `computer_use`** — se o agente tentar usar essa função, ela responde com uma mensagem avisando que não está disponível nesta build, em vez de travar o app.

Na dúvida, comece pela `-full`; se o instalador não abrir na sua distro, tente a `-compat`.

- **`.deb`** (Ubuntu/Debian/Mint): `sudo dpkg -i cerne-code_*.deb` — se faltar dependência, `sudo apt --fix-broken install`.
- **`.rpm`** (Fedora/RHEL/openSUSE): `sudo dnf install ./cerne-code-*.rpm` (ou `sudo rpm -i cerne-code-*.rpm`).
- **`.AppImage`** (qualquer distro): precisa marcar como executável antes de rodar — não roda com duplo clique direto na maioria dos gerenciadores de arquivo:
  ```bash
  chmod +x Cerne_Code_*.AppImage
  ./Cerne_Code_*.AppImage
  ```
  Se aparecer erro de sandbox/permissão (comum rodando como root, dentro de container, ou em algumas distros), rode com a flag `--no-sandbox`:
  ```bash
  ./Cerne_Code_*.AppImage --no-sandbox
  ```

Distros compatíveis (glibc/webkit2gtk) estão na tabela em [🐧 Compatibilidade com Linux](#-compatibilidade-com-linux), mais abaixo.

---

### 🚀 Pra que serve

**Cerne Code** é a interface gráfica que transforma qualquer modelo de linguagem num agente de código de verdade:

- **🧠 Edita arquivos** — pede, e ele lê, edita, adiciona funções, refatora. Com sandbox de segurança (você aceita ou rejeita cada alteração antes de aplicar).
- **🔍 Busca no código** — `grep` e `ast_grep` (busca estrutural por AST) pra achar exatamente o que você precisa, não só texto literal.
- **💻 Roda comandos** — executa testes, build, linter, instala dependência, e vê o resultado na hora. Com suporte a comandos em segundo plano (dev server, watch mode) sem travar o chat.
- **🌐 Busca na web** — pesquisa documentação, versões, APIs, e responde com fontes reais. Sem precisar instalar Docker nem chave de API (usa DuckDuckGo + Brave + Mojeek agregados).
- **🧩 Pipeline de qualidade** — pede pro agente implementar algo e ele mesmo verifica com testes reais antes de dizer que ficou pronto. Dev → QA → Analista, ciclo automático.
- **🔌 MCP** — conecta em servidores MCP externos pra expandir as capacidades (banco de dados, APIs, o que você quiser).
- **🔧 Ferramentas Python dinâmicas** — cria ferramentas reutilizáveis em Python que ficam disponíveis em todas as sessões futuras.
- **🎯 Modo Manual** — modo Manual de execução: o agente pede permissão antes de cada ação. Controle total sobre o que roda na sua máquina.
- **🎤 Áudio no composer** — fala diretamente pelo microfone em vez de digitar. O áudio é transcrito automaticamente e vira mensagem de texto.
- **🔊 Ouvir texto** — seleciona qualquer resposta do agente ou texto do chat e ouve em voz alta. Também dá pra ouvir a mensagem inteira com um clique.
- **🧠 Skills & Agents** — skills reutilizáveis que ensinam o agente a fazer tarefas específicas (revisar PR, organizar arquivos, resumir e-mails, etc.). Agents são sessões especializadas com instruções próprias.
- **👤 Personas** — cria personas com jeito próprio de responder. Cada uma com seu tom, regras e conhecimento — o agente assume a persona que você escolher.
- **🔍 Qualquer provedor de busca** — conecta com DuckDuckGo (sem chave, já vem funcionando), Brave Search API, Tavily ou sua própria instância SearXNG. Troca na hora pela tela de configurações.

### 🎯 Pra quem é

| Perfil | Por que baixar |
|--------|----------------|
| **Dev que usa Claude/GPT/web** | Chega de pagar por uso. Roda local, ilimitado, sem contar tokens |
| **Dev que usa Ollama/LM Studio** | Agente de código de verdade que funciona com modelos locais |
| **Quem quer privacidade** | Nada sai da sua máquina. Código, busca, tudo local |
| **Quem cansa de configuração** | Instala e usa. Busca web embutida, sem precisar subir Docker/SearXNG |

---

### ⚡ Comparação rápida

| Característica | Cerne Code | Claude Code CLI |
|----------------|:----------:|:---------------:|
| Interface gráfica ✅ | Sim | CLI + plugin VS Code |
| Roda 100% local | ✅ | ❌ (só cloud) |
| Gratuito | ✅ | ❌ (pago por uso) |
| Suporta modelos locais (Ollama, llama.cpp, LM Studio) | ✅ | ❌ |
| Sandbox de edição (aceitar/rejeitar) | ✅ | ✅ |
| Pipeline automático Dev→QA→Analista | ✅ | ❌ |
| Busca na web sem chave | ✅ | ❌ |
| MCP | ✅ | ✅ |
| Sub-agentes (task) | ✅ | ✅ |
| Windows, Linux, Mac | ✅ | ✅ (CLI) |

---

### 📥 Baixar

👉 **[Baixe a última versão do Cerne Code](https://ruvianfabrin.github.io/cerne-code.html)** (Windows, instalador NSIS)

> 🐧 **Linux:** o suporte está implementado e em fase final de validação — o
> instalador será publicado assim que os testes em VM terminarem. As versões
> compatíveis estão na tabela abaixo.

---

### 🐧 Compatibilidade com Linux

O Cerne Code precisa de **`webkit2gtk-4.1`**, que exige **glib ≥ 2.70**, e é
compilado sobre **glibc 2.35** — por isso algumas distros mais antigas não
conseguem rodar (nem compilar) ele.

**Regra prática:** funciona em qualquer distro com **glibc ≥ 2.35** e
**glib 2.70+**.

| Família | Versões que funcionam | Versões que **não** funcionam |
|---|---|---|
| **Debian / Ubuntu / Mint**<br>(`.deb`, `.AppImage`) | **Ubuntu 22.04 LTS** ou mais novo<br>**Kubuntu / Xubuntu / Lubuntu 22.04+**<br>**Linux Mint 21** ou mais novo<br>**Debian 12** (Bookworm) ou mais novo<br>**Pop!_OS 22.04+** · **Zorin 17+** · **elementary 7+** | Ubuntu 20.04 ❌<br>Linux Mint 20 ❌<br>Debian 11 (Bullseye) ❌ |
| **Fedora / Red Hat**<br>(`.rpm`) | **Fedora 36** ou mais novo<br>**Nobara / Ultramarine** (qualquer versão atual) | Fedora 35 e anteriores ❌<br>RHEL 9 / Rocky 9 / Alma 9 ❌¹ |
| **Arch**<br>(AUR) | **Arch Linux** · **Manjaro** · **EndeavourOS** · **Garuda** (rolling release, sempre atual) | — |
| **openSUSE** | **Tumbleweed** (rolling)<br>**Leap 15.6** ou mais novo | openSUSE Leap 15.5 ❌ |

¹ RHEL 9 e derivados têm glib 2.68 — abaixo do mínimo exigido pelo
`webkit2gtk-4.1`. É uma limitação atual do Tauri v2, não do Cerne Code.

**Não sabe qual é a sua?** Rode no terminal:

```bash
ldd --version | head -1        # precisa ser 2.35 ou maior
pkg-config --modversion glib-2.0   # precisa ser 2.70 ou maior
```

> 💡 **Sua distro não está na lista?** Você ainda pode compilar do código-fonte
> — veja a seção "Rodando" mais abaixo.

---

### 📖 Documentação completa neste README

Abaixo você encontra as instruções pra compilar e rodar o projeto a partir do código-fonte (Windows e Linux).

---

## English

# 🪵 Cerne Code

> **Your local coding agent. Free. No strings attached.**
>
> Tired of coding agents that charge per use, lock you into an ecosystem, or simply don't work with local models? **Cerne Code** is a full-featured desktop coding agent, completely free, running **100% locally** with any model (OpenAI, Claude, Ollama, llama.cpp, LM Studio, Qwen — anything that speaks the OpenAI-compatible API).

---

### 💾 How to install

> ⚠️ **Linux and macOS haven't been tested on real hardware yet.** The build pipeline (CI) already produces installers for all three platforms on every push, but only Windows has seen real-world use so far. Treat `.dmg`/`.deb`/`.rpm`/`.AppImage` as "should work, not yet confirmed."

**Windows**
1. Download the `.exe` installer (NSIS) from the [📥 Download](#-download) section below.
2. Run it and follow the wizard. If Windows SmartScreen warns "Windows protected your PC" (normal for an app without a paid certificate), click **More info → Run anyway**.

**macOS** *(untested)*
1. Download the `.dmg` and drag Cerne Code into **Applications**.
2. Since the app isn't signed/notarized by Apple, Gatekeeper will block the first launch. Right-click the app → **Open** → confirm the prompt. If it's still blocked, run in Terminal:
   ```bash
   xattr -cr "/Applications/Cerne Code.app"
   ```

**Linux** *(untested)*

Every Linux file comes in **two variants** — pick by the filename suffix:

- **`-full`**: has everything, including `computer_use` (screen capture and UI automation — clicking, typing, reading the screen). Requires a newer distro (the PipeWire library this feature needs only exists in recent releases).
- **`-compat`**: runs on more distros (including older ones), but **without `computer_use`** — if the agent tries to use it, it gets a clear "not available in this build" message instead of the app crashing.

When in doubt, start with `-full`; if the installer won't run on your distro, try `-compat`.

- **`.deb`** (Ubuntu/Debian/Mint): `sudo dpkg -i cerne-code_*.deb` — if a dependency is missing, `sudo apt --fix-broken install`.
- **`.rpm`** (Fedora/RHEL/openSUSE): `sudo dnf install ./cerne-code-*.rpm` (or `sudo rpm -i cerne-code-*.rpm`).
- **`.AppImage`** (any distro): needs to be marked executable before it'll run — double-clicking usually won't work in most file managers:
  ```bash
  chmod +x Cerne_Code_*.AppImage
  ./Cerne_Code_*.AppImage
  ```
  If you hit a sandbox/permission error (common when running as root, inside a container, or on some distros), run it with the `--no-sandbox` flag:
  ```bash
  ./Cerne_Code_*.AppImage --no-sandbox
  ```

Supported distros (glibc/webkit2gtk) are in the table under [🐧 Linux compatibility](#-linux-compatibility) below.

---

### 🚀 What it does

**Cerne Code** is the GUI that turns any language model into a real coding agent:

- **🧠 Edits files** — ask and it reads, edits, adds functions, refactors. With a safety sandbox (you approve or reject every change before it's applied).
- **🔍 Searches code** — `grep` and `ast_grep` (AST-based structural search) to find exactly what you need, not just literal text.
- **💻 Runs commands** — executes tests, builds, linters, installs dependencies, and shows results in real time. Supports background commands (dev server, watch mode) without blocking the chat.
- **🌐 Web search** — looks up documentation, versions, APIs, and answers with real sources. No Docker or API key required (uses aggregated DuckDuckGo + Brave + Mojeek).
- **🧩 Quality pipeline** — ask the agent to implement something and it verifies with real tests before claiming it's done. Dev → QA → Analyst, automatic cycle.
- **🔌 MCP** — connects to external MCP servers to expand capabilities (databases, APIs, anything).
- **🔧 Dynamic Python tools** — creates reusable Python tools that become available in every future session.
- **🎯 Manual mode** — the agent asks permission before every action. Full control over what runs on your machine.
- **🎤 Audio input** — speak directly through the mic instead of typing. Audio is transcribed to text automatically.
- **🔊 Text-to-speech** — select any agent response or chat text and hear it spoken aloud. Or listen to the entire message with one click.
- **🧠 Skills & Agents** — reusable skills that teach the agent specific tasks (PR review, file organizing, email triage, etc.). Agents are specialized sessions with their own instructions.
- **👤 Personas** — create personas with their own tone, rules, and knowledge. The agent takes on the persona you choose.
- **🔍 Any search provider** — connect with DuckDuckGo (no key, works out of the box), Brave Search API, Tavily, or your own SearXNG instance. Switch anytime from settings.

### 🎯 Who it's for

| Profile | Why download |
|---------|-------------|
| **Dev using Claude/GPT/cloud** | Stop paying per use. Run locally, unlimited, no token counting |
| **Dev using Ollama/LM Studio** | A real coding agent that works with local models |
| **Privacy-conscious** | Nothing leaves your machine. Code, search, everything local |
| **Tired of setup** | Install and use. Built-in web search, no Docker/SearXNG needed |

### ⚡ Quick comparison

| Feature | Cerne Code | Claude Code CLI |
|---------|:----------:|:---------------:|
| GUI ✅ | Yes | CLI + VS Code plugin |
| 100% local | ✅ | ❌ (cloud only) |
| Free | ✅ | ❌ (pay per use) |
| Supports local models (Ollama, llama.cpp, LM Studio) | ✅ | ❌ |
| Edit sandbox (approve/reject) | ✅ | ✅ |
| Auto Dev→QA→Analyst pipeline | ✅ | ❌ |
| Web search without API key | ✅ | ❌ |
| MCP | ✅ | ✅ |
| Sub-agents (task) | ✅ | ✅ |
| Windows, Linux, Mac | ✅ | ✅ (CLI) |

### 📥 Download

👉 **[Download the latest Cerne Code](https://ruvianfabrin.github.io/cerne-code.html)** (Windows, NSIS installer)

> 🐧 **Linux:** support is implemented and in final validation — the installer
> will be published as soon as VM testing is done. Compatible distros below.

---

### 🐧 Linux compatibility

Cerne Code needs **`webkit2gtk-4.1`**, which requires **glib ≥ 2.70**, and is
built against **glibc 2.35** — that's why older distros can't run (or build) it.

**Rule of thumb:** works on any distro with **glibc ≥ 2.35** and **glib 2.70+**.

| Family | Supported versions | **Not** supported |
|---|---|---|
| **Debian / Ubuntu / Mint**<br>(`.deb`, `.AppImage`) | **Ubuntu 22.04 LTS** or newer<br>**Kubuntu / Xubuntu / Lubuntu 22.04+**<br>**Linux Mint 21** or newer<br>**Debian 12** (Bookworm) or newer<br>**Pop!_OS 22.04+** · **Zorin 17+** · **elementary 7+** | Ubuntu 20.04 ❌<br>Linux Mint 20 ❌<br>Debian 11 (Bullseye) ❌ |
| **Fedora / Red Hat**<br>(`.rpm`) | **Fedora 36** or newer<br>**Nobara / Ultramarine** (any current release) | Fedora 35 and older ❌<br>RHEL 9 / Rocky 9 / Alma 9 ❌¹ |
| **Arch**<br>(AUR) | **Arch Linux** · **Manjaro** · **EndeavourOS** · **Garuda** (rolling release, always current) | — |
| **openSUSE** | **Tumbleweed** (rolling)<br>**Leap 15.6** or newer | openSUSE Leap 15.5 ❌ |

¹ RHEL 9 and derivatives ship glib 2.68 — below the minimum required by
`webkit2gtk-4.1`. This is a current limitation of Tauri v2, not of Cerne Code.

**Not sure about yours?** Run:

```bash
ldd --version | head -1            # needs to be 2.35 or higher
pkg-config --modversion glib-2.0   # needs to be 2.70 or higher
```

> 💡 **Your distro isn't listed?** You can still build from source — see the
> "Rodando" section below.

---

## Español

# 🪵 Cerne Code

> **Tu agente de código local, gratis y sin vueltas.**
>
> ¿Cansado de pagar por un agente de código que cobra por uso, te ata a un ecosistema o simplemente no funciona con modelos locales? **Cerne Code** es un agente de código desktop completo, gratuito, que funciona **100% local** con cualquier modelo (OpenAI, Claude, Ollama, llama.cpp, LM Studio, Qwen — cualquiera que hable la API compatible con OpenAI).

---

### 💾 Cómo instalar

> ⚠️ **Linux y macOS todavía no fueron probados en una máquina real.** El pipeline de build (CI) ya genera los instaladores de las tres plataformas en cada push, pero solo Windows tuvo uso real hasta ahora. Trata `.dmg`/`.deb`/`.rpm`/`.AppImage` como "debería funcionar, todavía no confirmado".

**Windows**
1. Descarga el instalador `.exe` (NSIS) en la sección [📥 Descargar](#-descargar) más abajo.
2. Ejecútalo y sigue el asistente. Si el SmartScreen de Windows avisa "Windows protegió tu PC" (normal en apps sin certificado pago), haz clic en **Más información → Ejecutar de todas formas**.

**macOS** *(no probado)*
1. Descarga el `.dmg` y arrastra Cerne Code a la carpeta **Aplicaciones**.
2. Como la app no está firmada/notarizada por Apple, Gatekeeper va a bloquear la primera apertura. Haz clic derecho en la app → **Abrir** → confirma el aviso. Si sigue bloqueada, ejecuta en Terminal:
   ```bash
   xattr -cr "/Applications/Cerne Code.app"
   ```

**Linux** *(no probado)*

Cada archivo de Linux viene en **dos variantes** — elige por el sufijo del nombre:

- **`-full`**: tiene todo, incluyendo `computer_use` (captura de pantalla y automatización de UI — clic, escritura, lectura de pantalla). Necesita una distro más reciente (la librería de PipeWire que usa esta función solo existe en versiones recientes).
- **`-compat`**: funciona en más distros (incluso más antiguas), pero **sin `computer_use`** — si el agente intenta usarla, devuelve un mensaje claro de "no disponible en esta build" en vez de que la app falle.

Si tienes dudas, empieza con `-full`; si el instalador no abre en tu distro, prueba `-compat`.

- **`.deb`** (Ubuntu/Debian/Mint): `sudo dpkg -i cerne-code_*.deb` — si falta una dependencia, `sudo apt --fix-broken install`.
- **`.rpm`** (Fedora/RHEL/openSUSE): `sudo dnf install ./cerne-code-*.rpm` (o `sudo rpm -i cerne-code-*.rpm`).
- **`.AppImage`** (cualquier distro): necesita permiso de ejecución antes de correr — hacer doble clic no suele funcionar en la mayoría de los gestores de archivos:
  ```bash
  chmod +x Cerne_Code_*.AppImage
  ./Cerne_Code_*.AppImage
  ```
  Si aparece un error de sandbox/permisos (común corriendo como root, dentro de un contenedor, o en algunas distros), ejecuta con la opción `--no-sandbox`:
  ```bash
  ./Cerne_Code_*.AppImage --no-sandbox
  ```

Las distros compatibles (glibc/webkit2gtk) están en la tabla de [🐧 Compatibilidad con Linux](#-compatibilidad-con-linux), más abajo.

---

### 🚀 Para qué sirve

**Cerne Code** es la interfaz gráfica que convierte cualquier modelo de lenguaje en un agente de código real:

- **🧠 Edita archivos** — pídele y lee, edita, agrega funciones, refactoriza. Con un sandbox de seguridad (aceptas o rechazas cada cambio antes de aplicarlo).
- **🔍 Busca en el código** — `grep` y `ast_grep` (búsqueda estructural por AST) para encontrar exactamente lo que necesitas, no solo texto literal.
- **💻 Ejecuta comandos** — corre tests, builds, linters, instala dependencias y ve el resultado al instante. Con soporte para comandos en segundo plano (dev server, watch mode) sin bloquear el chat.
- **🌐 Búsqueda web** — investiga documentación, versiones, APIs y responde con fuentes reales. Sin necesidad de Docker ni clave de API (usa DuckDuckGo + Brave + Mojeek agregados).
- **🧩 Pipeline de calidad** — pídele al agente que implemente algo y él mismo verifica con tests reales antes de decir que está listo. Dev → QA → Analista, ciclo automático.
- **🔌 MCP** — se conecta a servidores MCP externos para expandir capacidades (bases de datos, APIs, lo que quieras).
- **🔧 Herramientas Python dinámicas** — crea herramientas reutilizables en Python que quedan disponibles en todas las sesiones futuras.
- **🎯 Modo Manual** — el agente pide permiso antes de cada acción. Control total sobre lo que se ejecuta en tu máquina.
- **🎤 Audio en el compositor** — habla directamente por el micrófono en vez de escribir. El audio se transcribe automáticamente a texto.
- **🔊 Escuchar texto** — selecciona cualquier respuesta del agente o texto del chat y escúchalo en voz alta. También puedes oír el mensaje completo con un clic.
- **🧠 Skills & Agents** — skills reutilizables que le enseñan al agente tareas específicas (revisar PR, organizar archivos, resumir correos, etc.). Los agents son sesiones especializadas con instrucciones propias.
- **👤 Personas** — crea personas con su propio tono, reglas y conocimiento. El agente adopta la persona que elijas.
- **🔍 Cualquier proveedor de búsqueda** — conecta con DuckDuckGo (sin clave, funciona de fábrica), Brave Search API, Tavily o tu propia instancia SearXNG. Cámbialo al instante desde configuración.

### 🎯 Para quién es

| Perfil | Por qué descargar |
|--------|-------------------|
| **Dev que usa Claude/GPT/nube** | Deja de pagar por uso. Corre local, ilimitado, sin contar tokens |
| **Dev que usa Ollama/LM Studio** | Un agente de código real que funciona con modelos locales |
| **Quien valora la privacidad** | Nada sale de tu máquina. Código, búsqueda, todo local |
| **Hartos de configuraciones** | Instala y usa. Búsqueda web incorporada, sin Docker/SearXNG |

### ⚡ Comparación rápida

| Característica | Cerne Code | Claude Code CLI |
|----------------|:----------:|:---------------:|
| Interfaz gráfica ✅ | Sí | CLI + plugin VS Code |
| 100% local | ✅ | ❌ (solo nube) |
| Gratuito | ✅ | ❌ (pago por uso) |
| Soporta modelos locales (Ollama, llama.cpp, LM Studio) | ✅ | ❌ |
| Sandbox de edición (aceptar/rechazar) | ✅ | ✅ |
| Pipeline automático Dev→QA→Analista | ✅ | ❌ |
| Búsqueda web sin clave API | ✅ | ❌ |
| MCP | ✅ | ✅ |
| Sub-agentes (task) | ✅ | ✅ |
| Windows, Linux, Mac | ✅ | ✅ (CLI) |

### 📥 Descargar

👉 **[Descarga la última versión de Cerne Code](https://ruvianfabrin.github.io/cerne-code.html)** (Windows, instalador NSIS)

> 🐧 **Linux:** el soporte está implementado y en validación final — el instalador se publicará cuando terminen las pruebas en máquina virtual. Las versiones compatibles están en la tabla de abajo.

---

### 🐧 Compatibilidad con Linux

Cerne Code necesita **`webkit2gtk-4.1`**, que exige **glib ≥ 2.70**, y se compila sobre **glibc 2.35** — por eso algunas distros más antiguas no pueden ejecutarlo (ni compilarlo).

**Regla práctica:** funciona en cualquier distro con **glibc ≥ 2.35** y **glib 2.70+**.

| Familia | Versiones que funcionan | Versiones que **no** funcionan |
|---|---|---|
| **Debian / Ubuntu / Mint**<br>(`.deb`, `.AppImage`) | **Ubuntu 22.04 LTS** o más reciente<br>**Kubuntu / Xubuntu / Lubuntu 22.04+**<br>**Linux Mint 21** o más reciente<br>**Debian 12** (Bookworm) o más reciente<br>**Pop!_OS 22.04+** · **Zorin 17+** · **elementary 7+** | Ubuntu 20.04 ❌<br>Linux Mint 20 ❌<br>Debian 11 (Bullseye) ❌ |
| **Fedora / Red Hat**<br>(`.rpm`) | **Fedora 36** o más reciente<br>**Nobara / Ultramarine** (cualquier versión actual) | Fedora 35 y anteriores ❌<br>RHEL 9 / Rocky 9 / Alma 9 ❌¹ |
| **Arch**<br>(AUR) | **Arch Linux** · **Manjaro** · **EndeavourOS** · **Garuda** (rolling release, siempre al día) | — |
| **openSUSE** | **Tumbleweed** (rolling)<br>**Leap 15.6** o más reciente | openSUSE Leap 15.5 ❌ |

¹ RHEL 9 y derivados traen glib 2.68 — por debajo del mínimo que exige `webkit2gtk-4.1`. Es una limitación actual de Tauri v2, no de Cerne Code.

**¿No sabes cuál es la tuya?** Ejecuta:

```bash
ldd --version | head -1            # necesita ser 2.35 o mayor
pkg-config --modversion glib-2.0   # necesita ser 2.70 o mayor
```

> 💡 **¿Tu distro no está en la lista?** Todavía puedes compilar desde el código fuente — ver la sección "Rodando" más abajo.

---

## 中文

# 🪵 Cerne Code

> **你的本地代码助手，完全免费，没有套路。**
>
> 厌倦了按使用次数收费、把你锁在某个生态系统里、或者干脆无法与本地模型配合的代码助手？**Cerne Code** 是一款功能完整的桌面代码助手，完全免费，**100% 本地运行**，支持任何模型（OpenAI、Claude、Ollama、llama.cpp、LM Studio、Qwen —— 任何兼容 OpenAI API 的模型都可以）。

---

### 💾 如何安装

> ⚠️ **Linux 和 macOS 版本尚未在真实设备上测试过。** 构建流水线（CI）已经会在每次 push 时自动生成三个平台的安装包，但目前只有 Windows 版经过实际使用验证。请把 `.dmg`/`.deb`/`.rpm`/`.AppImage` 当作"理论上可用，但尚未确认"。

**Windows**
1. 在下方的 [📥 下载](#-下载) 部分获取 `.exe` 安装程序（NSIS）。
2. 运行并按向导操作。如果 Windows SmartScreen 提示"Windows 已保护你的电脑"（没有付费证书的应用很常见），点击 **更多信息 → 仍要运行**。

**macOS**（未测试）
1. 下载 `.dmg`，把 Cerne Code 拖到 **应用程序（Applications）** 文件夹。
2. 由于应用没有经过 Apple 签名/公证，Gatekeeper 会在首次打开时阻止运行。右键点击应用 → **打开** → 确认提示。如果仍被阻止，在终端运行：
   ```bash
   xattr -cr "/Applications/Cerne Code.app"
   ```

**Linux**（未测试）

每个 Linux 安装包都有**两个版本**——按文件名后缀区分：

- **`-full`**：功能完整，包含 `computer_use`（屏幕截图和 UI 自动化——点击、输入、读取屏幕）。需要较新的发行版（这个功能依赖的 PipeWire 库只有新版本才有）。
- **`-compat`**：兼容更多发行版（包括较旧的），但**不包含 `computer_use`**——如果智能体尝试调用它，会收到"此版本不支持"的清晰提示，而不是应用崩溃。

不确定选哪个就先试 `-full`；如果装不上，再试 `-compat`。

- **`.deb`**（Ubuntu/Debian/Mint）：`sudo dpkg -i cerne-code_*.deb` —— 如果缺少依赖，运行 `sudo apt --fix-broken install`。
- **`.rpm`**（Fedora/RHEL/openSUSE）：`sudo dnf install ./cerne-code-*.rpm`（或 `sudo rpm -i cerne-code-*.rpm`）。
- **`.AppImage`**（任意发行版）：运行前需要先赋予可执行权限 —— 大多数文件管理器里直接双击是不会启动的：
  ```bash
  chmod +x Cerne_Code_*.AppImage
  ./Cerne_Code_*.AppImage
  ```
  如果出现沙箱/权限相关的报错（常见于以 root 运行、在容器内运行，或某些发行版），加上 `--no-sandbox` 参数再运行：
  ```bash
  ./Cerne_Code_*.AppImage --no-sandbox
  ```

兼容的发行版（glibc/webkit2gtk）见下方 [🐧 Linux 兼容性](#-linux-兼容性) 表格。

---

### 🚀 它能做什么

**Cerne Code** 是一款图形界面工具，能把任何语言模型变成真正的代码助手：

- **🧠 编辑文件** —— 让它读取、编辑、添加函数、重构代码。带有安全沙箱（在应用之前，你可以批准或拒绝每一项更改）。
- **🔍 搜索代码** —— `grep` 和 `ast_grep`（基于 AST 的结构化搜索），能精确找到你需要的内容，而不只是逐字匹配。
- **💻 运行命令** —— 执行测试、构建、代码检查、安装依赖，并实时显示结果。支持后台命令（开发服务器、监听模式），不会阻塞对话。
- **🌐 网页搜索** —— 查找文档、版本信息、API，并提供真实来源的回答。无需安装 Docker 或 API 密钥（聚合 DuckDuckGo + Brave + Mojeek）。
- **🧩 质量流水线** —— 让助手实现某个功能，它会用真实测试来验证，然后才报告完成。开发 → 质量检查 → 分析师，自动循环。
- **🔌 MCP** —— 连接外部 MCP 服务器以扩展功能（数据库、API，任何你需要的）。
- **🔧 动态 Python 工具** —— 创建可复用的 Python 工具，这些工具将在所有未来的会话中可用。
- **🎯 手动模式** —— 助手在每次操作前都会请求许可。完全控制在你机器上执行的内容。
- **🎤 语音输入** —— 直接对着麦克风说话，不用打字。语音会自动转录为文字。
- **🔊 朗读文本** —— 选中助手的任何回复或聊天文本，即可听语音朗读。也可以一键收听整条消息。
- **🧠 Skills & Agents** —— 可复用的技能，教会助手执行特定任务（审查 PR、整理文件、分类邮件等）。Agents 是带有专属指令的专门会话。
- **👤 人设 (Personas)** —— 创建拥有自己语气、规则和知识的人设。助手会采用你选择的人设来回应。
- **🔍 任意搜索引擎** —— 连接 DuckDuckGo（无需密钥，开箱即用）、Brave Search API、Tavily 或你自己的 SearXNG 实例。随时在设置中切换。

### 🎯 适合谁

| 用户画像 | 为什么下载 |
|----------|-----------|
| **使用 Claude/GPT/云端 的开发者** | 停止按使用付费。本地运行，无限制，不计 token |
| **使用 Ollama/LM Studio 的开发者** | 一个能与本地模型配合的真正代码助手 |
| **注重隐私的用户** | 没有任何数据离开你的机器。代码、搜索，全部本地化 |
| **厌倦配置的用户** | 安装即可使用。内置网页搜索，无需 Docker/SearXNG |

### ⚡ 快速对比

| 特性 | Cerne Code | Claude Code CLI |
|------|:----------:|:---------------:|
| 图形界面 ✅ | 是 | CLI + VS Code 插件 |
| 100% 本地运行 | ✅ | ❌（仅云端） |
| 免费 | ✅ | ❌（按使用付费） |
| 支持本地模型 (Ollama, llama.cpp, LM Studio) | ✅ | ❌ |
| 编辑沙箱 (批准/拒绝) | ✅ | ✅ |
| 自动 Dev→QA→分析师 流水线 | ✅ | ❌ |
| 无需 API 密钥的网页搜索 | ✅ | ❌ |
| MCP | ✅ | ✅ |
| 子代理 (task) | ✅ | ✅ |
| Windows, Linux, Mac | ✅ | ✅ (CLI) |

### 📥 下载

👉 **[下载 Cerne Code 最新版本](https://ruvianfabrin.github.io/cerne-code.html)**（Windows，NSIS 安装程序）

> 🐧 **Linux：**支持已实现，正在进行最后验证 — 虚拟机测试完成后即发布安装包。兼容版本见下表。

---

### 🐧 Linux 兼容性

Cerne Code 需要 **`webkit2gtk-4.1`**，而它要求 **glib ≥ 2.70**，并且基于 **glibc 2.35** 构建 — 所以某些较旧的发行版无法运行（也无法编译）。

**简单规则：** 任何满足 **glibc ≥ 2.35** 和 **glib 2.70+** 的发行版都可以。

| 系列 | 支持的版本 | **不**支持的版本 |
|---|---|---|
| **Debian / Ubuntu / Mint**<br>(`.deb`, `.AppImage`) | **Ubuntu 22.04 LTS** 或更新<br>**Kubuntu / Xubuntu / Lubuntu 22.04+**<br>**Linux Mint 21** 或更新<br>**Debian 12** (Bookworm) 或更新<br>**Pop!_OS 22.04+** · **Zorin 17+** · **elementary 7+** | Ubuntu 20.04 ❌<br>Linux Mint 20 ❌<br>Debian 11 (Bullseye) ❌ |
| **Fedora / Red Hat**<br>(`.rpm`) | **Fedora 36** 或更新<br>**Nobara / Ultramarine**（任何当前版本） | Fedora 35 及更旧 ❌<br>RHEL 9 / Rocky 9 / Alma 9 ❌¹ |
| **Arch**<br>(AUR) | **Arch Linux** · **Manjaro** · **EndeavourOS** · **Garuda**（滚动发布，始终最新） | — |
| **openSUSE** | **Tumbleweed**（滚动）<br>**Leap 15.6** 或更新 | openSUSE Leap 15.5 ❌ |

¹ RHEL 9 及其衍生版本携带 glib 2.68 — 低于 `webkit2gtk-4.1` 所需的最低版本。这是 Tauri v2 的当前限制，不是 Cerne Code 的问题。

**不知道自己的版本？** 运行：

```bash
ldd --version | head -1            # 需要 2.35 或更高
pkg-config --modversion glib-2.0   # 需要 2.70 或更高
```

> 💡 **你的发行版不在列表中？** 依然可以从源码编译 — 见下方 "Rodando" 部分。

---

### 📖 Documentação completa neste README (Português)

Abaixo você encontra as instruções pra compilar e rodar o projeto a partir do código-fonte.

## Rodando

### Windows

```powershell
cd C:\cerne
npm install          # se ainda não rodou
npm run tauri dev    # janela nativa, hot-reload do frontend
```

### Linux

Instale as dependências de sistema (nomes do Debian/Ubuntu — em Fedora/Arch
use o equivalente):

```bash
sudo apt install build-essential curl wget file libssl-dev \
  libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev \
  librsvg2-dev libsecret-1-0 gnome-keyring libpipewire-0.3-dev libdbus-1-dev \
  libclang-dev libgbm-dev libwayland-dev libxkbcommon-dev libegl1-mesa-dev \
  libdrm-dev
```

> O `libsecret` + `gnome-keyring` são necessários pro cofre de chaves (onde a
> chave de API é guardada). Sem eles o app abre, mas não consegue salvar a chave.
> `libpipewire-0.3-dev`, `libdbus-1-dev` e `libclang-dev` são necessários só
> pra **compilar** (captura de tela, cofre de chaves via Secret Service e
> geração de bindings via `bindgen` linkam contra eles em tempo de build) —
> sem essas três o `cargo build`/`npm run tauri build` nem sai do lugar.

Depois:

```bash
git clone https://github.com/RuvianFabrin/cerne-code.git
cd cerne-code
npm install
npm run tauri dev      # desenvolvimento (hot-reload)
npm run tauri build    # gera o instalador (.deb / .rpm / .AppImage)
```

> ⚠️ **Para gerar um `.deb`/`.AppImage` que rode em distros mais antigas**,
> compile dentro de um **Ubuntu 22.04** (container ou VM). Compilando em
> Ubuntu 24.04+ o binário resultante exige glibc mais nova e **não abre** em
> 22.04 / Mint 21 / Debian 12.

`npm run dev` sozinho sobe só o Vite (útil pra iterar na UI sem recompilar
Rust, mas sem os comandos IPC — o app fica sem dados reais nesse modo).

