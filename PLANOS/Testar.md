# Checklist de Testes Manuais — Port Linux/macOS (Fases 1-2 + 4)

> Como usar: rode o app no Windows E no WSL (`wsl -d Ubuntu` →
> `cd ~/cerne-code/src-tauri && ./target/debug/cerne`) e confira item por
> item nos dois lados. Marque `[x]` só quando passar nos DOIS, exceto onde
> diz "só Windows" ou "só Linux".
>
> Ambiente WSL já configurado: `~/cerne-code`, deps Tauri instaladas,
> `cargo build` gera `target/debug/cerne`. A janela abre via WSLg.
>
> ⚠️ Keyring não testável no WSL atual (sem gnome-keyring; precisa de VM
> com desktop — ver pendência da Tarefa 2.5).

---

## 1. Boot e infraestrutura básica

- [ ] App abre sem crash no Linux (janela WSLg aparece com título "Cerne Code")
- [ ] App abre normalmente no Windows (regressão zero)
- [ ] Sidebar, abas de Settings, modal de Help renderizam sem erro de CSS/layout
- [ ] Criar sessão nova funciona (persiste em `~/.local/share/com.cerne.app/sessions/` no Linux)
- [ ] Fechar e reabrir o app: sessões criadas continuam listadas
- [ ] Skills globais foram backfilladas em `~/.local/share/com.cerne.app/skills/`

## 2. Chat e provedores (núcleo multiplataforma)

- [ ] Colar chave da OpenRouter nas Settings (se keyring falhar no WSL, anotar
      o erro exato — esperado até instalar gnome-keyring)
- [ ] Selecionar modelo e enviar mensagem simples ("diga oi") — resposta chega
- [ ] Streaming do token a token funciona (texto cresce progressivamente)
- [ ] Trocar entre 2 provedores custom (LM Studio/Ollama local se tiver) mantém histórico
- [ ] Context gauge atualiza o % conforme a conversa cresce
- [ ] Botão de nova sessão / troca de sessão não perde mensagens

## 3. run_command e background jobs (Fase 2 — o grosso das mudanças)

### Síncrono
- [ ] Pedir pro agente rodar `echo teste-multiplataforma` via run_command — exit_code 0 e stdout visível
- [ ] Comando que falha (`ls /pasta-que-nao-existe`) retorna stderr legível (não crash)
- [ ] Comando longo (>120s, ex.: `sleep 130`) expira com a mensagem padrão
      **E** não deixa processo órfão: após o timeout, rodar `ps -ef | grep sleep`
      (Linux) ou `tasklist | findstr sleep` (Windows) — NÃO deve aparecer
- [ ] Output grande (ex.: `cat` num arquivo de 10k linhas) é truncado sem travar

### Background
- [ ] Iniciar servidor fake em background (`python3 -m http.server 8099` no
      Linux / `python -m http.server 8099` no Windows) — job fica "rodando"
- [ ] `check_background_output` mostra o output inicial do servidor
- [ ] `stop_background` encerra e a porta 8099 é liberada
      (testar: `curl localhost:8099` deve falhar após o stop)
- [ ] **Teste crítico do órfão**: iniciar em background um script que spawna
      filho real (ex.: `bash -c '(sleep 300 & wait)'`), dar stop, e confirmar
      via `ps -ef | grep sleep` que NADA ficou rodando
- [ ] Fechar o app com job rodando: nenhum processo órfão sobrevive
      (`ps -ef | grep -E 'sleep|http.server'` depois do app fechar)

## 4. Ferramentas de arquivo (inclui o fix do resolve_path)

- [ ] read_file/edit_file/write_file dentro do projeto funcionam normal (ambos SOs)
- [ ] **write_file pra pasta EXTRA fora do projeto** (cadastrar pasta extra
      ReadWrite nas settings e pedir write lá) — arquivo aparece NA PASTA
      EXTERNA de verdade, não dentro do projeto (é o bug que corrigimos)
- [ ] Modo Manual: write_file pra caminho absoluto externo é REJEITADO com erro claro
- [ ] Modo YOLO: write externo escreve direto na pasta externa
- [ ] Modo Auto: write externo vai pra sandbox `_external/` e vira diff pendente
- [ ] Diff review mostra as mudanças pendentes e aceitar aplica no arquivo real
- [ ] grep encontra padrão em arquivo UTF-8 com acentos (ex.: "configuração")
- [ ] ast_grep encontra chamada de função JS/TS conhecida do próprio projeto

## 5. Git (já era cross-platform — regressão)

- [ ] Abrir projeto que é repositório git: status/diff funcionam
- [ ] Fazer commit pelo agente funciona
- [ ] git watchdog: comando git travado (simular com hook lento se quiser)
      é morto no timeout SEM deixar órfão

## 6. MCP

- [ ] Adicionar servidor MCP HTTP (streamable) e listar tools dele
- [ ] Chamar uma tool MCP HTTP pelo agente
- [ ] (Só pra validar depois da Fase 4.3) exemplos de config MCP na UI não
      mostram mais `C:\...` quando rodando no Linux

## 7. Skills, todo list, orquestração

- [ ] Importar uma skill e invocá-la numa conversa
- [ ] Todo list aparece, atualiza e persiste entre turnos
- [ ] Sub-agente (task tool) executa sub-tarefa e devolve relatório ao agente pai

## 8. Frontend neutro (Fase 4 — validar DEPOIS de implementada)

- [ ] Settings de llama.cpp: filtro do file picker não trava mais em `.exe`
      quando roda no Linux (no Windows continua filtrando .exe)
- [ ] Placeholder do campo de executável não diz mais `llama-server.exe...`
      no Linux
- [ ] Hint dos locales fala "llama-server" genérico (sem `.exe`)
- [ ] Mensagem de erro de fork inexistente não menciona `llama-server.exe not found`
- [ ] System prompt (pedir pro agente imprimir os paths que ele enxerga):
      no Linux ele usa `/`; no Windows continua `\`
- [ ] Exemplos MCP na UI mostram caminho neutro/genérico (sem `C:\`)

## 9. Regressão específica de Windows (roda SÓ no Windows)

- [ ] Suite completa: `cargo test --lib` → 218 passed (ou mais, nunca menos)
- [ ] Teste crítico `stop_kills_the_whole_process_tree_not_just_cmd_exe` passa
- [ ] Nenhuma janela de console pisca ao usar run_command (CREATE_NO_WINDOW intacto)
- [ ] Chave de API salva nas Settings sobrevive a restart do app (Credential Manager)
- [ ] Instalador NSIS ainda gera (se tiver que gerar release): `npm run tauri build`

## 9.5 computer_use no Linux/X11 (Fase 3, Tarefa 3.1 — ⚠️ NÃO TESTÁVEL NO WSLg)

> **Importante: rodar isto numa VM/máquina Linux com desktop X11 de verdade
> (GNOME/KDE/XFCE), NUNCA no WSLg.** Descobri ao vivo que o compositor do
> WSLg não implementa `_NET_CLIENT_LIST_STACKING` (propriedade EWMH que o
> xcap usa pra listar janelas) — todo item abaixo vai falhar no WSL só por
> isso, mesmo com o código correto. Ver nota "STATUS 2026-08-26" na Fase 3
> de `PLANOS/port_linux_macos.md`.

- [ ] `computer_use_list_windows` retorna a lista real de janelas abertas
      (pid, título, geometria) — não mais "não implementado"
- [ ] Abrir 2 apps (ex.: um terminal e um editor de texto), clicar na janela
      do terminal pra focar, e pedir pro agente `computer_use_click` num
      ponto qualquer — **não** deve dar erro "APLICACAO NAO AUTORIZADA"
      referente ao app errado (confirma que `get_foreground_exe_name`
      identificou o processo certo via `/proc/<pid>/exe`)
- [ ] `computer_use_authorize` seguido de click/type/scroll funciona de
      ponta a ponta (autorizar → digitar num campo → ver o texto aparecer)
- [ ] `computer_use_focus_window` com `wmctrl` instalado: título parcial
      traz a janela certa pro primeiro plano (testar com `sudo apt install
      wmctrl` se não tiver)
- [ ] Sem `wmctrl` nem `xdotool` instalados: `computer_use_focus_window`
      retorna erro claro pedindo pra instalar um dos dois (não trava/crasha)
- [ ] Confirmar que nada disso quebrou no Windows (checklist §3.1 Windows
      de `computer_use_get_window_state`/`click_element` continuam "só
      Windows" sem regressão)

## 9.6 computer_use no Linux/Wayland (Fase 3, Tarefa 3.1b — ⚠️ NUNCA TESTADO, nem no WSLg)

> **Mais importante ainda que a 9.5: isto é código que eu escrevi e só
> consegui compilar (WSL) — nunca rodei contra um compositor de verdade
> nem vi o diálogo de consentimento aparecer.** A sessão GNOME/KDE
> completa (não WSLg, que não tem os portais RemoteDesktop/Screencast) é
> o requisito mínimo. Espere ter que iterar aqui — é o pedaço mais
> arriscado de tudo que foi implementado até agora.

- [ ] Sessão GNOME 46+/KDE Plasma 6+ com Wayland ativo (confirmar:
      `echo $XDG_SESSION_TYPE` deve dizer `wayland`)
- [ ] Pedir pro agente `computer_use_click` num ponto qualquer — deve
      aparecer o diálogo NATIVO do compositor pedindo autorização de
      "controlar teclado e mouse" (é a primeira vez que a sessão é criada)
- [ ] Aceitar o diálogo — o clique deve acontecer na coordenada certa
      (comparar com o screenshot que o agente tinha acabado de tirar)
- [ ] Recusar o diálogo (testar em outra conversa/sessão) — deve voltar
      erro claro pro agente, não travar nem crashar o app
- [ ] `computer_use_type_text` funciona depois do click (mesma sessão,
      sem novo diálogo)
- [ ] `computer_use_scroll` e `computer_use_press_key` (testar `ctrl+a`,
      `enter`) funcionam sem novo diálogo
- [ ] **Verificar alinhamento em tela com escala fracionária** (125%/150%
      no GNOME): é esperado que o clique DESALINHE — calibração (Tarefa
      3.1b, item 1) não foi implementada. Anotar o desvio em pixels se
      acontecer, útil pra calibrar depois
- [ ] Deixar a sessão parada por 5+ minutos e clicar de novo — deve
      reabrir um novo diálogo de consentimento (watchdog de inatividade)
- [ ] Confirmar que o teclado/mouse FÍSICO do usuário continua funcionando
      normalmente durante e depois de uma ação do agente (o portal não
      deveria capturar input real, só injetar sintético)

## 9.7 AX-tree no Linux/AT-SPI2 (Fase 3, Tarefa 3.3 — ⚠️ NUNCA TESTADO)

> Igual à 9.6: código compilado, nunca executado contra um AT-SPI de
> verdade. Precisa de sessão gráfica com apps GTK/Qt normais (não WSLg).

- [ ] Abrir um app GTK simples (ex.: `gedit`, `nautilus`, ou até o GNOME
      Calculator) e pegar o PID dele (`pgrep gedit` ou similar)
- [ ] `computer_use_get_window_state` com esse PID retorna uma lista de
      elementos `[N] Role "nome"` (não "nenhum elemento encontrado")
- [ ] Testar com um app SEM suporte a acessibilidade ativo (ou um PID que
      não existe) — deve dar erro claro, não travar
- [ ] `computer_use_click_element` num índice de botão real (ex.: "Fechar"
      ou "Novo") efetivamente aciona o botão (confirmar visualmente)
- [ ] Testar num app Electron (ex.: VS Code, se tiver) — anotar se a AX-tree
      vem vazia/parcial (esperado, mencionado no plano) ou funciona igual
- [ ] Testar um app com árvore grande (ex.: uma janela de configurações com
      muitas abas) — confirmar que não trava/demora demais (cap de 200
      elementos/profundidade 12, nunca cronometrado de verdade)
- [ ] Comparar side-by-side com o mesmo fluxo no Windows (mesmo app se
      houver equivalente) — a lista de elementos deve ter a mesma utilidade
      prática, mesmo com Role names diferentes

## 10. Conhecido-pendente (NÃO marcar como bug)

- [~] Keyring no WSL: vai falhar sem gnome-keyring (instalar na VM futura;
      dependências documentadas na Tarefa 1.1)
- [~] computer_use no Wayland: calibração HiDPI e cancelamento via
      GlobalShortcuts/hot-corner não implementados (Tarefa 3.1b parcial) —
      cliques podem desalinhar em telas com escala fracionária, é esperado
- [~] computer_use no macOS: ainda stub "não implementado nesta plataforma"
      (Tarefa 3.1c)
- [~] AX-tree (`computer_use_get_window_state`/`click_element`) no macOS:
      continua "não implementado" (Tarefa 3.3-macOS via Accessibility API,
      escopo separado) — no Linux já foi implementada via AT-SPI2, ver §9.7
- [~] llama.cpp local: ajustes cosméticos são justamente a Fase 4 em curso
- [~] Fixtures de teste INI com `C:\` (Tarefa 2.4): cosmético, decidir depois

---

## Roteiro rápido (versão 15 minutos)

Se estiver sem tempo, esse é o smoke test mínimo após cada fase:

1. App abre nos 2 SOs sem crash
2. Uma conversa completa com streaming num provedor real
3. run_command síncrono + background com stop (checar órfão via ps/tasklist)
4. write_file externo cai na pasta certa (bug do join POSIX)
5. `cargo test --lib` verde nos 2 SOs
