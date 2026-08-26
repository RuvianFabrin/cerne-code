# Port do Cerne Code para Linux e macOS

> Levantamento feito em 2026-08-21 (revisão 2: varredura complementar de
> `tools.rs`, `sandbox.rs`, `encoding.rs`, capabilities, stores), **sem
> editar nenhum código** — só leitura do repositório (`src/`, `src-tauri/`,
> manifestos, docs). Cada achado cita o arquivo/linha onde a evidência está.
> Dividido em fases ordenadas por dependência: não adianta empacotar (Fase 5)
> antes de compilar (Fase 1), e não adianta implementar computer_use no Unix
> (Fase 3) antes do app abrir e o keyring funcionar (Fase 2).

---

## 0. Resumo executivo

O Cerne Code é um app **Tauri 2** (Vue 3 + TypeScript no frontend, Rust no
backend, ~840 KB de código Rust em 33 arquivos). A base é mais portátil do
que parece à primeira vista: Tauri, enigo, xcap e o shell já têm camada
`#[cfg(windows)]`/`#[cfg(not(windows))]`. O que falta se concentra em
**7 pontos reais de quebra** e **3 pontos de degradação**, detalhados abaixo.

| # | Severidade | O que | Onde |
|---|-----------|-------|------|
| 1 | 🔴 Não compila | `keyring` com `features = ["windows-native"]` | `src-tauri/Cargo.toml:32` |
| 2 | 🔴 Não compila | `windows` + `uiautomation` crates só existem pra Windows (ok, já em `cfg(windows)`) mas `computer.rs` usa API Win32 em funções chamadas sem guard completo | `src-tauri/src/agent/computer.rs` |
| 3 | 🔴 Quebra em runtime | `keyring` sem backend Linux/macOS → salvar/ler chave de API falha | `src-tauri/src/config.rs`, `providers/custom.rs`, `search.rs` |
| 4 | 🔴 Quebra em runtime | `background.rs::stop` chama `taskkill` **sem `#[cfg]`** — em Unix o `let _ =` engole o erro e o processo filho fica **órfão** | `src-tauri/src/agent/background.rs:285` |
| 5 | 🟡 Feature morta | computer_use: `get_foreground_exe_name` retorna `Err` fora do Windows → `check_authorization` falha → click/type/scroll 100% quebrados | `src-tauri/src/agent/computer.rs:104` |
| 6 | 🟡 Feature morta | `exec_list_windows`, `exec_ax_tree`, `exec_click_element` têm stub "só Windows" | `src-tauri/src/agent/computer.rs` |
| 7 | 🟡 UX/confusão | Frontend assume `.exe` (file picker filtra `exe`, placeholder `llama-server.exe...`) e caminhos `C:\` | `src/components/Settings.vue:108,786`, `src/locales/*.json:207` |
| 8 | 🟡 UX/confusão | System prompt ensina o LLM a usar sintaxe Windows (`{root}\arquivo.txt`) sem considerar Unix | `src-tauri/src/agent/mod.rs:814` |
| 9 | 🟡 Testes quebram | Suite de testes usa `powershell`, `tasklist`, `ping -n`, caminhos `C:\` | `background.rs` (testes), `llama_cpp.rs` (testes) |
| 10 | 🔴 Quebra em runtime | Timeout do `run_command` síncrono mata só o shell (`kill_on_drop`), não os filhos → comando real fica **órfão** ao expirar (mesma classe do #4) | `src-tauri/src/agent/tools.rs:1157-1180` |
| 11 | 🟡 Falta infra | Frontend **não tem detecção de plataforma nenhuma** (grep por `navigator.platform`/`userAgent`/`isMac`: zero ocorrências) — necessária pro filtro `exe` condicional (Tarefa 4.1) e futuros atalhos Ctrl/Cmd | `src/` inteiro |
| 12 | 🟢 Empacotamento | `tauri.conf.json` só configura NSIS (Windows); sem CI; README/release notes só mencionam `.exe` | `src-tauri/tauri.conf.json`, inexistente `.github/` |

**Já está OK (não mexer):**
- `agent/shell.rs` — detecção de shell completa (pwsh→powershell→cmd no
  Windows; `$SHELL`/`/bin/sh` no Unix), `command_exists` (`where`/`which`),
  `apply_creation_flags`/`apply_std_creation_flags` (no-op em Unix). ✅
  *(Nota: o `kill_pid_tree_blocking` dela compila e roda em Unix, mas usa
  `kill -9 <pid>` que não mata a árvore — ver Tarefa 2.1.)*
- `mcp.rs:198-212` — build do comando MCP já tem `#[cfg]` (shell no Windows,
  exec direto em Unix). ✅
- `git.rs:78-89` — watchdog de timeout já tem `#[cfg]` (`taskkill` vs `kill -9`). ✅
- `sandbox.rs` — caminhos construídos com `Path::join` (portável); o
  `sanitize_external_path` usa `Component::Prefix` que só casa em Windows,
  mas é inofensivo em Unix (o match simplesmente não cai nesse braço —
  caminhos externos Unix são sanitizados pelos componentes `Normal`). ✅
- `encoding.rs` — deteccão de encoding via `encoding_rs`/`chardetng`,
  totalmente independente de SO (inclusive protege arquivos UTF-16/1252
  vindos do Windows lidos em qualquer SO). ✅
- Ferramentas Python (`python_tools.rs`) — rodam via `uv run`, portável;
  o aviso de `uv` ausente usa `command_exists` que já é cross-platform. ✅
- `tauri-plugin-opener` (`open_skills_folder`, `open_external_url`) —
  cross-platform pelo plugin. ✅
- Frontend Vue em si (nenhum acesso direto a API nativa, tudo via Tauri;
  os splits de path usam regex `[/\\]` que aceitam os dois separadores). ✅
- Ícones: já existe `icon.icns` (macOS) além de `icon.ico` (Windows). ✅
- Capabilities Tauri: só permissões padrão (`core/opener/dialog`) — nada
  específico de Windows. ✅

---

## 0.1 Tabela de paridade de funcionalidades (Windows vs Linux/macOS)

> Resposta à pergunta "todas as funcionalidades do Windows estarão presentes
> no Linux?" — **não todas**. O núcleo de agente de código chega a paridade
> ~100% após as Fases 1, 2 e 4, mas o **computer_use** fica recortado por
> sessão gráfica por limitação estrutural do SO (não do port).

### Paridade total esperada (após Fases 1-4)

| Feature | Observação |
|---|---|
| Chat/sessões/projeto | leitura, edição, busca, AST grep, diff/review, todo list, skills |
| `run_command` + background jobs | depende das Tarefas 2.1/2.2 (kill de árvore via process group) |
| Git | já tem `#[cfg]` — status, diff, commit, watchdog |
| MCP (stdio e HTTP) | build do comando já cross-platform |
| Ferramentas Python via `uv` | portável |
| Office/PDF/Excel | bibliotecas Rust puras |
| llama.cpp local | binário sem `.exe`; ajustes da Fase 4 |
| Keyring | ⚠️ com pré-requisito: Linux exige `libsecret` + `gnome-keyring`/KWallet no sistema. Headless/sem keyring → fallback a decidir na Tarefa 1.1 |

### Paridade parcial — computer_use

| Feature | Windows | Linux X11 | Linux Wayland | macOS |
|---|---|---|---|---|
| Screenshot | Total | OK (`xcap`) | Depende de portal xdg-desktop; pode falhar | OK (`xcap`) + **permissão Screen Recording (TCC 3.1c)** |
| Click/type/scroll/key (enigo) | Total c/ autorização | OK (injeção de input) | **Possível via Tarefa 3.1b** (portal RemoteDesktop; fallback `ydotool`) — sem ela, bloqueado | OK + **permissão Accessibility (TCC 3.1c)** |
| Listar janelas (`exec_list_windows`) | UIA completo | OK via `xcap` | **Sem suporte** (o SO não expõe janelas alheias) | OK (CGWindowList via `xcap`) + Screen Recording |
| Autorização por executável | Lista persistida por `.exe` | Possível (xdotool/`/proc/<pid>/exe`) | **Inviável** (não há "janela em foco" consultável) | `NSWorkspace.frontmostApplication` (preferir a AppleScript — TCC 3.1c) |
| AX-tree sem visão (`get_window_state`/`click_element`) | UIA completo | AT-SPI2 (Tarefa 3.3) cobre GTK/Qt bem; Electron/Chrome pior que UIA | idem (se o compositor expuser) | Accessibility API (`AXUIElement`, Tarefa 3.3) |

### Sem equivalente previsto (limitação estrutural)

1. **AX-tree Unix até a Fase 3.3 ficar pronta**: automação no Linux fica
   **100% dependente de visão** (screenshot), enquanto no Windows o agente
   opera "no escuro".
2. **Instalador NSIS multi-idioma**: no Linux a distribuição é `.deb` +
   `.AppImage`, sem o mesmo nível de instalador polido.

### Implicação prática

Wayland é o default em distros modernas (Ubuntu 22.04+, Fedora). Sem a
Tarefa 3.1b, esses usuários ficam limitados a screenshot; **com ela** (portal
RemoteDesktop + calibração + cancelamento), o input completo volta a
funcionar — restando como gaps estruturais do Wayland apenas: listar janelas,
autorização por processo (substituída pelo consentimento do portal) e AX-tree
até a Fase 3.3.
**Decisão de produto a registrar cedo**: priorizar a Tarefa 3.1b, exigir/
documentar X11 pro computer_use completo, ou aceitar a degradação como
limitação conhecida.

---

## 0.2 Guia futuro: distribuir o DMG do macOS SEM pagar US$ 99/ano

> Decisão tomada em 2026-08-22 (sem orçamento pra conta Apple Developer).
> Este guia é pra o "eu do futuro" executar sem precisar pesquisar de novo.
> Baseado em: 9to5mac.com/2024/08/06, arstechnica.com (Gatekeeper no
> Sequoia), docs do Tauri (macOS Application Bundle) e práticas de projetos
> OSS (ad-hoc sign + script xattr + Homebrew).

**O que fazer na release (checklist):**

1. **Garantir ad-hoc sign no artefato** — sem isso o macOS acusa falso
   "app danificado" em vez do aviso normal. O Tauri assina ad-hoc por
   padrão; validar no `.app` empacotado:
   ```bash
   codesign -dv "/Applications/Cerne Code.app"   # deve mostrar Signature=adhoc
   ```
   Se não estiver: `codesign --deep --force -s - "Cerne Code.app"`.

2. **Documentar na release notes / README (seção macOS)** — texto pronto:
   > No primeiro abrir, o macOS vai bloquear ("não pôde verificar o
   > desenvolvedor"). Vá em **Ajustes do Sistema → Privacidade e Segurança**
   > e clique em **"Abrir Mesmo Assim"**. Alternativa via Terminal:
   > `xattr -dr com.apple.quarantine "/Applications/Cerne Code.app"`

3. **NÃO contar com "botão direito → Abrir"** — o macOS 15 Sequoia REMOVEU
   esse atalho clássico. Instruções antigas da internet não funcionam mais.

4. **Depois que o projeto amadurecer**: publicar Homebrew cask
   (`brew install --cask cerne-code`) — é o canal profissional gratuito;
   apps instalados pelo brew têm bem menos atrito de Gatekeeper.

5. **Se/projeto crescer**: aí sim Developer ID (US$ 99/ano) + notarização =
   duplo clique e abre, sem avisos. Migração é só configurar
   `bundle.macOS.signingIdentity` no tauri.conf.json e rodar notarização —
   nada mais muda.

**Custo assumido consciente desta decisão:**

- Usuário aprova a abertura manualmente na 1ª vez (e após cada download novo);
- Binário ad-hoc muda de identidade a cada build → as permissões TCC
  (Accessibility/Screen Recording, ver Tarefa 3.1c) **zeram a cada
  atualização instalada** — quem usa computer_use re-aprova por versão;
- Sem notarização, antivírus corporativos podem reclamar do app.

**Lembrete relacionado:** em DEV local no Mac, configurar uma Apple
Development identity fixa (conta gratuita serve) já evita re-aprovar
permissões TCC a cada rebuild durante os testes (ver Tarefa 3.1c).

---

## Fase 1 — Compilar em Linux e macOS

> **STATUS (2026-08-22): implementada no Windows, validação Unix pendente.**
>
> ✅ Feito e verificado no Windows:
> - Tarefa 1.1: `keyring` agora com features `["windows-native",
>   "linux-native", "apple-native"]` (mudança aditiva; `windows-native`
>   preservado). `cargo check` OK e `cargo test --lib` **218 passed,
>   0 failed**, incluindo o teste de regressão crítico
>   `stop_kills_the_whole_process_tree_not_just_cmd_exe`.
> - Tarefa 1.2: confirmado que `windows`/`uiautomation` estão só em
>   `[target.'cfg(windows)'.dependencies]`.
> - Tarefa 1.3: confirmado `#[cfg(windows)]` no build.rs.
> - Usos de keyring (`config.rs`, `custom.rs`, `search.rs`) usam a mesma
>   API `keyring::Entry::new` — zero mudança de código necessária.
>
> ❌ Tarefa 1.4 NÃO fechável no Windows puro: o `cargo check --target
> x86_64-unknown-linux-gnu` para na fase de *link* das libs nativas do GTK
> (gdk-sys via pkg-config exige sysroot Linux com webkit2gtk/glib etc.) —
> limitação conhecida de cross-compilar Tauri de Windows pra Linux; não é
> um erro do nosso código. **Fechamento real da Tarefa 1.4 depende de:
> compilar numa máquina Linux de verdade (Mint/xRDP já disponível), WSL2
> (Ubuntu: `sudo apt install libwebkit2gtk-4.1-dev build-essential curl
> wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`)
> ou CI ubuntu-latest (Tarefa 5.2).** O mesmo vale pra macOS
> (aarch64-apple-darwin: só compila em macOS).

> Objetivo: `cargo build` e `npm run tauri build` passarem nos 3 SOs.
> Nada disso muda comportamento no Windows.

### Tarefa 1.1 — `keyring` multi-plataforma
**Arquivo:** `src-tauri/Cargo.toml:32`

Hoje:
```toml
keyring = { version = "3", features = ["windows-native"] }
```
Sem feature de Linux/macOS, a crate não tem backend nesses SOs (o mesmo bug
documentado no README §"nenhuma chave de API era realmente salva" — só que
agora na outra direção).

- [ ] Adicionar features por alvo:
  - Linux: `linux-native` (Secret Service via D-Bus — exige `libsecret`/`gnome-keyring` ou `kwallet` no sistema — **documentar como dependência de sistema**)
  - macOS: `apple-native` (Keychain)
- [ ] Avaliar fallback pra Linux headless/sem keyring (o crate oferece
  `linux-native-sync-persistent` ou um mock de arquivo) — decidir se o app
  aceita funcionar com chave em arquivo protegido por permissão 600 como
  fallback, ou se exige o Secret Service.

**⚠️ Cuidado Windows:** manter `windows-native` — é a correção do bug real
já documentado no README. A mudança é **aditiva** (features por `cfg`),
não substitutiva.

### Tarefa 1.2 — Dependências Win32 já isoladas (verificação, sem código)
**Arquivo:** `src-tauri/Cargo.toml:62-70`

`windows = "0.58"` e `uiautomation = "0.25"` já estão em
`[target.'cfg(windows)'.dependencies]` — correto, não compila em Unix e não
precisa. Só **confirmar** com um `cargo check --target x86_64-unknown-linux-gnu`
e `--target aarch64-apple-darwin` que nada vazou pra fora do `cfg`.

### Tarefa 1.3 — `build.rs` já isolado (verificação, sem código)
**Arquivo:** `src-tauri/build.rs:16`

O `println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:...")` já está dentro de
`#[cfg(windows)]` — em Unix é no-op natural. Verificação apenas.

### Tarefa 1.4 — Toolchain de cross-check
- [ ] Adicionar ao fluxo de dev: `cargo check` pros alvos
  `x86_64-unknown-linux-gnu` e `aarch64-apple-darwin` (mesmo sem CI ainda —
  ver Fase 5) pra garantir que a Fase 1 fechou. Rodar localmente primeiro
  (via `rustup target add`).

---

## Fase 2 — App abre e o essencial funciona (chat, config, keyring)

> **STATUS (2026-08-22): Tarefas 2.1, 2.2 e 2.3 CONCLUÍDAS E VALIDADAS nos
> dois SOs. BÔNUS: bug latente de portabilidade do `resolve_path` corrigido.**
>
> ✅ Windows: `cargo test --lib` **218 passed, 0 failed**
> (`stop_kills_the_whole_process_tree_not_just_cmd_exe` intacto).
> ✅ Linux (WSL Ubuntu 26.04): `cargo check` + `cargo build` OK;
> `cargo test --lib` **218 passed, 0 failed** (incluindo o novo
> `stop_kills_the_whole_process_tree_not_just_sh`);
> **o app roda de verdade** (binário abriu via WSLg,
> `~/.local/share/com.cerne.app/` criado com sessions/skills/storage).
>
> Implementação:
> - `shell.rs`: `kill_pid_tree_blocking` Unix agora mata o **process group**
>   (`kill -9 -<pgid>`) com fallback pro PID individual; nova helper
>   `apply_process_group` (Unix: `process_group(0)`; Windows: no-op).
> - `background.rs::start`: spawn com grupo próprio no Unix.
> - `background.rs::stop`: braço Unix via `spawn_blocking(kill_pid_tree_blocking)`;
>   caminho Windows (`taskkill /T /F`) intacto.
> - `tools.rs::run_command`: timeout captura o PID antes do
>   `wait_with_output()` consumir o Child, e mata a árvore explicitamente
>   nos dois SOs; fluxo de sucesso e mensagem do LLM inalterados.
> - Testes (Tarefa 2.3): teste de árvore marcado `#[cfg(windows)]` + novo
>   `stop_kills_the_whole_process_tree_not_just_sh` (Unix), helpers
>   `child_pid_of` (`/proc/<pid>/task/<tid>/children`) e `pid_exists`
>   (`/proc/<pid>`).
>
> ### Achados da depuração no WSL (importantes pro futuro)
>
> 1. **Shells POSIX fazem EXEC() direto em comando simples**: `sh -c "sleep 30"`
>    substitui o shell pelo sleep — o PID rastreado pelo tokio JÁ É o processo
>    final, não há árvore nem filho. O teste precisa de `(sleep 30 & wait)`
>    pra manter o bash vivo com subshell+sleep como filhos reais. No Windows
>    isso nunca apareceu porque cmd.exe nunca faz exec-otimização.
> 2. **BUG LATENTE CORRIGIDO em `tools.rs::resolve_within`**: o código antigo
>    fazia `project_root.join(rel.trim_start_matches(['/','\\']))` — em POSIX,
>    tirar a `/` inicial de um caminho absoluto o transformava em RELATIVO,
>    e o write caia DENTRO do projeto em vez da pasta externa. Em Manual
>    (sandbox), um write pra `/tmp/x` era tratado como caminho interno e
>    aceito/espelhado errado. Correção: caminho absoluto é preservado COMO
>    ESTÁ (`PathBuf::from(trimmed)`); relativo cai no join. Os 2 testes
>    `write_file_*absolute*` pegaram exatamente isso — validando que a suite
>    no Linux vale ouro.
>
> Pendências restantes da Fase 2: Tarefa 2.4 (fixtures INI com `C:\` — baixa
> prioridade, decidir depois) e 2.5 (validar keyring ao vivo — WSL não tem
> gnome-keyring instalado e precisa de sudo interativo; fica pro VM real).

> Objetivo: no Linux/macOS o usuário abre o app, cola a chave da OpenRouter,
> cria sessão e conversa — sem computer_use, sem llama.cpp local ainda.

### Tarefa 2.1 — Matar processo em background no Unix
**Arquivo:** `src-tauri/src/agent/background.rs:285-288`

Hoje `stop()` chama `taskkill` direto, sem `#[cfg]`:
```rust
let mut taskkill = tokio::process::Command::new("taskkill");
taskkill.args(["/PID", &pid.to_string(), "/T", "/F"]);
super::shell::apply_creation_flags(&mut taskkill);
let _ = taskkill.output().await;
```
Em Unix, `taskkill` não existe → o `let _ =` engole o erro **silenciosamente**
e o processo filho (o `python`/`node` de verdade, filho do `/bin/sh -c`) fica
órfão — exatamente o bug que esse código corrigiu no Windows.

- [ ] Substituir pela função **já existente** `shell::kill_pid_tree_blocking`
  (que já tem o `#[cfg]` correto), ou replicar o padrão dela aqui. Como aqui é
  async (`tokio::process::Command`), manter async mas com `cfg` — ou delegar
  pra `spawn_blocking(kill_pid_tree_blocking)`.
- [ ] **Revisar o equivalente Unix de matar a árvore**: `kill -9 <pid>` mata só
  o processo direto (o `/bin/sh -c`), não os filhos — o mesmo problema de órfão
  que o Windows tinha. Em Unix o correto é matar o **process group**
  (`kill -- -<pgid>`, ou `setpgid`/`process_group` no spawn + `killpg`).
  Isso é mais envolvido que no Windows — o `shell.rs::kill_pid_tree_blocking`
  atual tem o mesmo gap. Resolver os dois juntos.

**⚠️ Cuidado Windows:** este é o caminho de código do fix do "processo órfão"
(documentado extensivamente no README §530-550 e nos comentários de
`background.rs:244-294`). **Não simplificar/unificar** o caminho Windows —
manter `taskkill /PID /T /F` intacto e apenas adicionar o braço Unix. O teste
de regressão `stop_kills_the_whole_process_tree_not_just_cmd_exe` precisa
continuar passando no Windows.

### Tarefa 2.2 — Timeout do `run_command` síncrono também deixa órfãos
**Arquivo:** `src-tauri/src/agent/tools.rs:1157-1180`

Mesma classe de bug da Tarefa 2.1, num lugar diferente: quando um comando
síncrono estoura os 120s, o `tokio::time::timeout` derruba o future e o
`kill_on_drop(true)` mata **só o processo do shell** (`pwsh -Command ...`
no Windows, `/bin/sh -c ...` no Unix) — o comando real (filho do shell)
fica órfão rodando. No Windows isso é menos grave porque `kill_on_drop` no
Windows encerra o processo; em Unix o gap é real e igual ao já documentado.

- [ ] Ao expirar, matar explicitamente a árvore (mesma solução da Tarefa 2.1:
  process group em Unix, `taskkill /T /F` no Windows) antes de devolver a
  mensagem de timeout.

**⚠️ Cuidado Windows:** comportamento atual do Windows está correto — a
mudança é adicionar o kill de árvore explícito, sem alterar a mensagem nem
o fluxo de sucesso.

### Tarefa 2.3 — Testes do background que só rodam no Windows
**Arquivo:** `src-tauri/src/agent/background.rs:609-684` (testes)

- `stop_kills_the_whole_process_tree_not_just_cmd_exe` usa `powershell` e
  `tasklist` (linhas 659, 678), e `ping -n` (flag Windows — em Unix é `-c`).
- [ ] Marcar testes Windows-específicos com `#[cfg(windows)]` e criar
  equivalentes Unix (usando `ps`, `kill -0`, `sleep` em vez de `ping`).
- [ ] O helper `child_pid_of` (PowerShell/CIM) precisa de equivalente Unix
  (ler `/proc/<pid>/task/*/children` no Linux, `pgrep -P` no macOS) — ou
  reescrever o teste pra não depender de inspecionar a árvore por fora.

### Tarefa 2.4 — Testes do llama.cpp que assumem `C:\`
**Arquivo:** `src-tauri/src/providers/llama_cpp.rs:298-401` (testes)

Os fixtures usam `C:\models\...` e `llama-server.exe`. Como só testam
**parsing de INI** (não executam nada), funcionariam em Unix — mas são
frágeis/enganosos. Decidir: trocar pra caminhos neutros (`/models/...`) ou
manter (o parsing não liga pro separador). Baixa prioridade, mas anotado.

### Tarefa 2.5 — App data dir
**Arquivo:** `src-tauri/src/lib.rs` (setup)

`app.path().app_data_dir()` do Tauri já resolve o lugar certo por SO
(`%APPDATA%\com.cerne.app` no Windows, `~/.local/share/com.cerne.app` no
Linux, `~/Library/Application Support/com.cerne.app` no macOS). Sem código
necessário — só **validar** ao testar ao vivo que as pastas são criadas.

---

## Fase 3 — computer_use funcional em Linux/macOS

> Objetivo: automação de tela (screenshot/click/type/scroll) funcionar.
> Esta é a fase **mais trabalhosa** — a AX-tree (get_window_state/click_element)
> é profundamente Windows (UI Automation) e não tem equivalente direto.

> **STATUS (2026-08-26): Tarefa 3.1 implementada para Linux/X11 (não a
> alternativa pragmática — a real).**
>
> `get_foreground_exe_name` e `exec_list_windows` ganharam braço
> `#[cfg(target_os = "linux")]` usando `xcap::Window::all()` +
> `is_focused()`/`pid()` (protocolo EWMH via XCB, já era dependência —
> `capture_screen_base64` já usava a mesma API). O PID é resolvido pro nome
> do executável via `/proc/<pid>/exe` (mais confiável que `WM_CLASS`, usado
> só como fallback). Isso destrava `check_authorization` de verdade (não
> pulando a checagem — resolvendo o processo em primeiro plano igual ao
> Windows), o que por sua vez libera click/type/scroll (que já eram
> cross-platform via enigo, só bloqueados pelo `Err` fixo).
>
> `exec_focus_window`/`maybe_focus_from_args` também ganharam braço Linux:
> como não há equivalente de `SetForegroundWindow` acessível via crate pura
> sem reimplementar client messages EWMH cruas (xcb não é dependência
> direta), a ativação é via `wmctrl -a <título>` (busca por substring, com
> `xdotool search --name ... windowactivate` como fallback) — mesmo padrão
> de dependência de sistema opcional já usado pro `uv` em `agent::tools`
> (`command_exists`). Sem nenhum dos dois instalado, retorna erro claro
> explicando a dependência faltante em vez de falhar silenciosamente.
>
> ✅ Validação: `cargo check` + `cargo test --lib` **218 passed, 0 failed**
> nos dois SOs (Windows e WSL Ubuntu) — nenhuma mudança nos caminhos
> Windows (`#[cfg(windows)]` intactos).
>
> ⚠️ **Validação funcional ainda pendente**: testei um probe ao vivo
> (`get_foreground_exe_name`/`exec_list_windows` chamados de verdade) no
> WSLg e o compositor dele **não implementa `_NET_CLIENT_LIST_STACKING`**
> (propriedade EWMH que o xcap usa) — erro `_NET_CLIENT_LIST_STACKING not
> supported`. É uma limitação conhecida do compositor do WSLg, não do
> código: um X11 "de verdade" (GNOME/KDE/XFCE em VM ou máquina real)
> implementa essa propriedade normalmente. **Pendência real**: validar em
> Linux com sessão X11 completa (não WSLg) antes de considerar 3.1 fechada
> de fato — só então dá pra confirmar que autorização por processo e
> `computer_use_list_windows` funcionam ao vivo, não só compilam.
>
> Wayland permanece sem suporte por essa via, como já esperado (a
> "janela ativa" não é exposta a outros processos por segurança) — cai no
> mesmo `Err` tratado como "aplicação não autorizada", que é o
> comportamento aceitável até a Tarefa 3.1b.

### Tarefa 3.1 — Desbloquear screenshot/click/type/scroll
**Arquivo:** `src-tauri/src/agent/computer.rs:104-108`

```rust
#[cfg(not(windows))]
fn get_foreground_exe_name() -> Result<String> {
    Err(anyhow!("get_foreground_exe_name nao implementado nesta plataforma"))
}
```
Esse `Err` propaga pra `check_authorization` → **toda** chamada de
click/type/scroll falha, mesmo enigo/xcap sendo cross-platform.

- [ ] Implementar `get_foreground_exe_name` por SO:
  - **Linux/X11**: via `xcap` (já dependência) — `Window::all()` + foco, ou
    `xdotool getactivewindow getwindowpid` / ler `_NET_ACTIVE_WINDOW` e
    `/proc/<pid>/exe`.
  - **Linux/Wayland**: **problema real** — Wayland não expõe janela ativa de
    outro processo por segurança. Provável resposta: "sem suporte a
    autorização por processo em Wayland" (degradar pra autorização manual
    por sessão, ou avisar que computer_use exige X11).
  - **macOS**: `NSWorkspace.frontmostApplication` via crate `objc2`/`cocoa`,
    ou AppleScript `tell application "System Events" to get name of first
    process whose frontmost is true`.
- [ ] Alternativa pragmática pra Fase 3 inicial: em Unix, **pular a checagem
  de autorização por processo** (manter o rate-limit e o consentimento via
  `ask` no fluxo), marcando como limitação conhecida — destrava
  screenshot/click/type/scroll sem resolver a identificação de processo.

**⚠️ Cuidado Windows:** a identificação por processo + lista de autorização
persistida (`computer_permissions.json`) é uma feature de segurança real.
O braço Unix deve ser **adição** (`#[cfg(not(windows))]` implementando o
mesmo contrato), nunca mudança no caminho Windows. Manter o formato do
`computer_permissions.json` idêntico pra sessões não divergirem.

### Tarefa 3.1b — Wayland: input via portal RemoteDesktop + calibração + cancelamento

> **STATUS (2026-08-26): rota de input via portal implementada e
> compilando de verdade no WSL (não só no Windows). Calibração e
> cancelamento avançado (bullets 3 e 4 abaixo) ficaram fora de escopo —
> ver detalhe no fim desta nota.**
>
> Novo módulo `src-tauri/src/agent/computer_wayland.rs`
> (`#[cfg(target_os = "linux")]`), usando a crate `ashpd` (bindings Rust
> pros portais XDG — nova dependência, só no target Linux). Sessão
> combinada `RemoteDesktop` (mouse/teclado) + `Screencast` (só pra obter o
> `stream` node id do PipeWire que `notify_pointer_motion_absolute` exige
> como referência de coordenadas — não consome o vídeo em si, mesmo escopo
> de monitor do `computer_use_screenshot` via xcap). Sessão cacheada com
> timeout de inatividade de 5min; consentimento via diálogo nativo do
> compositor só na primeira chamada.
>
> `computer.rs::execute()` detecta `is_wayland_session()` (mesma
> heurística `WAYLAND_DISPLAY`/`XDG_SESSION_TYPE` que o xcap usa
> internamente) e desvia click/type_text/press_key/scroll pro backend do
> portal — **pulando** `check_authorization` (não dá pra identificar
> processo em foco no Wayland) e `maybe_focus_from_args` (portal não
> permite focar por título de janela), exatamente a "alternativa
> pragmática" que o plano original previa: o consentimento do diálogo do
> portal já é a barreira de segurança equivalente nesse caminho.
>
> Mapeamento de teclas via keysyms X11 (`keysymdef.h`, valores estáveis
> desde os anos 90): ASCII direto pro codepoint, resto via convenção XKB
> Unicode (`0x01000000 + codepoint`). Scroll via `notify_pointer_axis`.
> Botões via códigos evdev `BTN_LEFT`/`BTN_RIGHT`/`BTN_MIDDLE`.
>
> ✅ Validação: `cargo check` + `cargo test --lib` **218 passed, 0 failed**
> nos dois SOs. Como esperado, **nenhuma validação ao vivo é possível**
> aqui — nem WSLg tem portal `RemoteDesktop`/`Screencast` disponível (são
> específicos de sessão gráfica completa GNOME/KDE), então até compilar
> foi o teto do que dava pra confirmar sem a VM.
>
> ⚠️ **Pendências reais, fora de escopo desta rodada**:
> 1. **Calibração de 5 pontos** (bullet 3 abaixo): a conversão pixel do
>    screenshot → coordenada do portal aqui é 1:1. Sem calibração, telas
>    com HiDPI/escala fracionária provavelmente desalinham cliques. Só dá
>    pra calibrar testando ao vivo com hardware real.
> 2. **Cancelamento via GlobalShortcuts/hot-corner** (bullet 4 abaixo): não
>    implementado. Mitigação parcial: o portal injeta input **sintético**
>    (não captura o mouse/teclado físico do usuário), então o usuário
>    sempre pode retomar controle por vias normais do SO mesmo sem esses
>    mecanismos extra — mas o watchdog de inatividade (5min) é o único
>    "freio" automático hoje.
> 3. Nunca testado contra um compositor de verdade — a assinatura exata de
>    `select_sources`/`start`/`notify_*` foi confirmada via docs.rs oficial
>    (não inventada), mas comportamento em runtime (ordem de diálogos,
>    mensagens de erro reais do GNOME/KDE) é 100% desconhecido até a VM.
>
> O enigo injeta input direto no display server — o Wayland bloqueia isso por
> design. Mas existem rotas oficiais de injeção que tornam o computer_use
> **possível** no Wayland, com consentimento explícito do usuário.

**Rotas de injeção (em ordem de preferência):**

| Rota | Mouse | Teclado | Exige |
|---|---|---|---|
| `xdg-desktop-portal` **RemoteDesktop** (D-Bus): sessão confirmada por diálogo nativo do desktop, depois `NotifyPointerMotionAbsolute(x,y)`, `NotifyPointerButton`, `NotifyKeyboardKeysym` etc. | ✅ | ✅ | GNOME 46+ / KDE Plasma 6+ |
| `ydotool` (dispositivo virtual via `/dev/uinput`, nível kernel) | ✅ | ✅ | daemon `ydotoold` + grupo `uinput`/root — setup manual, documentar como fallback |
| `wtype` (`zwp_virtual_keyboard_v1`) | ❌ | ✅ | suporte variável por compositor |

A posição na tela continua vindo da mesma fonte de hoje (screenshot via
`xcap` ou portal Screenshot) — só muda quem executa o movimento/clique.
Compositores menores (Sway/Hyprland via portal wlroots): suporte parcial →
fallback `ydotool`.

- [ ] Implementar backend de input Wayland via portal RemoteDesktop (D-Bus),
      com `#[cfg(target_os = "linux")]` + detecção de sessão (X11 vs Wayland
      via `WAYLAND_DISPLAY`/`XDG_SESSION_TYPE`) escolhendo entre o caminho
      enigo (X11) e o portal (Wayland).
- [ ] Tratar o consentimento do portal como **substituto natural da
      autorização-por-processo** (impossível no Wayland — ver 3.1): o
      diálogo nativo do desktop ("este app quer controlar seus dispositivos")
      é garantia mais forte que a lista `computer_permissions.json`. Não
      exigir a lista nesse cenário.
- [ ] **Calibração de 5 pontos** (cantos + centro da área útil), executada
      na primeira vez e sob demanda:
      1. mover o cursor aos 5 pontos com pausa em cada;
      2. perguntar ao usuário via `ask` se o cursor passou por todos;
      3. se não, calcular fator de correção (offset/escala lógica↔física,
         crítico em HiDPI/multi-monitor com escala fracionária) e persistir
         num arquivo de calibração (mesmo espírito do
         `computer_permissions.json`);
      4. recalibrar quando a configuração de displays mudar.
      Em monitor único sem escala fracionária os pontos passam de primeira —
      a calibração vale como teste de sanidade/confiança contra falha
      silenciosa de clique.
- [ ] **Cancelamento** (o mouse estará sendo usado pelo agente, então o
      mecanismo não pode depender dele):
      - atalho global via portal `GlobalShortcuts` (GNOME 47+/KDE) ou
        binding do compositor (ex.: `F12`) encerra a sessão RemoteDesktop
        na hora — teclado físico do usuário NUNCA é capturado pela injeção;
      - hot-corner de fuga: cursor físico num canto definido aborta;
      - watchdog: sem comandos de input por N segundos → solta a sessão;
      - botão CANCELAR always-on-top do próprio Cerne como rede secundária.
- [ ] Documentar atrito: cada sessão pede consentimento (ou persiste por app
      conforme versão do portal) — coerente com o modelo de segurança do
      Wayland, mas mais atrito que o Windows.

**⚠️ Cuidado Windows:** nada disso toca o caminho Windows/X11 — é um backend
novo atrás de cfg, selecionado só quando a sessão for Wayland.

### Tarefa 3.1c — macOS: permissões TCC + assinatura + HiDPI

> No macOS as features de computer_use compilam e rodam, mas dependem de
> permissões de sistema (TCC) que o app precisa pedir/guiar. Sem isso,
> tudo falha **silenciosamente**. Além disso, binário não-assinado perde as
> permissões a cada rebuild.
>
> **SOLUÇÃO ENCONTRADA (pesquisa web): existe caminho pronto pra ~100%.**
>
> **STATUS (2026-08-26): backend parcial — plugin de TCC integrado +
> screenshot/click/type/scroll/list_windows generalizados pra macOS via
> reuso do código Linux. Fluxo de UI guiada e assinatura/HiDPI ainda não.**
>
> **Muito mais às cegas que Linux**: aqui nem `cargo check --target
> aarch64-apple-darwin` funciona (sem toolchain C pra macOS no Windows —
> `objc2-exception-helper` já falha no build script por falta de `cc`).
> Diferente de tudo que veio antes nesta fase (Linux sempre validado via
> WSL de verdade), isto é 100% pesquisa de docs.rs + zero compilação.
>
> O que foi feito:
> - `tauri-plugin-macos-permissions` (v2.3.0) integrado: dependência
>   `#[target.'cfg(target_os = "macos")']`, registrado no builder do
>   `lib.rs` via `#[cfg(target_os = "macos")] let builder =
>   builder.plugin(...)`. Expõe pro frontend `checkAccessibilityPermission`/
>   `requestAccessibilityPermission`/`checkScreenRecordingPermission`/
>   `requestScreenRecordingPermission` (entre outras) via
>   `tauri-plugin-macos-permissions-api` (npm) — **nenhuma UI foi
>   construída ainda consumindo isso**, só o backend está pronto pra ser
>   chamado.
> - **Quase-bug real pego a tempo**: a primeira tentativa adicionou
>   `"macos-permissions:default"` direto no `capabilities/default.json`
>   (compartilhado por todos os SOs) — isso **quebrou o build no Windows**
>   (`cargo check` recusou com "Permission macos-permissions:default not
>   found", porque o plugin nem compila fora do macOS, então o schema de
>   permissões gerado não conhece esse identificador). Corrigido criando
>   `capabilities/macos-permissions.json` separado com `"platforms":
>   ["macOS"]` — o Tauri pula a validação desse arquivo em builds de outro
>   SO. Confirmado com `cargo check` no Windows depois da correção.
> - **`get_foreground_exe_name` e `exec_list_windows` generalizados pra
>   macOS reaproveitando o código Linux/X11 já escrito** (não é
>   implementação nova): o `xcap` tem a MESMA API pública nos dois SOs
>   (`Window::all()`, `is_focused()`, `pid()`, `app_name()`, `title()`,
>   `x()/y()/width()/height()` — só a implementação interna muda por SO,
>   confirmado lendo o código-fonte do crate). Só o fallback
>   `/proc/<pid>/exe` (Linux-only) ficou dentro de um `#[cfg(target_os =
>   "linux")]` interno; no macOS cai direto no `app_name()` (que lá vem do
>   `NSRunningApplication`, identificador natural pro vocabulário de
>   autorização). `check_authorization` e `exec_click`/`exec_type_text`/
>   `exec_scroll` (que já eram cross-platform via enigo) passam a
>   funcionar no macOS **de graça** por consequência — eram só bloqueados
>   pelo antigo `Err` fixo do stub.
> - `computer_use_authorize` (spec da tool) atualizado pra explicar o
>   formato do nome no macOS (nome de exibição do app, ex: "Google
>   Chrome", não caminho de executável).
> - `exec_focus_window` **NÃO** generalizado pra macOS — depende de
>   `wmctrl`/`xdotool`, ferramentas que só existem no Linux. Fica pro
>   escopo futuro (osascript ou API nativa).
>
> ✅ Validado: `cargo check`/`cargo test --lib` 218/218 no Windows (onde o
> plugin nem compila) e no WSL Linux (onde o branch generalizado continua
> batendo o caminho Linux original, sem regressão). **Zero validação
> possível pro código macOS em si** — nem compilação.
>
> ❌ Pendências reais desta tarefa (não tocadas): UI guiada de permissão
> (check → botão pedir/abrir Ajustes → re-checar), Info.plist (só
> necessário se algum dia pedir mic/câmera — não é o caso hoje),
> assinatura estável em dev (`signingIdentity`), e a solução determinística
> de HiDPI/Retina (escala lógica↔física antes de despachar coordenadas de
> click). Tudo isso exige testar contra um Mac de verdade pra fazer com
> confiança — implementar às cegas essas partes específicas (assinatura,
> HiDPI) tem risco alto demais de erro sutil sem nenhuma forma de detectar.

- [ ] **Usar `tauri-plugin-macos-permissions` (v2.3.0, MIT,
      github.com/ayangweb/tauri-plugin-macos-permissions)**: plugin Tauri 2
      que faz check/request/open-settings de Accessibility, Screen
      Recording, Full Disk Access e Input Monitoring — API Rust +
      frontend. Elimina a implementação manual de detecção. Fluxo:
      antes de usar screenshot/input, `check()` → se negado, UI guiada com
      botão `request()`/abrir Ajustes → re-checar.
- [ ] Alternativa sem plugin (APIs nativas direto de Rust):
      - Screen capture: `CGPreflightScreenCaptureAccess()` (checa sem
        prompt) e `CGRequestScreenCaptureAccess()` (dispara o diálogo) —
        via crate `objc2-core-graphics` ou `core-graphics2`;
      - Acessibilidade: `AXIsProcessTrustedWithOptions` com
        `kAXTrustedCheckOptionPrompt = true` — o próprio macOS mostra o
        prompt de concessão (doc Apple: ApplicationsServices/
        1459186-axisprocesstrustedwithoptions).
- [ ] **Detecção de app em foco (3.1)**: preferir `NSWorkspace`
      (objc2/cocoa) a AppleScript — AppleScript dispara prompt adicional de
      Automation/Apple Events e exige chave `NSAppleEventsUsageDescription`
      no Info.plist.
- [ ] **Info.plist**: garantir as chaves de privacidade necessárias no
      bundle (`NSAppleEventsUsageDescription` se usar AppleScript; screen
      capture é TCC direto, não tem chave de uso). Manuseio de Info.plist
      customizado pelo Tauri é parcialmente documentado (issue
      tauri-apps/tauri#13068) — validar na versão em uso; senão, script
      pós-build sobre o `.app`.
- [ ] **Assinatura estável desde o dev**: binário ad-hoc/não-assinado muda
      de identidade por build e o macOS zera as permissões TCC a cada
      rebuild. Solução: configurar `bundle.macOS.signingIdentity` com uma
      Apple Development identity fixa já no ambiente de dev (conta gratuita
      de developer serve pra assinar localmente); distribuição final com
      Developer ID + notarização (decisão da Tarefa 5.1).
- [ ] **HiDPI/Retina — solução determinística**: obter o fator de escala
      por monitor (`NSScreen.backingScaleFactor` via objc2, ou comparar
      largura lógica vs física que o xcap reporta) e converter as
      coordenadas do screenshot (pixels físicos) pra coordenadas lógicas
      antes de despachar input. A calibração manual de 5 pontos (3.1b)
      fica como rede de segurança/fallback, não como mecanismo primário
      no macOS.

### Tarefa 3.2 — `exec_list_windows` no Unix
**Arquivo:** `src-tauri/src/agent/computer.rs:818-823` (stub atual)

- Linux/X11: `xcap::Window::all()` já lista janelas (o próprio
  `capture_screen_base64` usa pra screenshot de janela específica) — reusar.
- macOS: `xcap` também cobre (CGWindowList).
- Wayland: sem suporte (mesma limitação da Tarefa 3.1).

### Tarefa 3.3 — AX-tree (get_window_state/click_element)
**Arquivo:** `src-tauri/src/agent/computer.rs:737-1171`

> **STATUS (2026-08-26): implementada pra Linux via AT-SPI2, compila limpo
> nos dois SOs, zero validação ao vivo (mesma ressalva de sempre).**
>
> Novo módulo `src-tauri/src/agent/computer_atspi.rs`
> (`#[cfg(target_os = "linux")]`), usando a crate `atspi` (0.30, wrapper
> D-Bus AT-SPI2) + `zbus` como dependência direta (pra resolver o PID de
> cada app via `org.freedesktop.DBus.GetConnectionUnixProcessID` no nome
> único do barramento dela — AT-SPI não expõe PID como propriedade do
> objeto acessível, então precisa desse passo extra; é a mesma técnica que
> leitores de tela como o Orca usam).
>
> Fluxo: conecta no barramento de acessibilidade → lista as aplicações
> registradas no registry AT-SPI → resolve o PID de cada uma → acha a que
> bate com o PID pedido → percorre a árvore de filhos dela (busca em
> profundidade **iterativa**, não recursiva — evitou precisar de mais uma
> dependência só pra boxing de future recursivo) coletando elementos com
> papel "acionável" (`Role::Button`, `Entry`, `CheckBox`, `ComboBox`,
> `Link`, `ListItem`, `MenuItem`, `RadioButton`, `PageTab` — mesmo
> vocabulário de controles do braço Windows). `click_element` reusa a
> mesma travessia e invoca a ação padrão (índice 0) via interface `Action`
> — **mais robusto que o Windows nesse ponto**: não depende de calcular
> "clickable point" nem do elemento estar visível na tela, já que AT-SPI
> invoca a ação diretamente no objeto.
>
> Todas as assinaturas de API (`AccessibleProxy`, `ActionProxy`,
> `AccessibilityConnection`, `ObjectRef`, `Role`,
> `DBusProxy::get_connection_unix_process_id`) foram **confirmadas via
> docs.rs oficial antes de escrever o código** (não inventadas) — mesmo
> assim precisou de zero iteração de correção no WSL, compilou de primeira
> (diferente da Tarefa 3.1b, que exigiu um fix real).
>
> ✅ Validação: `cargo check` (zero warnings) + `cargo test --lib`
> **218 passed, 0 failed** nos dois SOs.
>
> ⚠️ **Nunca testado contra um AT-SPI de verdade** — não tem app GTK/Qt
> rodando com acessibilidade ativa no WSL pra testar contra. Riscos reais
> que só a VM vai revelar: apps sem suporte a acessibilidade habilitado
> (alguns toolkits exigem variável de ambiente ou flag pra ligar AT-SPI),
> Electron/Chrome com suporte parcial (mencionado no plano original), e a
> própria latência de D-Bus pra árvores grandes (o cap de 200 elementos/
> profundidade 12 é um chute razoável, não testado).
>
> A árvore de acessibilidade é o caminho **sem visão** pra automatizar tela —
> feature diferenciadora do Cerne. Equivalentes:
>
> - **Linux**: AT-SPI2 (`atspi` crate) — o padrão de acessibilidade em GTK/Qt.
>   ✅ feito acima.
> - **macOS**: Accessibility API (`AXUIElement`) via `objc2`/`accessibility`
>   crate. Continua pendente (Tarefa 3.1c/3.3-macOS) — nem compilável aqui
>   sem uma máquina Mac.

### Tarefa 3.4 — Atualizar descrições das tools
**Arquivo:** `src-tauri/src/agent/computer.rs` (tool_specs)

As descrições das tools dizem "So Windows" / exemplos `chrome.exe`,
`notepad.exe`. Ao implementar os braços Unix, ajustar os textos (que vão
parar no system prompt do LLM) pra não prometer nem negar errado por SO.

---

## Fase 4 — llama.cpp local e frontend neutros de plataforma

### Tarefa 4.0 — Infra de detecção de plataforma no frontend
**Arquivos:** `src/` (novo utilitário, ex: `src/platform.ts`) + consumidores

O frontend hoje **não tem nenhuma** detecção de SO (grep por
`navigator.platform`/`userAgent`/`isMac`: zero ocorrências). Várias tarefas
desta fase e da Fase 3 dependem disso:

- [ ] Criar util único (`getPlatform()` via `navigator.userAgent`/`userAgentData`
  ou comando Tauri `get_platform`) — **um só ponto de verdade**, não regex
  espalhada por componente.
- [ ] Consumidores imediatos: filtro `exe` do file picker (Tarefa 4.1),
  placeholders de executável, exemplos MCP (Tarefa 4.3).
- [ ] Futuro (baixa prioridade): atalhos de teclado Ctrl vs Cmd se a UI ganhar
  shortcuts dependentes de plataforma.

### Tarefa 4.1 — Nome do executável do llama-server
**Arquivos:** `src-tauri/src/providers/llama_cpp.rs:224`,
`src/components/Settings.vue:108,786`, `src/locales/*.json:207`

- `llama_cpp.rs:224`: mensagem de erro hardcoded `llama-server.exe not found` —
  trocar por algo neutro (`llama-server não encontrado em {path}`), já que
  em Unix o binário não tem `.exe`.
- `Settings.vue:108`: filtro do file picker `extensions: ["exe"]` → em Unix
  precisa aceitar executável sem extensão. O Tauri dialog aceita filtro
  condicional — decidir em runtime pelo SO (usar o util da Tarefa 4.0).
- `Settings.vue:786`: placeholder `llama-server.exe...` → idem.
- `src/locales/*.json:207`: hint de configuração menciona `llama-server.exe`
  nos 4 idiomas → parametrizar ou neutralizar.

**⚠️ Cuidado Windows:** em Windows o filtro `exe` continua correto — a
mudança é **condicional por SO**, não remover o filtro globalmente (senão o
usuário Windows perde a conveniência do filtro).

### Tarefa 4.2 — System prompt neutro de separador de caminho
**Arquivo:** `src-tauri/src/agent/mod.rs:814`

```
(ex: {root}\arquivo.txt) — nunca caminhos relativos.
```
O `\` hardcoded ensina o LLM errado em Unix. Trocar por instrução neutra
("caminho absoluto completo, com o separador do SO") ou interpolar o
separador real via `std::path::MAIN_SEPARATOR`. Mesmo tratamento no
`agent/tools.rs:150` (descrição da `read_file` com exemplo `F:\outro-repo`).

**⚠️ Cuidado Windows:** o prompt em Windows deve continuar mostrando `\` (ou
ser neutro o bastante pros dois). Não trocar por `/` fixo — modelos já se
confundem com caminho Windows com `/`.

### Tarefa 4.3 — `mcpConnectors.ts` com exemplos Windows
**Arquivo:** `src/content/mcpConnectors.ts:49,124`

Exemplos de configuração MCP usam `C:\caminho\da\pasta`. São **só exemplos**
na UI, mas confundem usuário Unix. Parametrizar por SO ou usar caminho
neutro com nota.

---

## Fase 5 — Build, empacotamento e distribuição

### Tarefa 5.1 — Targets de bundle no `tauri.conf.json`
**Arquivo:** `src-tauri/tauri.conf.json` (`bundle`)

> **STATUS (2026-08-26): categoria + versão mínima do macOS adicionadas,
> validado no Windows. `"targets": "all"` mantido de propósito (ver
> abaixo) — não é o `[ ]` faltando que parece.**
>
> Adicionado `bundle.macOS.minimumSystemVersion: "10.15"` e
> `bundle.category: "DeveloperTool"` (nível raiz do `bundle`, não dentro de
> `macOS` — **primeira tentativa colocou `category` dentro de
> `bundle.macOS` e o `cargo check` recusou** com "unknown field category,
> expected... minimum-system-version... dmg" — o build-script do Tauri
> valida o schema do `tauri.conf.json` em tempo de compilação, então o erro
> apareceu na hora, não só no `tauri build`). `windows.nsis` continua
> intocado.
>
> **Decisão sobre `"targets": "all"`**: mantido como está, não trocado por
> lista explícita por SO. Motivo: `"all"` no Tauri já escopa
> automaticamente pro que o SO **host** da build suporta (não tenta gerar
> `.msi` no Linux nem `nsis` no Linux) — o risco que o bullet original
> temia ("build falhar tentando empacotador que falta") não existe na
> prática; trocar por targets explícitos seria mudança sem ganho real e
> com chance de digitar um nome de target errado sem poder testar nos 3
> SOs aqui. Deixado como está até a Fase 5.2 (CI) rodar de verdade e provar
> o contrário.
>
> ✅ Validado: `cargo check --lib` + `cargo test --lib` (218/218) no
> Windows após a mudança.
>
> Pendências reais: assinatura/notarização real do macOS (decisão já
> registrada abaixo, ad-hoc — só falta executar quando houver Mac pra
> testar) e checklist de dependências Linux (webkit2gtk etc.) — já
> documentadas no `ci.yml` (Tarefa 5.2), faltando só espelhar no README
> (Tarefa 5.3).

- [x] macOS: configurar `bundle.macOS` (categoria, mínimo de sistema) e
      decidir **assinatura de código + notarização** (sem isso, o `.app`
      abre com aviso de "desenvolvedor não identificado" no Gatekeeper — decisão de
      produto: aceitar o aviso ou pagar Apple Developer).
      **DECISÃO (sem orçamento pra US$ 99/ano): distribuir DMG assinado
      ad-hoc.** Implicações registradas (pesquisa web):
      - macOS 15 Sequoia REMOVEU o "botão direito → Abrir": usuário precisa
        de Ajustes → Privacidade e Segurança → "Abrir Mesmo Assim"
        (9to5mac.com/2024/08/06, arstechnica.com — gatekeeper sequoia);
      - SEM ad-hoc sign, o macOS pode reportar falso "app danificado" —
        garantir `codesign --deep -s -` (Tauri já faz ad-hoc por padrão,
        validar no artefato);
      - documentar na release: passo a passo "Abrir Mesmo Assim" + comando
        `xattr -dr com.apple.quarantine "/Applications/Cerne Code.app"`
        como alternativa (padrão usado por projetos OSS);
      - caminho profissional sem custo: publicar Homebrew cask depois de
        maduro (`brew install --cask cerne-code`);
      - custo conhecido do ad-hoc: identidade muda a cada build →
        permissões TCC (3.1c) zeram a cada atualização; usuário de
        computer_use re-aprova por versão. Reavaliar Developer ID +
        notarização (US$ 99/ano) se/projeto crescer.
- [x] Linux: formatos ficam a cargo do `"all"` (`.deb`+`.AppImage` no
  Ubuntu/Debian, o que o host suportar noutras distros) — decisão tomada
  acima, junto com `"targets"`. Dependências de sistema já documentadas no
  `ci.yml` (Tarefa 5.2): `libwebkit2gtk-4.1-dev`, `libxdo-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`,
  `build-essential`. Só falta espelhar isso no README (abaixo, Tarefa 5.3).
- [x] Ícones: confirmado por `ls` que todos os arquivos da lista
  `bundle.icon` existem de fato em `src-tauri/icons/` (`icon.icns` incluso).

### Tarefa 5.2 — CI multi-plataforma

> **STATUS (2026-08-26): workflow criado (`.github/workflows/ci.yml`),
> ainda NÃO commitado/pushado — validação real só acontece no primeiro
> push pro GitHub (não dá pra rodar Actions localmente).**
>
> Matrix `ubuntu-latest`/`windows-latest`/`macos-latest`, cada um rodando
> na ordem: instala deps de sistema do WebView (só Linux — mesma lista da
> Tarefa 1.4), `vue-tsc --noEmit`, `cargo check --lib`, `cargo test --lib`,
> `npm run tauri build` (empacotamento completo), upload dos instaladores
> como artifact por SO. Dispara em push/PR pra `main` e manualmente
> (`workflow_dispatch`). Cache de `cargo` (`Swatinem/rust-cache`) e `npm`
> (`actions/setup-node` com `cache: npm`) pra não re-baixar tudo a cada run.
>
> **Deliberadamente sem `continue-on-error`** no step de build: é esperado
> que `npm run tauri build` falhe em macOS/Linux até a Tarefa 5.1 (targets
> de bundle por SO, assinatura) fechar — o objetivo da CI é revelar esse
> gap, não escondê-lo. `cargo check`/`cargo test --lib`, porém, devem
> passar nos 3 SOs desde já (é o que as Fases 1-4 + parte da 3 entregaram).
>
> **Atualização (mesmo dia): build universal no macOS.** O runner
> `macos-latest` do GitHub hoje é Apple Silicon — sem tratamento especial o
> `.dmg` gerado só rodaria nativamente em arm64 (Intel dependeria de
> Rosetta 2). Ajustado: `dtolnay/rust-toolchain` instala os targets
> `aarch64-apple-darwin` + `x86_64-apple-darwin` só no job do macOS, e o
> step de build chama `npm run tauri build -- --target
> universal-apple-darwin` (caminho oficial do Tauri — builda os dois
> targets e junta com `lipo`, ~2x mais lento de propósito). Upload de
> artifact ajustado pra cobrir os dois paths de saída possíveis
> (`target/release/bundle` normal e `target/universal-apple-darwin/release/
> bundle` do build universal). YAML validado com `js-yaml`; comportamento
> real só confirma no primeiro push.
>
> Isso finalmente vai dar a primeira confirmação real de compilação em
> Linux/macOS via CI **de verdade** (runner nativo, não WSL/cross-compile) —
> inclusive vai revelar na hora se os módulos novos da Fase 3
> (`computer_wayland`, `computer_atspi`) têm algum problema que só aparece
> fora do ambiente WSL específico usado pra validar até aqui.
>
> Pendente: commitar e dar push pra ver o primeiro run real (ação que pede
> confirmação explícita, não faço sozinho).

- [x] GitHub Actions com matrix
  `windows-latest` / `ubuntu-latest` / `macos-latest` rodando:
  `cargo check`, `cargo test --lib`, `vue-tsc --noEmit`, `npm run tauri build`.
- [x] Artefatos por SO (instalador NSIS / `.dmg` / `.deb`+`.AppImage`) via
  `actions/upload-artifact`.
- [ ] Primeiro push/run real pra confirmar que o workflow funciona (não dá
  pra testar Actions localmente).
- Isso **também protege o Windows**: qualquer mudança das Fases 1-4 que
  quebrar o Windows falha na CI antes de chegar no usuário — a principal
  rede de segurança contra regressão.

### Tarefa 5.3 — Documentação

> **STATUS (2026-08-26): parcial, de propósito.**
>
> - [x] `comandos para rodar o projeto.txt`: `npm run tauri build` agora
>   explica que o instalador gerado varia por SO host.
> - [ ] **README (linhas 51-243) — deliberadamente NÃO tocado.** A tabela
>   "Windows, Linux, Mac ✅" descreve o produto **comparado**, não o Cerne —
>   mas mesmo assim seria prematuro anunciar suporte Linux/macOS: Linux só
>   foi validado via WSL (nunca em VM/hardware real), macOS nem compila
>   (sem máquina Mac disponível). Mudar o README antes disso seria
>   propaganda enganosa. Decisão herdada de sessão anterior ("atualizar
>   quando o port sair") — mantida.
> - [x] `RELEASE_NOTES_v0.1.0.md` — **deliberadamente NÃO tocado.** É
>   registro histórico de uma versão já lançada, só-Windows na época; editar
>   pra mencionar Linux/macOS seria reescrever história, não documentar.
> - [x] Dependências de sistema do Linux (webkit2gtk-4.1, libxdo,
>   libayatana-appindicator3, librsvg2, openssl) já documentadas em
>   `.github/workflows/ci.yml` (Tarefa 5.2) e na Tarefa 1.1 (keyring:
>   `linux-native` exige `libsecret`/`gnome-keyring` ou `kwallet`). Limitação
>   Wayland documentada extensivamente na Tarefa 3.1b (portal RemoteDesktop,
>   sem calibração/cancelamento ainda). Nada novo a escrever — já está tudo
>   neste arquivo, que é a fonte de verdade até o port realmente fechar.

---

## Mapa de riscos de regressão no Windows (checklist transversal)

O que **não pode quebrar** — verificar em cada fase:

1. **`background.rs::stop`** — o fix do processo órfão (taskkill com pai vivo,
   `kill_on_drop` como rede de segurança). Teste de regressão:
   `stop_kills_the_whole_process_tree_not_just_cmd_exe`. **Rodar a suite
   `cargo test --lib` inteira no Windows depois de cada fase.**
1a. **`tools.rs::run_command` timeout** — mesmo tema (Tarefa 2.2): o kill de
   árvore explícito novo não pode alterar o fluxo de sucesso nem a mensagem
   de timeout que o LLM consome.
2. **`keyring` Windows** — manter `windows-native` (bug já corrigido uma vez,
   README §939-949). Mudança só aditiva.
3. **`shell.rs`** — não tocar no que já é cross-platform. A tentação de
   "simplificar" unificando caminhos é onde mora o risco.
4. **Filtro `exe` / placeholder no Settings.vue** — condicional por SO, não
   remoção.
5. **Formato de `computer_permissions.json`** — idêntico entre SOs.
6. **Prompt do LLM** — testar ao vivo no Windows depois de mexer em
   `mod.rs:814`/`tools.rs:150` (o prompt é o contrato de comportamento do
   agente; uma mudança ruim aqui degrada tudo de forma silenciosa).
7. **`tauri.conf.json`** — a seção `windows.nsis` (idiomas do instalador,
   `installMode: perMachine`) fica **intocada**; adições em seções novas
   (`macOS`, `linux`).
8. **CI (Fase 5.2)** é a rede de segurança final: sem ela, cada fase depende
   de alguém lembrar de testar Windows manualmente.

## Ordem sugerida de ataque

```
Fase 1 (compila)      → 1-2 dias, baixo risco, sem mudança de comportamento
Fase 2 (app abre)     → 2-4 dias, risco médio (background.rs é delicado)
Fase 4 (llama/frontend)→ 1-2 dias, baixo risco, independente da 3
Fase 3 (computer_use) → 5-10 dias, risco alto (AX-tree Unix é o maior
                        bloco de trabalho novo; aceitar degradação
                        "só com visão" no início)
Fase 5 (build/CI/docs) → 2-3 dias, trava a qualidade de tudo acima
```
