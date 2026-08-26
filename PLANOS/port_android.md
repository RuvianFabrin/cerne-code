# Port do Cerne Code para Android (.apk)

> Levantamento feito em 2026-08-22, com pesquisa na web + leitura do
> repositório (sem editar código). Complementa `port_linux_macos.md`.
> Conclusão em uma linha: **o app abre no Android, mas ele NÃO é um celular —
> o núcleo de agente de código sobrevive parcialmente; shell/computer_use/
> processos locais não existem lá por limitação do SO.**

---

## 0. Resumo executivo

Tauri 2 suporta Android oficialmente (estável desde out/2024), mas o modelo é
diferente: o Rust roda dentro do app Android via JNI, a UI roda no WebView do
sistema (Android System WebView / Chrome) e **não existe "shell" nem
"desktop"**. Muitas capacidades que o Cerne assume (spawnar `pwsh`/`/bin/sh`,
matar árvore de processos com `taskkill`, enigo/xcap, abrir pasta no
explorador) simplesmente não têm equivalente.

| Bloco | Status no Android | Motivo |
|---|---|---|
| Compilar (`cargo`/`tauri android build`) | 🟡 Trabalho de setup real (NDK, targets, cfgs novos) | ver Fase A |
| Frontend Vue | ✅ Deve funcionar quase sem mudança | WebView moderno; já é tudo via Tauri IPC |
| Chat/provedores HTTP (OpenRouter etc.) | ✅ Funciona | `reqwest` funciona em Android |
| Sessões/config/persistência JSON | ✅ Funciona | arquivos locais do app |
| Git | 🟡 Só se git existir no device (Termux) ou via libgit2 | não há `git` garantido no PATH |
| MCP stdio | 🔴 Quebrado (spawn de processo) | sem shell/binários garantidos |
| MCP HTTP/streamable | ✅ Funciona | rede pura |
| run_command/background jobs | 🔴 Sem sentido no mobile | não há terminal do sistema |
| computer_use (enigo/xcap/UIA) | 🔴 Impossível | Android não dá injeção de input global nem screenshot de outros apps |
| llama.cpp local | 🔴 Inviável na prática | binário desktop + RAM/VRAM |
| keyring crate | 🔴 Sem backend Android nativo na v3 atual | issue #127 fechada sem suporte; alternativas abaixo |
| Ferramentas Python (uv) | 🔴 Inviável | uv não roda no Android padrão |

**Estratégia recomendada:** tratar Android como um **cliente "chat +
revisão de código + monitoramento de sessões desktop"**, não como um Cerne
completo. As ferramentas de agente que dependem de SO ficam desabilitadas
por capability/flag, e o frontend adapta a UI (já existe infra nenhuma de
detecção de plataforma — Tarefa 4.0 do plano Linux/macOS vira pré-requisito).

---

## 1. Pré-requisitos de build (o que instalar)

Seguem o guia oficial do Tauri 2 para Android
(https://v2.tauri.app/start/prerequisites/, https://v2.tauri.app/develop/android/):

1. **Rust** com targets Android:
   ```
   rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
   ```
   (aarch64 = celulares modernos; os outros cobrem emuladores/dispositivos antigos)
2. **Android Studio** (ou só as cmdline-tools) com:
   - Android SDK Platform (API mínimo suportado pelo Tauri: 24)
   - Android SDK Platform-Tools
   - **Android NDK** (obrigatório pra compilar Rust)
   - aceitar licenças: `sdkmanager --licenses`
3. **Java JDK 17** (vem com o Android Studio)
4. Variáveis de ambiente (PowerShell, exemplo):
   ```powershell
   $env:NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\<versao>"
   $env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
   ```
5. No projeto:
   ```
   npm install -D @tauri-apps/cli
   npx tauri android init      # gera src-tauri/gen/android
   npx tauri android dev       # roda no emulador/device com hot reload
   npx tauri android build --apk     # .apk de teste (debug assinado)
   npx tauri android build           # .aab pra Play Store
   ```

**Aviso realista:** `tauri android init` usa o cargo-mobile2, que é mantido
"apenas pro caso de uso do Tauri" (https://github.com/tauri-apps/cargo-mobile2)
— falhas de init com mensagens vagas são comuns (issues #13930, #10065 do
tauri). Espere ajustar versão de NDK/Gradle manualmente na primeira vez.

---

## 2. Fase A — Compilar o projeto atual pra Android

> O código atual NUNCA foi compilado pra target Android. Isto é o que falta:

### Tarefa A.1 — Cargo.toml: dependências que não compilam em Android

- [ ] `keyring = { features = ["windows-native"] }` → sem backend Android.
      Opções (decidir):
      a. `cfg(any(windows, target_os = "linux", target_os = "macos"))` no uso,
         com stub de erro claro em Android;
      b. migrar segredo pra arquivo cifrado (ex.: chave derivada de um PIN);
      c. plugin comunitário `tauri-plugin-keystore` (Android Keystore /
         iOS Keychain — https://github.com/impierce/tauri-plugin-keystore).
      Referência: keyring-rs não suporta Android
      (https://github.com/open-source-cooperative/keyring-rs/issues).
- [ ] `windows`/`uiautomation` já estão atrás de `cfg(windows)` ✅ (nada a fazer).
- [ ] Avaliar crates que podem não ter build Android: `pdf-extract`,
      `calamine`, `docx-rust`, `printpdf` (são pure-Rust, provavelmente ok),
      `ast-grep-language` (pure-Rust, ok), `xcap`/`enigo` (**não suportam
      Android** — precisam ficar atrás de cfg).

### Tarefa A.2 — Código Rust com cfg novo

- [ ] Criar um módulo `platform.rs` com `is_mobile()` /
      `supports_shell()` / `supports_computer_use()` (um único ponto de
      verdade, usado por todos os cfg espalhados).
- [ ] `computer.rs`: stub completo de todas as tools em Android (as specs
      das tools nem devem ser registradas — ver A.4).
- [ ] `shell.rs`/`background.rs`/`tools.rs::run_command`: em Android, recusar
      com mensagem clara ("execução de comandos indisponível nesta
      plataforma") em vez de spawnar `/bin/sh` que não vai existir.
- [ ] `lib.rs`: pular setup de llama.cpp/git watchdog em Android.

### Tarefa A.3 — Capabilities/permissões

- [ ] Revisar `src-tauri/capabilities/*.json`: permissões core + dialog +
      opener continuam; garantir que nada de desktop vaze.
- [ ] `AndroidManifest.xml` gerado: adicionar `<uses-permission
      android:name="android.permission.INTERNET"/>` (o template do Tauri já
      traz) e revisar storage pra exportação de sessões (zip).

### Tarefa A.4 — Registro condicional de tools no system prompt

- [ ] `agent/mod.rs`: ao montar as tool_specs, filtrar por plataforma
      (Android: só chat/read_file/grep/edit/web_search/web_fetch/skills/
      todo_list/ask — sem run_command/background/computer_use/python_tools/
      office? office pode manter, é puro Rust e lê arquivos do app).
- [ ] O system prompt precisa refletir isso, senão o LLM tenta rodar comando
      e toma erro toda hora.

### Tarefa A.5 — Frontend

- [ ] Depende da Tarefa 4.0 do plano Linux/macOS (`src/platform.ts`) —
      reutilizar.
- [ ] Esconder/desabilitar: abas de llama.cpp, MCP stdio, execução de
      comandos, computer_use, pastas extras de leitura fora do sandbox.
- [ ] Layout responsivo/mobile (hoje minWidth 820 — precisa de layout que
      funcione em ~360dp; avaliar tela lateral colapsável).

### Tarefa A.6 — Distribuição

- [ ] `.apk` de teste: `npx tauri android build --apk` (assinatura debug serve).
- [ ] Pra distribuir fora da Play Store: assinar com keystore própria
      (`keytool` + config no `build.gradle` gerado).
- [ ] Play Store exige `.aab` + conta de developer (US$ 25 únicos).

---

## 3. O que CONSEGUE rodar no Android

| Funcionalidade | Como |
|---|---|
| Chat com provedores (OpenRouter etc.) | reqwest → HTTP puro ✅ |
| Sessões, histórico, pastas de sessões | arquivos locais do app ✅ |
| Skills, todo list, task pipeline | lógica pura ✅ |
| web_search / web_fetch | reqwest ✅ |
| Leitura/edição de arquivos **dentro do sandbox do app** | filesystem do app ✅ (limitado ao que o usuário importar/criar) |
| Diff/review de código, AST grep | pure-Rust ✅ |
| Office/PDF/Excel (leitura/geração) | pure-Rust ✅ (calamine/pdf-extract/docx/printpdf) |
| MCP via HTTP/streamable | rede ✅ |
| Export/import de sessões (zip) | pure-Rust ✅ |

---

## 4. O que NÃO consegue rodar (limitação estrutural)

| Funcionalidade | Por quê |
|---|---|
| `run_command` / background jobs | Não existe terminal/shell do sistema num app Android comum; spawn de `pwsh`/`sh` não faz sentido |
| computer_use inteiro (screenshot, click, type, AX-tree) | Android não permite screenshot de outros apps nem injeção de input global via API pública (MediaProjection dá screenshot do próprio app com consentimento, mas não automação geral) |
| MCP via stdio (processos filhos) | Depende de spawn de binários que não existem no device |
| llama.cpp local | Binário desktop; compilar llama-server pro Android é projeto separado e RAM/térmico inviabiliza |
| Ferramentas Python (uv) | uv não tem build Android; Python no Android exige Termux/proot (fora de escopo) |
| Keyring seguro nativo | keyring-rs não suporta Android (issue #127); alternativa: tauri-plugin-keystore ou arquivo cifrado |
| Abrir pasta do projeto no explorer / open_external_url com opener | opener funciona pra URL; "reveal in file manager" é inconsistente no Android (issue #2913 dos plugins-workspace) |
| Git | Binário `git` não vem no Android padrão; exigiria libgit2 (migração) ou git no Termux |
| Dialog com filtro de arquivo | plugin-dialog no Android tem bugs conhecidos com filtros/paths (issues #2826, #2749) — usar sem filtro ou via SAF simplificado |

**Consequência honesta:** no Android, o Cerne vira essencialmente um
**cliente de chat + revisão + monitor**. O modo mais útil de usá-lo em
conjunto com o desktop seria: sessões criadas/executadas no PC, consultadas
pelo celular (requer sincronização — hoje inexistente; futuro: sync via
MCP HTTP ou export/import manual de zip).

---

## 5. Ordem sugerida

```
Fase A.1+A.2 (compila)        → 2-5 dias, médio risco (deps + cfgs)
Fase A.4 (tools por platform) → 1 dia, baixo risco, alto valor
Fase A.5 (frontend mobile)    → 2-4 dias (layout responsivo é o maior bloco)
Fase A.3+A.6 (permissões/apk) → 1 dia
```

Pré-requisito forte: fazer antes a Tarefa 4.0 (platform.ts) do plano
Linux/macOS, que este plano reutiliza.

## 6. Fontes principais

- Pré-requisitos Android do Tauri 2: https://v2.tauri.app/start/prerequisites/
- Desenvolvimento Android: https://v2.tauri.app/develop/android/
- cargo-mobile2 (manutenção limitada): https://github.com/tauri-apps/cargo-mobile2
- keyring-rs sem suporte Android: https://github.com/open-source-cooperative/keyring-rs/issues/127
- Plugin dialog bugs Android: https://github.com/tauri-apps/plugins-workspace/issues/2826 , #2749
- Plugin opener erro Android: https://github.com/tauri-apps/plugins-workspace/issues/2913
- Alternativa de keystore: https://github.com/impierce/tauri-plugin-keystore
