# Cerne Code

Agente de código local com interface gráfica — o equivalente ao Claude Code
CLI, mas em app desktop. Tauri v2 (Rust) + Vue 3 + PrimeVue.

## Rodando

```powershell
cd C:\cerne
npm install          # se ainda não rodou
npm run tauri dev    # janela nativa, hot-reload do frontend
```

`npm run dev` sozinho sobe só o Vite (útil pra iterar na UI sem recompilar
Rust, mas sem os comandos IPC — o app fica sem dados reais nesse modo).

### Testando a janela nativa de verdade (não uma copia sem IPC)

O `request_access` do computer-use não reconhece o Cerne Code (sem atalho no Menu
Iniciar). Pra clicar/digitar/screenshotar a janela WebView2 de verdade (com
IPC real do Tauri, não uma aba de navegador solta), use debug remoto do
Chromium por baixo do WebView2 + `scripts/cdp.mjs` (cliente CDP sem
dependências, usa o `WebSocket` nativo do Node):

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
npm run tauri dev
```

```bash
node scripts/cdp.mjs screenshot out.png
node scripts/cdp.mjs click 600 700
node scripts/cdp.mjs eval "document.title"
```

Dialogos nativos do Windows (o file-picker de "Escolher pasta") não fazem
parte do DOM, então o CDP não alcança — pra criar sessão com projeto sem
passar pelo picker, chame o comando Tauri direto via `eval`:
`window.__TAURI_INTERNALS__.invoke('create_session', { title, provider,
model, projectRoot: 'C:/caminho/com/barra/normal', forkId })` (barra normal
no caminho, backslash escapado pelo shell costuma corromper o valor).

## Decisões tomadas

- **Tauri, não Electron** — Rust não estava instalado nesta máquina; foi
  instalado (rustup, toolchain MSVC) como parte deste setup, já que o
  usuário queria Rust e a máquina já tinha o Visual Studio com C++.
- **4 providers, mesma API** — OpenRouter, llama.cpp local (os dois forks em
  `C:\ai-turboquant`), Ollama e LM Studio são todos OpenAI-compatíveis
  (`/v1/chat/completions`), então existe uma única implementação de
  streaming (`src-tauri/src/providers/mod.rs`) reaproveitada pelos 4 — só
  muda base_url/chave/endpoint de listagem de modelo.
- **Sandbox de edição** — `write_file`/`edit_file` nunca tocam o arquivo
  real: escrevem em `{projeto}_cerne_sandbox/` ao lado do projeto (mesmo
  padrão do `file_editor/` do Regente) e o usuário aceita/rejeita o diff na
  UI antes de aplicar.
- **Tema** — PrimeVue (preset Aura customizado), fundo branco, bordas de
  1px, peso de fonte 500, ícones Google Material Symbols self-hosted (pacote
  npm `material-symbols`, sem dependência de rede em runtime).
- **PrimeVue pinado em 4.5.5 (+ `@primeuix/themes` em 2.0.3), não a última
  versão** — a partir do PrimeVue 5.0.0 (e `@primeuix/themes` 3.x) o pacote
  passou a exigir uma chave de licença ("PrimeUI"), mesmo pra uso
  individual/gratuito — sem ela a UI mostra um banner vermelho "Invalid
  PrimeUI License". 4.5.5/2.0.3 são as últimas versões MIT de verdade, sem
  esse requisito. **Não faça `npm update` nesses dois pacotes** sem checar a
  licença de novo antes.
- **Compactação de contexto** — a cada passo do loop, se o histórico passar
  de 50% da janela de contexto do modelo ativo (estimativa chars/4), o
  agente resume tudo exceto o system prompt e as últimas 6 mensagens numa
  chamada extra ao mesmo provider/modelo (`agent::maybe_compact`). Indicador
  de uso fica no rodapé do composer (`ContextGauge.vue`).
- **Busca web via SearXNG** (`agent/websearch.rs`) — reaproveita o mesmo
  container Docker (`searxng`, porta 8888) e settings.yml do Regente, sem
  precisar subir infra nova. Duas tools: `web_search` (lista título/URL/
  trecho) e `web_fetch` (extrai texto visível de uma URL específica via
  `scraper`, HTML→texto). Diferente do Regente, não faz fetch+resumo em
  massa automático — o próprio agente decide quando buscar mais fundo,
  chamando `web_fetch` como uma tool separada. `web_fetch` tem guarda contra
  SSRF (só http/https, resolve o host e bloqueia IP loopback/privado/
  link-local) já que a URL normalmente vem de resultado de busca ou texto
  do próprio modelo — testes em `websearch.rs::tests`.
- **Skills** (`src-tauri/src/skills.rs`) — pastas com `SKILL.md` (mesmo
  formato do Claude Code: frontmatter `name`/`description` + corpo livre).
  Globais em `%APPDATA%\Cerne\skills\`, por projeto em
  `<projeto>/.cerne/skills\`. O agente só vê nome+descrição de cada skill no
  system prompt (catálogo) e carrega o corpo inteiro sob demanda via
  `load_skill`, pra não inflar o prompt com skills que não vão ser usadas.
  Gerenciamento (listar/criar/abrir pasta) na tela de Configurações.
- **Sessão fixa em provider+modelo, editável só por comando dedicado** —
  cada sessão pina `provider`/`model` na criação (`sessions.rs`) e nunca
  mais lê a config global; o seletor no composer edita a sessão atual de
  verdade via `update_session_provider_model`, não a config. A tela de Nova
  Sessão tem seu próprio seletor local em vez de depender da config global
  (que podia nunca ter sido setada — era a causa do erro `model is
  required` que apareceu nos testes).
- **Health-check real no `start_llama_server`** — só devolve sucesso depois
  de `/health` responder (ou detecta o processo morrendo sozinho e devolve
  o código de saída); antes disso, `spawn()` bem-sucedido já contava como
  "rodando" mesmo se o processo morresse logo em seguida por config
  inválida. Testado ao vivo forçando um `models.ini` inexistente.
- **Gestão automática do servidor llama.cpp** (`ensure_llama_ready` em
  `lib.rs`) — só um fork roda por vez (os dois disputam a mesma porta na
  GPU local). Ao mandar mensagem numa sessão llama.cpp com o servidor
  parado, sobe sozinho antes de enviar (status "Iniciando servidor
  local..." na UI). Ao trocar o seletor pra outro fork ou pra outro
  provider inteiramente, para o processo anterior — e se a troca for pra
  outro fork llama.cpp, já sobe o novo na hora, sem esperar o próximo
  envio. Generalizado por fork_id (não hardcoded pro TurboQuant/PrismML
  desta máquina) — outro fork ou setup local futuro só precisa de uma
  entrada nova em `providers.toml`. Testado ao vivo os 3 caminhos: auto-start
  no envio, auto-stop ao trocar de provider, auto-start ao trocar de volta.
- **Pontinho verde/vermelho de status** (`StatusDot.vue`, `useLlamaHealth.ts`)
  — nas linhas de fork em Configurações e no composer (só quando o provider
  ativo é llama.cpp). Polling a cada 4s via `llama_server_health`. A checagem
  não é só "a porta responde" — os dois forks compartilham a mesma porta
  (só um roda por vez), então um ping puro de porta mostraria os dois como
  "rodando" ao mesmo tempo. O comando cruza isso com o rastreamento interno
  do app (`AppState.llama_children`, qual fork o próprio Cerne Code iniciou) e só
  confirma via `/health` o fork que bate com esse rastreamento. Testado ao
  vivo, inclusive matando o processo por fora do app e vendo o ponto virar
  vermelho sozinho no próximo ciclo de polling.
- **`ast_grep`/`ast_edit`** (`agent/ast_tools.rs`) — busca e reescrita
  **estrutural** de código (AST), não textual: `console.log($$$ARGS)` acha
  qualquer chamada independente de espaço/quebra de linha/formatação, coisa
  que `grep` não faz. Primeiro item portado da análise do oh-my-pi (seção
  6.8 do `agent-architecture-research.md`) — mas em vez de vendorizar o
  fork deles, depende direto das crates reais e publicadas
  `ast-grep-core`/`ast-grep-language` (0.44, mesmo motor que a ferramenta
  `ast-grep` CLI usa), já que estão no crates.io e não precisam de fork.
  Inicialmente só 12 das 28 linguagens do crate estavam habilitadas (mapeamento
  manual de extensão por linguagem); depois trocado por `SupportLang::file_types()`
  (do próprio `ast-grep-language`, não reimplementado) direto no `WalkBuilder::types()`,
  então as 28 linguagens que o crate traz prontas (bash, c, cpp, csharp, css,
  dart, elixir, go, haskell, hcl, html, java, javascript, json, kotlin, lua,
  markdown, nix, php, python, ruby, rust, scala, solidity, swift, typescript,
  tsx, yaml) funcionam sem duplicar a lista de extensão a mão — os "~57" do
  fork do oh-my-pi são gramáticas *extras* vendorizadas além do crate real;
  não vendorizamos isso, só maximizamos as 28 que já vêm prontas. **Achado ao
  testar C/C#**: ao contrário de JS/Python (que aceitam uma chamada solta como
  padrão), linguagens baseadas em statement como C/C# exigem o `;` no final do
  padrão pra casar uma chamada de função como statement — particularidade do
  ast-grep por linguagem, não bug do Cerne Code. `ast_edit` passa pelo mesmo
  sandbox+diff dos outros editores. **Achado real ao implementar**: a API
  `Root::replace()` só troca a primeira ocorrência por chamada, não todas —
  precisei de um loop até não sobrar match nenhum (pego por um teste que
  falhou, não por leitura de doc). 6 testes unitários em
  `ast_tools.rs::tests`, mais verificação ao vivo no app (tool call
  disparada, executada, resultado na UI). **Limitação observada, não é bug
  do Cerne Code**: o modelo local pequeno (`gemma4-e4b-qat-mtp` via TurboQuant)
  embaralha os `$` de `$$$ARGS` em números ao gerar o argumento da tool
  call — parece ser o template de chat desse modelo quantizado tendo
  problema com o caractere `$`. Modelos maiores (Claude, GPT, Qwen maiores)
  tendem a não ter esse problema; vale reavaliar se aparecer de novo com
  outro modelo.

- **`grep` via crates reais do ripgrep** (`agent/tools.rs::grep_search`) — trocou o regex
  linha-a-linha manual (`content.lines()` + `regex::Regex`) por `grep-regex`/`grep-searcher`
  (crate `grep = "0.3"`, já estava no `Cargo.toml` sem uso — mesmas crates que o binário `rg`
  usa). Mesma linha do `ast_grep`: nada de vendorizar o `pi-uu-grep` do oh-my-pi, foi direto nas
  crates publicadas equivalentes do BurntSushi/ripgrep. Ganho concreto: detecção de arquivo
  binário de verdade (`BinaryDetection::quit`, pula sem tentar decodificar) em vez de só
  ignorar erro de UTF-8 depois de já ter lido o arquivo inteiro. 3 testes unitários em
  `tools.rs::tests` (match com número de linha certo, arquivo binário pulado, sem match).

- **Cache de travessia com TTL** (`agent/walk_cache.rs`) — trocou o `ignore::WalkBuilder` cru
  que o `grep` chamava direto a cada busca por uma camada de cache (root canonico → lista de
  arquivos, TTL de 1s) na frente dele. Inspirado no `pi-walker` do oh-my-pi
  (`crates/pi-walker/src/cache.rs`, lido em `C:\Users\ru\oh-my-pi-src` — clone raso local pra
  consulta), mas **não vendorizado**: o original tem ~5.600 linhas cobrindo generalidade que o
  Cerne Code não precisa (bindings N-API, streaming visitors, ranking). Aqui bastou `ignore` (já
  dependência) + `dashmap` (crate real e mantida, não fork) — a mesma ideia central (cache com
  TTL, invalidação explícita), reduzida ao tamanho de um app local de uma sessão só. Ganho
  concreto: se o agente chama `grep` várias vezes seguidas na mesma subpasta dentro de 1s (comum
  ao investigar um bug em sequência), a segunda chamada em diante reusa a varredura em vez de
  re-percorrer o disco. Invalidação explícita em dois pontos — `accept_edit` (quando o usuário
  aceita um diff da sandbox pro arquivo real) e `run_command` (comando arbitrário pode ter criado/
  apagado arquivo) — pra nunca servir uma lista de arquivos desatualizada depois de uma mudança
  real conhecida. **Paralelismo adicionado depois** (`WalkBuilder::build_parallel()`, o walker
  paralelo nativo da própria crate `ignore` — o mesmo motor que o binário `rg` usa por baixo, não
  a crate `rayon` do `pi-walker` original): escala sozinho pro número de CPUs sem dependência
  nova. A ordem de chegada entre threads não é determinística, então o resultado é ordenado antes
  de cachear (saída sempre igual pro mesmo estado de disco). 4 testes unitários em
  `walk_cache.rs::tests` (serve do cache dentro do TTL, `invalidate` força reescaneio e também
  invalida por sub-caminho, travessia paralela não perde arquivo em árvore com várias subpastas,
  saída fica ordenada). `list_dir` (não recursivo, `std::fs::read_dir` direto) não precisou de
  cache/paralelismo — só o `grep`/`ast_grep`, que são recursivos, se beneficiam.

- **Fallback fuzzy no `edit_file`** (`agent/tools.rs::find_trimmed_line_windows`/
  `reindent_replacement`) — antes, `old_str` tinha que bater byte-a-byte com exatamente 1
  trecho do arquivo, senão o modelo recebia um erro genérico ("encontrado 0 vezes") mesmo
  quando a diferença era só espaço em branco/indentação (comum quando o modelo "lembra" o
  trecho de uma leitura anterior com indentação diferente da real). Inspirado no modo `replace`
  do oh-my-pi (`packages/coding-agent/src/edit/modes/replace.ts::seekSequence`, lido em
  `C:\Users\ru\oh-my-pi-src`), que faz uma cascata de 8 passos (trim/comentário/unicode/prefixo/
  substring/fuzzy por similaridade Levenshtein/character-level). O Cerne Code portou 3 níveis dessa
  cascata (`FuzzyEditOutcome`/`find_edit_window`) em vez dos 8 — cada nível só roda se o anterior
  não achou nada, e se um nível achar mais de uma janela, para ali e reporta ambiguidade em vez
  de tentar o próximo nível mais frouxo (mais chance de ambiguidade, não menos):
  1. **Espaço/indentação** (`find_trimmed_line_windows`) — compara linha a linha ignorando espaço
     nas pontas; cobre indentação/trailing whitespace diferente do que o modelo "lembra" do arquivo.
  2. **Pontuação tipográfica unicode** (`find_unicode_normalized_windows`/`normalize_unicode`) —
     como o nível 1, mas também normaliza aspas curvas (“”‘’), travessão (—–) e reticências (…)
     pro equivalente ASCII; cobre o caso de o modelo "embelezar" texto ao citar de volta um
     trecho que passou por renderização markdown.
  3. **Similaridade de texto** (`fuzzy_window_scores`/`text_similarity`, Levenshtein implementado
     a mão — ~15 linhas, mesma ideia que o próprio oh-my-pi usa, não há crate publicada padrão só
     pra isso) — último recurso: pontua toda janela por similaridade média (limiar 95%), só aceita
     se exatamente 1 janela passar do limiar; cobre erro de digitação/paráfrase de um modelo mais
     fraco (ex: `compute_totals` em vez de `compute_total`). Se não achar nada nem no nível 3, a
     mensagem de erro inclui a % de similaridade do trecho mais próximo, pra o modelo saber se
     vale a pena tentar de novo ou se o `old_str` está mesmo errado.

  Não portados: prefixo/substring (ambiguidade difícil de desambiguar com pouco ganho real) e
  comentário-prefixo (benefício estreito). Todos os níveis passam pelo mesmo reindent e sandbox
  — o pior caso continua sendo um diff estranho, não um arquivo corrompido. Match exato com
  múltiplas ocorrências continua sendo erro imediato (ambiguidade real, fuzzy não ajudaria a
  desambiguar). 16 testes (unitários de cada nível + 4 end-to-end via `execute_project_tool`
  cobrindo sucesso em cada nível e a mensagem de erro com % de similaridade). **Testado
  ao vivo no app** (não só nos testes): sessão real com `qwen3.5-9b-mtp` via TurboQuant pediu
  pra documentar uma função, o modelo encadeou sozinho `list_dir`→`grep`→`read_file`→`edit_file`,
  a escrita foi pra sandbox, e o diff aceito na UI aplicou certo no arquivo real — confirma o
  `grep`/`edit_file` novos funcionando dentro do loop de agente de verdade, não só isolados.
  Verificado via CDP no WebView2 (`--remote-debugging-port`), já que o `request_access` do
  computer-use não reconhece o Cerne Code (sem atalho no Menu Iniciar); sessão de teste criada
  direto por `window.__TAURI_INTERNALS__.invoke('create_session', ...)` porque o seletor de
  pasta do projeto abre o file-picker nativo do Windows, fora do alcance do CDP.

- **✅ IMPLEMENTADO — Detecção e preservação de codificação de arquivo** (`src-tauri/src/encoding.rs`) — antes,
  `read_file`/`edit_file`/`ast_edit`/`ast_grep` liam com `std::fs::read_to_string` (exige UTF-8
  válido) e escreviam sempre como UTF-8 puro. Ler assim já era seguro (bytes inválidos falham a
  leitura, nada é escrito), mas dois problemas reais: (1) o Cerne Code simplesmente não conseguia
  editar arquivo em Windows-1252/ISO-8859-1 nem UTF-16 — a leitura falhava com erro genérico;
  (2) um caso silencioso e perigoso — texto ASCII puro salvo em UTF-16LE/BE (`'H' '\0' 'e' '\0'
  ...`) é, por coincidência, uma sequência de bytes *válida* como UTF-8 (byte `0x00` é o próprio
  código NUL), então a leitura "funcionava" e devolvia uma string cheia de NUL entre cada
  caractere — reescrever isso como UTF-8 de verdade corromperia um arquivo que qualquer programa
  Windows lia perfeitamente bem. Nenhum dos três harnesses de referência (oh-my-pi, opencode,
  grok-build — checados no clone local) trata isso: todos assumem UTF-8 estrito e recusam ou
  substituem bytes inválidos (`TextDecoder("utf-8", {fatal:true})`/`String::from_utf8_lossy`),
  só tratando BOM/EOL à parte — não havia o que portar, foi implementado do zero com uma crate
  real (`encoding_rs`, da Mozilla — implementação de referência do WHATWG Encoding Standard, o
  motor por baixo do Firefox/Servo), mesmo padrão das outras portas desta sessão. Fluxo de
  detecção, do mais confiável pro mais especulativo: (1) BOM — UTF-8/UTF-16LE/UTF-16BE,
  determinístico; (2) sem BOM, UTF-8 estrito válido — a maioria dos arquivos hoje; (3) sem BOM e
  inválido como UTF-8 — **detecção estatística real via `chardetng`** (também da Mozilla, o
  mesmo motor do "Detectar automaticamente" do Firefox), não mais um fallback cego pra
  Windows-1252: cobre qualquer codificação legada que um navegador reconheceria (Shift-JIS, GBK,
  EUC-KR, KOI8-R, as variantes ISO-8859 de verdade, etc.), não só a mais comum no Windows — e
  quando o sinal é fraco (arquivo curto, pouco texto não-ASCII) o próprio `chardetng` cai pra
  Windows-1252 sozinho, mesmo comportamento de fallback que um navegador teria pra página sem
  `charset` declarado. A codificação detectada na leitura é reaplicada na escrita
  (`sandbox::write_sandboxed`), incluindo BOM; UTF-16 é empacotado a mão (`str::encode_utf16` +
  bytes LE/BE) porque o `encoding_rs` **decodifica** UTF-16 mas nunca **codifica** pra UTF-16 —
  de propósito, por seguir a spec (formulário web nunca submete UTF-16). Arquivo novo (sem
  original) usa UTF-8 sem BOM. 11 testes unitários em `encoding.rs::tests` (round-trip de cada
  encoding, fallback pra Windows-1252 quando o sinal é fraco, detecção de Shift-JIS de verdade —
  japonês repetido o bastante pra dar sinal estatístico — confirmando que não é mais "sempre
  Windows-1252", BOM preservado/removido) + 2 testes end-to-end via
  `execute_project_tool` confirmando que `edit_file` preserva UTF-16LE e Windows-1252 byte a
  byte. **Testado ao vivo no app**: arquivo `.ini` real em Windows-1252 (bytes `E9`/`E7`/`FA`,
  sem BOM, inválido como UTF-8) editado por uma sessão real com `qwen3.5-9b-mtp` — o `read_file`
  mostrou os acentos corretos (não veio garbled), o `edit_file` trocou só a linha pedida, e os
  bytes no disco depois de aceitar continuaram `E9`/`E7`/`FA` (não viraram `C3 A9`/`C3 A7`/`C3
  BA`, que é como ficariam se tivesse virado UTF-8 por engano). **`grep` também corrigido**
  (`agent/tools.rs::grep_search`) — antes buscava via `grep-searcher` direto nos bytes crus do
  disco (`search_path`), então um padrão acentuado (o modelo manda sempre em UTF-8) nunca batia
  dentro de um arquivo Windows-1252/ISO (o "é" acentuado é 1 byte cru lá, 2 bytes em UTF-8).
  Agora decodifica cada arquivo via `crate::encoding` antes de buscar (`search_slice` sobre o
  texto já em UTF-8, não `search_path` sobre o disco) — mesma lógica que já resolvia isso no
  `edit_file`/`read_file`. Teste unitário confirmando busca acentuada batendo num arquivo
  Windows-1252 cru.
- **Comando em segundo plano** (`agent/background.rs`, tools `check_background_output`/
  `stop_background`/`list_background` + `run_command(background=true)`) — antes, `run_command`
  era sempre síncrono: um `npm run dev`/`cargo watch`/dev server travava o loop do agente pra
  sempre, já que a chamada só retorna quando o processo termina, e um dev server nunca termina
  sozinho. Padrão inspirado no `task`/`monitor`/`get_task_output` do grok-build e no `hub`
  (`start`/`ps`/`logs`/`stop`) do oh-my-pi, reduzido ao que o Cerne Code precisa — sem guarda de
  profundidade de subagente (o Cerne Code ainda não tem subagentes) nem categorias de resumo de
  output por ferramenta. `run_command(background=true)` spawna e devolve um id na hora sem
  esperar terminar; duas tasks assíncronas acumulam stdout/stderr num buffer limitado (últimas
  2000 linhas, tipo `tail -f` com limite, pra um dev server rodando horas não crescer sem limite
  de memória); `check_background_output(id)` lê o acumulado + status (rodando/encerrado) sem
  parar nada; `stop_background(id)` mata. Escopo global ao app (não por sessão) — simplificação
  razoável pra um app de um usuário só, já que o id (UUID) devolvido na hora de iniciar já aponta
  pro processo certo sem ambiguidade. **Bug real encontrado testando ao vivo, não nos testes**:
  todo comando roda via `cmd /C <command>` no Windows, então o processo de verdade (`python`,
  `node`, etc.) é **filho** do `cmd.exe` que o tokio rastreia — matar só o `cmd.exe`
  (`child.start_kill()`) não mata os filhos automaticamente no Windows. Confirmado ao vivo: um
  `python server.py` continuou respondendo por HTTP depois do `stop_background` reportar
  sucesso. Corrigido com `taskkill /PID <pid> /T /F` (mata a árvore inteira pelo PID) — mas isso
  revelou um **segundo bug**, uma race condition: o `Child` foi criado com `kill_on_drop(true)`,
  então dropar o valor cedo demais (antes do `taskkill` terminar) matava o `cmd.exe` sozinho
  primeiro, e o `/T` do `taskkill` precisa que o processo pai ainda exista no momento da chamada
  pra conseguir montar a árvore de descendentes — se o pai já morreu, o `taskkill` falha com
  "processo não encontrado" e os filhos ficam órfãos do mesmo jeito. Corrigido mantendo o valor
  vivo até o `taskkill` terminar. Achado depurando um teste de regressão que falhava só às vezes
  (checando por PID específico via PowerShell/CIM, não por nome de imagem — nome de imagem
  colide com outro teste rodando `ping` em paralelo no mesmo processo de teste). 8 testes
  unitários em `background.rs::tests` (start/read/stop, listar, id desconhecido, e o teste de
  regressão específico do bug do processo órfão) + 1 teste end-to-end via `execute_project_tool`.
  **Testado ao vivo duas vezes**: a primeira rodada expôs os dois bugs acima (servidor Python
  continuou respondendo depois do "stop" reportar sucesso); depois de corrigir e reiniciar o
  app, a segunda rodada confirmou o processo realmente morto (`tasklist` vazio, `curl` com
  timeout de conexão recusada).
- **Subagentes (`task`)** (`agent/subagent.rs`) — delega uma sub-tarefa bem definida pra um
  agente descartável que roda seu próprio loop de ferramentas (até concluir ou um limite de 8
  passos) e devolve só o relatório final — os passos intermediários não poluem o histórico da
  conversa principal, só aparecem como eventos de tool-call na UI (prefixados "↳ sub-agente").
  Padrão confirmado em duas fontes independentes (grok-build e opencode, ver seção 2.3.2/6.7 do
  `agent-architecture-research.md`): **guarda de profundidade que bloqueia recursão por padrão**
  — a única restrição que de fato importa (não uma lista extensa de permissões reduzidas): o
  sub-agente recebe `project_tool_specs()` inteiro **menos a própria `task`**, então
  estruturalmente não pode delegar pra outro sub-agente. Reduzido de propósito em relação ao
  opencode/grok-build: roda **síncrono** (bloqueia o turno do agente pai até terminar) em vez de
  background-com-notificação — mais simples, e o Cerne Code já tem esse mecanismo assíncrono pra outra
  coisa (`check_background_output`), não precisava duplicá-lo aqui; sem isolamento de filesystem
  por sub-agente (usa a mesma sandbox da sessão pai, sem `pi-iso`/git-worktree — fica pro item 6
  da lista de porte, só relevante com tasks concorrentes de verdade). Tratado à parte em
  `agent::mod::run_turn` (mesmo padrão já usado pra `load_skill`) em vez de mudar a assinatura de
  `tools::execute_tool` pra toda ferramenta — evitou precisar refatorar todos os testes
  existentes só pra uma ferramenta que precisa de mais contexto (app/provider) que as outras. 1
  teste unitário confirmando a guarda de profundidade (toolset do sub-agente = toolset do pai
  menos `task`, nada mais faltando). **Testado ao vivo com sucesso, mas revelou um bug real e
  sério em código já existente (não novo desta sessão)**: pedi pro sub-agente adicionar docstring
  em 3 funções do mesmo arquivo — `edit_file` sempre lia o arquivo REAL (que só muda quando o
  usuário aceita), então cada uma das 3 edições partia do mesmo original, sem ver as anteriores;
  ao aceitar as 3 na UI, só a **última** sobreviveu, perdendo as outras duas silenciosamente (sem
  erro nenhum, UI mostrando "tudo aceito"). Corrigido com `sandbox::read_current_content`
  (`src-tauri/src/sandbox.rs`) — prefere ler da sandbox, se já existir uma edição anterior
  pendente pro mesmo arquivo, em vez do arquivo real, fazendo cada edição nova nascer em cima da
  anterior. Esse bug preexistia desde que `edit_file`/`ast_edit` foram implementados (afeta
  qualquer sequência de edições no mesmo arquivo antes de aceitar, com ou sem sub-agente — o
  sub-agente só expôs o caso de forma clara, chamando `edit_file` 3x em sequência pro mesmo
  arquivo). 1 teste de regressão confirmando que a 2ª edição acumula sobre a 1ª (não a
  substitui), mais um teste confirmando a invariante que também ajuda mesmo sem essa correção
  (`to_sandbox_path` é determinístico por arquivo — múltiplas edições no mesmo arquivo sempre
  compartilham o mesmo caminho de sandbox, então aceitar qualquer uma das entradas de diff na UI
  aplica o estado cumulativo mais recente). **Testado ao vivo de novo depois da correção**:
  mesmo cenário (3 docstrings no mesmo arquivo), diff mostrado na UI já veio cumulativo (2
  funções no mesmo diff, não isoladas), e as 3 docstrings sobreviveram no arquivo real depois de
  aceitar tudo.
- **Guarda de loop** (`agent/mod.rs::is_doom_loop`) — se as últimas 3 chamadas de ferramenta
  executadas (mesmo dentro do sub-agente) forem a mesma ferramenta com os mesmos argumentos
  brutos, o Cerne Code para a execução e avisa numa mensagem visível ("⚠️ Parei a execução...") em vez
  de continuar chamando o modelo indefinidamente. Mesmo valor/ideia do `DOOM_LOOP_THRESHOLD = 3`
  do opencode (`packages/opencode/src/session/processor.ts`, seção 2.3.2/6.7 do research doc),
  mas reduzido: o opencode **pausa e pede permissão** pra continuar (tem infraestrutura de
  permissão mid-turn); o Cerne Code **para e avisa**, sem pedir permissão pra continuar automaticamente
  — mais simples e mais seguro como default sem essa infraestrutura ainda. Olha só a **janela
  final** de 3 chamadas (não um contador global), então uma chamada diferente no meio reseta a
  detecção — argumentos diferentes na mesma ferramenta (progresso real, tipo `read_file` em 3
  arquivos diferentes) nunca contam como loop. 5 testes unitários em `agent::tests` cobrindo:
  abaixo do limiar, 3 repetições idênticas detectadas, argumentos diferentes não contam, uma
  chamada diferente no meio reseta a janela, e só a janela final importa (histórico anterior
  repetido não conta se a mais recente quebrou o padrão).
- **MCP (Model Context Protocol)** (`src-tauri/src/mcp.rs`) — conecta em servidores MCP externos
  configurados pelo usuário e expõe as ferramentas deles pro agente igual as embutidas, só que
  rodando fora do processo do Cerne Code. Usa `rmcp` (SDK oficial em Rust do
  `modelcontextprotocol.io`/`github.com/modelcontextprotocol/rust-sdk`) direto — nada pra
  vendorizar, mesmo padrão das outras portas desta sessão. Só transporte stdio (`TokioChildProcess`
  — sobe o servidor como subprocesso, fala JSON-RPC pela stdin/stdout), que cobre a maioria dos
  servidores MCP reais distribuídos hoje (`npx pacote`, `uvx pacote`, binário local); SSE/HTTP
  streamable ficam de fora por enquanto. Tools de servidor MCP aparecem namespaced
  `mcp__{servidor}__{tool}` (mesma convenção de clientes MCP reais, evita colisão entre
  servidores). Disponível em toda sessão (com ou sem projeto, igual `web_search`) e também no
  sub-agente (`task`) — não é um risco de recursão, então não é restringido pela guarda de
  profundidade como a própria `task` é. Config em `mcp_servers.toml` (mesmo padrão de
  `providers.toml` pros forks de llama.cpp), com tela de gerenciamento em Configurações → 
  "Servidores MCP" (adicionar/habilitar/desabilitar/remover). **Achado real testando ao vivo**:
  primeira tentativa falhou com "program not found" ao conectar via `npx` — no Windows, comandos
  do ecossistema Node (`npx`, `npm`) são shims `.cmd`, e `Command::new` do Rust não resolve isso
  sozinho (não passa pelo `PATHEXT` que o `cmd.exe` resolveria), mesmo com `npx` funcionando
  normal no terminal. Mesmo motivo pelo qual `run_command`/`background.rs` já envolvem tudo em
  `cmd /C` no Windows — aplicada a mesma correção aqui (só no Windows; outros SOs rodam o comando
  direto). 6 testes unitários em `mcp.rs::tests` (parsing do nome namespaced, round-trip de
  config) + 1 teste de roteamento em `tools.rs` (chamada `mcp__` sem servidor conectado dá erro
  claro, não "ferramenta desconhecida"). **Testado ao vivo de ponta a ponta** depois da correção:
  servidor de referência oficial (`npx -y @modelcontextprotocol/server-everything`) configurado
  pela UI de Configurações, sessão real (`qwen3.5-9b-mtp`) chamou `mcp__everything__echo` de
  verdade e recebeu "Echo: ola do servidor MCP" de volta — round-trip completo por um processo
  externo real. Confirmado também que o processo do servidor não fica órfão ao fechar o app
  (verificado via `tasklist` depois de encerrar o Cerne Code) — `disconnect_all()` existe mas não está
  conectado a nenhum hook de shutdown ainda; o cleanup observado vem do comportamento padrão do
  runtime/SO ao encerrar o processo pai, não de um shutdown gracioso explícito (item de polish,
  não bug confirmado).
- **`ask` (pergunta estruturada mid-turn)** (`agent/mod.rs::ask_user`, tool `ask`) — o modelo pode
  pausar o turno e perguntar algo específico ao usuário (múltipla escolha e/ou texto livre) antes
  de continuar, em vez de assumir uma opção sozinho. Mesmo padrão do `question` do opencode (seção
  2.3.2 do research doc). Mecanismo: um canal `tokio::sync::oneshot` — a task async do `run_turn`
  (que já roda solta via `tauri::async_runtime::spawn` em `send_message`, não bloqueia nada) fica
  literalmente parada num `.await` no lado receptor do canal, sem precisar serializar/retomar
  estado em disco; o comando Tauri `answer_ask` (disparado quando o usuário clica numa opção ou
  manda texto livre) só precisa mandar a resposta pelo lado emissor. Simples porque o Rust async já
  resolve "pausar e continuar de onde parou" de graça — não precisei de uma máquina de estados
  própria. UI: card amarelo (`AskCard.vue`) com botões de opção + campo de texto livre, aparece no
  fluxo da conversa igual o `DiffReview` de edição pendente. Disponível em toda sessão (com ou sem
  projeto, igual `web_search`) mas **não** disponível pro sub-agente (`task`) — vive em
  `always_tool_specs()`, que o sub-agente não herda (só herda `project_tool_specs()`), então fica
  de fora por construção, sem precisar de filtro explícito; intencional, um sub-agente autônomo não
  devia interromper o usuário no meio de uma delegação, só reportar no final. **Testado ao vivo**:
  pedi pro modelo perguntar "qual linguagem você quer usar" com 3 opções — o card apareceu com os 3
  botões + campo de texto, cliquei numa opção, o turno retomou de verdade incorporando a resposta
  ("Entendido! Vamos criar um projeto em **Rust**."). Sem teste automatizado (precisaria de um
  `AppHandle` real do Tauri pra emitir evento, como o `mock_app()` da própria crate — não
  implementado ainda, ficou só verificação ao vivo).
- **"Goal mode" com verificador adversarial** (`agent/verifier.rs`, tool `verify_completion`) —
  antes de declarar uma tarefa complexa concluída, o modelo pode disparar um verificador
  independente e **cético** (não a mesma entidade que alega ter terminado) que reconfere com
  evidência real — nunca só a narrativa de quem alega sucesso. Padrão vem do "goal mode" do
  grok-build (seção 3.3 do research doc): quando o modelo chama `update_goal(completed: true)`, o
  harness deles dispara um "painel cético" de subagentes verificadores. Reduzido de propósito: sem
  critério de aceite "congelado" antes de começar nem cutucão a cada turno — só a peça que a
  pesquisa aponta como a que realmente muda o resultado (o veredito adversarial em si), acionada
  sob demanda pelo próprio modelo via uma tool, não um "modo" separado com config própria. Reusa a
  mesma máquina de loop de ferramentas do `subagent.rs` (`task`), só com prompt e toolset
  diferentes. **Toolset é só leitura/execução** (`read_file`/`list_dir`/`grep`/`ast_grep`/
  `run_command`, sem `write_file`/`edit_file`/`ast_edit`) — o verificador só observa e reporta,
  nunca "conserta" o que encontrar. Prompt assume REFUTADO por padrão quando incerto, e exige que
  o veredito comece com a palavra exata "APROVADO" ou "REFUTADO" na primeira linha — se o modelo
  verificador não seguir esse formato, o veredito vira REFUTADO por segurança em vez de
  silenciosamente aprovar algo mal confirmado (`extract_verdict`). Prompt também trata
  explicitamente o caso de edição ainda não aceita na sandbox como "pendente", não "falhou" — mas
  isso só vale pra verificação por LEITURA de código; se a verificação pedida for **rodar um
  teste de verdade** contra o arquivo real, um REFUTADO nesse caso é o comportamento correto (o
  teste genuinamente falha até o usuário aceitar o diff). Mesma guarda de profundidade do
  sub-agente (sem `task`/`ask`/`verify_completion` recursivo). 4 testes unitários (toolset
  allowlist, parsing do veredito em 3 variações + fallback REFUTADO quando o formato não é
  seguido). **Testado ao vivo com um caso real e completo**: pedi pra implementar uma função e
  verificar rodando um teste Python de verdade — primeira chamada de `verify_completion` deu
  **REFUTADO** (a edição ainda só estava na sandbox, então o teste rodado contra o arquivo real
  genuinamente falhava — o modelo principal corretamente NÃO alegou sucesso, explicando o motivo
  exato); depois de aceitar o diff na UI, pedi pra reverificar — segunda chamada deu **APROVADO**
  com evidência concreta (`exit_code: 0`, saída real do teste "TODOS OS TESTES PASSARAM"), e só aí
  o modelo declarou sucesso. Round-trip completo do design pretendido, incluindo o caso adversarial
  (rejeitar até ter prova real) funcionando exatamente como pesquisado.
- **Pastas extras de leitura por sessão** (`Session.extra_read_paths`, comando Tauri
  `update_session_read_paths`) — `read_file`/`list_dir`/`grep`/`ast_grep` aceitam caminho
  ABSOLUTO dentro de uma allowlist de pastas configurada por sessão, além do `project_root`
  normal (útil pra referenciar outro repo/documentação sem abrir o disco inteiro).
  `write_file`/`edit_file`/`ast_edit` continuam restritos ao `project_root` de propósito — a
  sandbox de edição só espelha ele, então escrever fora dele não teria onde ficar "pendente de
  aceite". Resolução centralizada em `tools::resolve_within` (`resolve_path` pras ferramentas de
  escrita, sem extra roots; `resolve_read_path` pras de leitura, com extra roots) — mesma
  verificação `canonicalize()` + `starts_with()` de antes, só que contra uma lista de raízes
  permitidas em vez de uma só. UI: botão de pasta ao lado do seletor de provider/modelo no
  composer (só aparece com `project_root` configurado), abre um popover listando as pastas
  extras com nome no hover (`v-tooltip`) e um "x" discreto pra remover — sem limite de
  quantidade. 5 testes unitários novos (aceita caminho dentro de extra root, rejeita caminho fora
  de qualquer root permitida, `list_dir`/`grep` com extra root, e a garantia negativa de que
  `write_file` recusa mesmo com extra roots configuradas). **Testado ao vivo**: criei uma sessão
  com projeto, adicionei duas pastas extras via UI (popover mostrou os nomes certos e o badge de
  contagem), removi uma pela UI e confirmei que a lista e o badge atualizaram e que a mudança
  persistiu no `session.json` via round-trip do comando Tauri.
- **Renderização de markdown de verdade no chat** (`src/markdown.ts`, `MarkdownContent.vue`) —
  antes o conteúdo das mensagens era mostrado cru (`**negrito**` aparecia literal, sem
  formatação); agora usa `markdown-it` de verdade (negrito, itálico, listas, tabelas,
  blockquote, links) com `highlight.js` pros blocos de código, sempre passando o HTML resultante
  por `DOMPurify.sanitize` antes de ir pro `v-html` — o modelo não é uma fonte confiável de HTML
  (prompt injection poderia tentar smugglar `<script>`/`<img onerror>` numa resposta), então
  sanitizar não é opcional. `highlight.js` importado via `/lib/core` + só as linguagens que o
  Cerne Code já usa (mesma lista do `ast_grep`, ver `ast_tools.rs::SUPPORTED_LANGS`) em vez do pacote
  cheio (~190 gramáticas) — reduziu o bundle de ~1.58MB pra ~725KB. Um componente
  `MarkdownContent.vue` compartilhado entre `MessageBubble.vue` (mensagens finais) e o balão de
  streaming em `ChatView.vue` (texto chegando token a token já renderiza como markdown, não só
  no final), com uma variante `dark` pro balão escuro do usuário. **Testado ao vivo**: injetei
  mensagens de teste direto na store Pinia (via `document.querySelector('#app').__vue_app__`,
  sem precisar de inferência real) cobrindo negrito/itálico/lista/tabela/bloco de
  código-com-highlight/blockquote/link — tudo renderizou certo, incluindo a variante escura no
  balão do usuário; e confirmei que uma tentativa de XSS (`<img onerror=...>` +
  `<script>...</script>`) apareceu como texto escapado na tela, sem disparar o handler nem
  executar o script (`window.__xss_fired` continuou `undefined`).
- **Texto explicativo sobre ferramentas dependerem de pasta de projeto** (`ComposerBar.vue`) —
  em vez de separar "chat" e "code" como modos distintos (o roteamento já era de facto assim:
  uma sessão sem `project_root` já só tinha `web_search`/`web_fetch`/`ask`/MCP disponível, ver
  `project_tool_specs()`), só um aviso no composer quando a sessão atual não tem pasta de
  projeto associada, deixando explícito que sem ela o agente só busca na web e usa MCP.
  **Testado ao vivo**: aviso aparece corretamente numa sessão sem `project_root` e some assim
  que a sessão tem uma pasta associada.
- **Anexar arquivos no composer** (`attachments.rs`, comando Tauri `extract_attachment_text`,
  `ComposerBar.vue`) — botão "+" do composer agora abre um seletor nativo multi-arquivo
  (`@tauri-apps/plugin-dialog`, `multiple: true`, sem limite de quantidade) pra pdf/docx/xlsx/
  xlsm/xls/ods/md/txt/código; cada arquivo tem o texto extraído via crate real (`pdf-extract`
  pra PDF, `calamine` pra planilha, `docx-rust` pra Word — percorrendo `body.content` de
  verdade: parágrafo/run/tabela/hyperlink/SDT, não só o texto solto) e injetado como um bloco
  `### Anexo: nome\n\n<texto>` antes da mensagem digitada, com o mesmo corte de 20.000
  caracteres do `read_file` pra não estourar o contexto sozinho. Imagem/áudio/vídeo ficam de
  fora de propósito — dependem de suporte multimodal real por provider, que ainda precisa ser
  pesquisado caso a caso (gemma4 no llama.cpp sem o `.mmproj` carregado, por exemplo, não tem
  visão de verdade mesmo o modelo base suportando — ver próxima tarefa antes de prometer isso na
  UI). UI mostra cada anexo como um chip com nome truncado + tooltip com o caminho completo no
  hover, spinner enquanto extrai, estado de erro visível, e um "x" discreto pra remover — a
  mensagem exibida no histórico do usuário mostra só o texto digitado + nome dos anexos (📎), não
  o conteúdo extraído inteiro, que só vai pro que é enviado ao modelo (`sessionStore.send` ganhou
  um segundo parâmetro `displayText` pra essa separação). 5 testes unitários novos em
  `attachments.rs` (texto puro, truncamento, docx com parágrafo+tabela via round-trip real
  escrevendo com o próprio `docx-rust` e lendo de volta, xlsx com célula numérica e texto via um
  `.xlsx` real montado à mão). **Testado ao vivo parcialmente**: confirmei via IPC direto que
  `extract_attachment_text` funciona ponta a ponta contra um arquivo real, e que o botão "+" está
  habilitado e disparando o handler certo (sem erro no console); o seletor nativo de arquivos do
  Windows em si não dá pra automatizar via CDP (fora do DOM da página, mesma limitação já
  documentada no `cdp.mjs` pro picker de pasta) — o fluxo completo de escolher arquivo de verdade
  fica pra confirmação manual do usuário.
- **Upload de imagem no composer com detecção real de vision** (`providers::supports_vision`,
  `providers::to_wire_messages`, `llama_cpp::preset_supports_vision`, comandos Tauri
  `check_vision_support`/`read_image_as_data_url`, `ComposerBar.vue`) — construído em cima da
  pesquisa da seção abaixo. `ChatMessage` ganhou um campo `images: Vec<String>` (data URIs
  base64); `to_wire_messages` reescreve o `content` pro array multi-parte da OpenAI
  (`{"type":"text",...}` + `{"type":"image_url",...}`) só quando a mensagem carrega imagem,
  então os 4 providers (todos batem no mesmo `/chat/completions` OpenAI-compatible) recebem o
  formato certo sem precisar de código separado por provider nessa parte. A UI só deixa anexar
  imagem depois de perguntar de verdade pro provider (`check_vision_support` chama `/api/show`
  no Ollama, `/models` com `architecture.input_modalities` no OpenRouter, `/api/v0/models` com
  `type=vlm` no LM Studio, ou confere se o preset do `.ini` do llama.cpp tem uma chave
  `mmproj`/`clip` configurada) — nunca assume vision pelo nome do modelo. Suporta colar direto
  (Ctrl+V no textarea, formato de imagem lido via `FileReader` no próprio navegador, sem round-trip
  de arquivo) e escolher pelo "+", com chip mostrando miniatura, nome, e "x" pra remover. 9 testes
  unitários novos (conversão pro formato wire com 1 e várias imagens, preset com/sem mmproj
  configurado). **Bug real encontrado e corrigido testando ao vivo**: o chip de anexo (tanto
  imagem quanto documento) ficava preso em "carregando" pra sempre — mutar o objeto guardado
  antes dele entrar no array reativo do Vue muda o objeto "cru", não o proxy que o Vue observa,
  então a tela nunca via a atualização de status. Corrigido guardando só o `id` e buscando o item
  de volta no array reativo (`updateAttachment`) antes de mutar. **Testado ao vivo com modelo
  real**: `check_vision_support` retornou `true` pro `gemma4:e4b` local via Ollama (confirmado
  batendo direto no `/api/show` real, que retorna `capabilities: ["vision", ...]`) e `false` pra
  um modelo inexistente; enviei uma imagem real (ícone do app, 128×128) pro `gemma4:e4b` via
  Ollama e o modelo descreveu corretamente ("imagem predominantemente preta, com padrões escuros e
  abstratos") — confirma que `to_wire_messages` chega certo no provider de verdade, não só em
  teste unitário isolado. Round-trip de colar imagem testado ao vivo simulando um evento `paste`
  de verdade na janela real do app.
- **Servidores MCP em `.json` + editar pela UI + testar conexão antes de salvar**
  (`mcp.rs`, comando Tauri `test_mcp_server`, `Settings.vue`) — configuração migrada de
  `mcp_servers.toml` pra `mcp_servers.json` no formato `{"mcpServers": {"nome": {...}}}` (objeto
  por nome), o mesmo que a maioria dos servidores MCP reais já documenta no README pra colar
  direto — migração automática e silenciosa do `.toml` antigo na primeira leitura, se o `.json`
  ainda não existir. A UI agora deixa editar um servidor já salvo (não só adicionar/remover) e tem
  campo de variáveis de ambiente (`CHAVE=valor`, uma por linha). Botão "Testar conexão" sobe o
  servidor numa conexão descartável (nunca entra no pool compartilhado de `McpClients`), faz o
  handshake MCP e lista as tools reais antes de salvar — mesma ideia do teste de conexão que o LM
  Studio faz, com timeout de 15s e mensagens de erro específicas (comando não encontrado,
  handshake que não responde, etc.) em vez de só "falhou". **Testado ao vivo com servidor real**:
  configurei `npx -y @modelcontextprotocol/server-everything`, cliquei em "Testar conexão" e
  recebi de volta as 13 ferramentas reais desse servidor (`echo`, `get-env`, `get-tiny-image`,
  etc.); salvei, confirmei o `mcp_servers.json` gerado no formato certo, editei o servidor salvo
  (campos vieram preenchidos), e removi — todo o ciclo funcionando contra um processo MCP de
  verdade, não um mock.
- **Forks llama.cpp configuráveis pela UI, sem hardcode** (`providers/llama_cpp.rs`, comandos
  Tauri `add_llama_fork`/`remove_llama_fork`, `Settings.vue`) — antes, `default_forks()` escrevia
  automaticamente dois forks hardcoded apontando pro layout de pastas específico desta máquina de
  desenvolvimento (`C:\ai-turboquant\...`), o que não faz sentido pra uma distribuição open
  source (nenhum outro usuário vai ter essas pastas). Agora uma instalação nova começa com
  **zero** forks configurados, e a tela de Configurações ganhou um formulário (id, rótulo, porta,
  seletor de arquivo pro `llama-server.exe` e pro `models.ini`) pra adicionar quantos forks
  quiser, com "Remover" em cada linha — tudo persistido no mesmo `providers.toml` de antes, só
  que agora editável pela UI em vez de só por escrita manual do arquivo. A mudança não afeta
  quem já tinha um `providers.toml` de uma versão anterior (só o comportamento de primeira
  instalação mudou). 4 testes unitários novos (lista vazia em instalação nova, add/load
  roundtrip, upsert por id em vez de duplicar, remove só o id certo). **Testado ao vivo**:
  adicionei um fork customizado pela UI (apareceu na lista, escrito certo no `providers.toml`
  junto dos 2 forks reais já configurados nesta máquina, sem duplicar nem perder nenhum) e removi
  em seguida, confirmando que só o fork removido saiu do arquivo.
- **Provider "customizado" genérico — Claude/Grok/ChatGPT/Qwen/Kimi/qualquer OpenAI-compatible
  sem hardcode** (`ProviderKind::Custom`, `providers/custom.rs`, comandos Tauri
  `list_custom_providers`/`test_custom_provider`/`add_custom_provider`/`remove_custom_provider`,
  seção "Providers customizados" em `Settings.vue`) — pesquisei os 5 providers que o usuário
  pediu (Claude, Grok/xAI, ChatGPT/OpenAI, Qwen/DashScope, Kimi/Moonshot) e confirmei que **todos
  falam o formato de chat completions da OpenAI** (Claude via seu shim oficial em
  `https://api.anthropic.com/v1/`, embora a Anthropic recomende a Messages API nativa pra
  features avançadas que o Cerne Code não usa hoje — PDF nativo, citations, prompt caching; os outros
  4 são OpenAI-compatible nativamente). Em vez de hardcodar um `ProviderKind` por vendor (o que
  ainda seria hardcode, só que uma lista maior — e sempre incompleta pra distribuição open
  source), adicionei **um único kind genérico `Custom`**: o usuário cadastra id/rótulo/URL
  base/chave via Settings, e a mesma infraestrutura OpenAI-compatible que já existia
  (`chat_stream`/`list_models`, usada por OpenRouter/Ollama/LM Studio) atende qualquer um desses
  vendors — e qualquer outro que apareça depois — sem uma linha de código específica de vendor.
  Chave de API guardada no keyring do SO (mesmo padrão já usado pra chave do OpenRouter), nunca
  em texto puro; UI mostra "Testar conexão" **sem persistir nada** (comando dedicado
  `test_custom_provider` recebe URL/chave direto do formulário, chama `/models` de verdade,
  descarta a conexão) antes do usuário confirmar "Adicionar". Cada conexão configurada aparece
  como sua própria entrada de "conexão" no seletor do composer/nova sessão (mesmo padrão de
  segundo nível já usado pra fork do llama.cpp), mostrando "rótulo · modelo" igual aos 4
  providers embutidos já mostravam. `Session` ganhou `custom_provider_id` (mesmo papel do
  `llama_fork`) e `AppConfig` ganhou `active_custom_provider_id`; toda a resolução de
  URL/chave/modelo pra `Custom` passa por uma única função (`build_provider_config`, reusada
  tanto pelos comandos Tauri quanto pelo `agent::provider_config_for` do turno real). Vision
  fica conservadoramente `false` pra qualquer `Custom` (não há campo padrão de modalidade no
  `/models` genérico da OpenAI — o `architecture.input_modalities` usado pra detectar vision no
  OpenRouter é específico dele). 4 testes unitários novos (lista vazia/roundtrip/upsert/remove,
  mesmo padrão dos testes de fork). **Testado ao vivo com conexão real**: cadastrei o próprio
  Ollama local como "provider customizado" (via `http://127.0.0.1:11434/v1`, o mesmo endpoint
  OpenAI-compatible que o Ollama já expõe) só pra provar que o caminho genérico funciona de
  ponta a ponta contra um endpoint de verdade sem nenhum código específico de Ollama — "Testar
  conexão" achou os 28 modelos reais da máquina, salvei, criei uma sessão nova escolhendo
  "Customizado" → "Meu Ollama via Custom" → um modelo real, mandei "qual a capital do Brasil?" e
  recebi "Brasília." de volta, confirmando o `chat_stream`/`to_wire_messages` genérico
  funcionando com uma conexão customizada de verdade, não só em teste unitário isolado.
- **Renomear e excluir sessão pela lista lateral** (`sessions::update_title`, comando Tauri
  `update_session_title`, `Sidebar.vue`) — cada item da lista agora tem um lápis e uma lixeira
  que só aparecem no hover, sem poluir a lista o tempo todo. O lápis vira o título num campo de
  texto inline (foco automático, texto já selecionado), confirma com Enter ou ao clicar fora
  (`blur`), cancela com Escape; a lixeira exclui a sessão direto (mesmo padrão sem confirmação já
  usado pros outros "Remover" do app — fork/MCP/provider customizado). Um teste unitário novo.
  **Testado ao vivo**: renomeei uma sessão real pela lista (o campo herdou o texto certo com
  seleção pronta, Enter confirmou e persistiu no `session.json`) e excluí outra em seguida
  (sumiu da lista e a pasta da sessão saiu do disco).
- **Modo Manual vs Automático de execução — primeiro sistema de permissão do Cerne Code**
  (`ExecutionMode`, `agent::request_permission`, comandos Tauri `update_session_execution_mode`/
  `answer_permission`/`cancel_turn`, `PermissionCard.vue`, seletor no composer) — até aqui
  nenhuma tool call pedia confirmação antes de rodar. Agora cada sessão tem um modo:
  **Manual** pausa ANTES de rodar qualquer tool call (exceto a própria `ask`, que já é uma pausa
  esperando o usuário — pedir permissão pra perguntar seria só redundante) e só prossegue com
  aprovação explícita; **Automático** (default, não muda nada pra quem já usa o Cerne Code) roda livre,
  com um botão "Cancelar" na lista de tarefas lateral que aborta o turno inteiro a qualquer
  momento. Mesmo padrão de canal `oneshot` que o `ask` já usava (`request_permission` suspende a
  task async esperando `answer_permission`, sem precisar serializar/retomar estado). Cancelamento
  usa `JoinHandle::abort()` sobre a task inteira do turno (guardada em `AppState.running_turns`
  por sessão) — derruba a chamada HTTP em andamento na hora, não espera nenhum checkpoint
  cooperativo. Escopo desta primeira versão: só o loop principal do turno é gated — sub-agente
  (`task`) e verificador (`verify_completion`) ainda rodam suas próprias tool calls sem passar
  pelo gate (ficaria pra uma proxima rodada se via a valer a pena). 3 testes unitários novos
  (sessão nova sempre nasce em `Auto`, `update_execution_mode` persiste, roundtrip). **Testado
  ao vivo com modelo real, os três caminhos**: (1) modo Manual — pedi pra listar arquivos, o
  card de permissão apareceu mostrando a tool (`list_dir`) e os argumentos formatados, aprovei,
  a chamada rodou e o modelo errou o caminho na primeira tentativa, se autocorrigiu e pediu
  permissão de novo pra um `list_dir` diferente, aprovei, e recebi a listagem real da raiz do
  projeto; (2) recusar — pedi pra ler um arquivo, cliquei "Recusar" duas vezes seguidas em
  tentativas diferentes (`read_file` depois `run_command`) e confirmei que o modelo recebeu
  "Ação negada pelo usuário." e tentou caminhos alternativos, exatamente como esperado; (3)
  cancelar — com uma permissão pendente, cliquei "Cancelar" na lista de tarefas e o turno
  inteiro parou na hora, com a mensagem "Execução cancelada pelo usuário." aparecendo e a UI
  voltando pro estado idle; (4) modo Automático — troquei pra "Auto" e mandei um pedido
  parecido: o `read_file` rodou direto sem nenhum card de permissão, confirmando que o
  comportamento de quem não usa Manual continua exatamente como antes.
- **Template pré-formatado ao criar skill, em pt-br ou inglês** (`SkillLanguage`, comandos Tauri
  `create_skill` (ganhou o parâmetro `language`), `read_skill`/`save_skill` novos, seletor de
  idioma + editor inline em `Settings.vue`) — antes o `create_skill` gerava só uma linha genérica
  ("Descreva aqui o passo a passo..."); agora o corpo inicial do `SKILL.md` vem com seções
  (Objetivo/Purpose, Quando usar/When to use, Passo a passo/Instructions, Exemplo/Example) que
  guiam o usuário no que preencher, no idioma escolhido num `<select>` ao lado do botão "Criar"
  (o resto da UI do Cerne Code continua só em pt-br — não tem infra de i18n, só o corpo da skill em si
  muda de idioma). Também virou possível editar o `SKILL.md` inteiro (frontmatter + corpo) direto
  pela tela — botão "Editar" expande uma textarea que carrega o conteúdo atual via `read_skill` e
  salva ao perder o foco ou pelo ícone de disquete, mesmo padrão de "salva das duas formas" já
  usado no rename de sessão, sem precisar abrir a pasta num editor externo pra isso. 1 teste
  unitário ajustado pro novo parâmetro. **Testado ao vivo**: criei uma skill em inglês pela tela,
  confirmei que o template (com as 4 seções) apareceu certinho no editor inline, editei o corpo
  adicionando uma linha extra, fechei e reabri o editor — o `read_skill` trouxe de volta o texto
  editado direto do disco, confirmando que o `save_skill` no blur persistiu de verdade (não só
  ficou no estado da tela).
- **Busca na web sem depender de instalar nada** (`src/search.rs` novo, `agent/websearch.rs`
  reescrito, tela "Busca na web" em `Settings.vue`) — antes `web_search` só funcionava contra um
  container SearXNG local hardcoded (`http://127.0.0.1:8888`), que o usuário precisava ter
  instalado e rodando via Docker; sem isso, a ferramenta simplesmente falhava. Pesquisei como
  opencode (plugins da comunidade, já que o core não traz busca nativa) e outras ferramentas de
  agente resolvem isso e copiei o padrão mais comum: por padrão (`Auto`) a busca sai via HTML do
  DuckDuckGo (`html.duckduckgo.com/html/`, parseado com `scraper`, desembrulhando o redirect
  `uddg` e filtrando anúncios patrocinados) — nenhuma conta, nenhuma chave, nenhum instalador.
  Quem quiser resultados melhores pode trocar pra **Brave Search API** ou **Tavily** (chave salva
  no keyring, mesmo padrão de segredo dos providers customizados) ou apontar pra uma instância
  própria de **SearXNG** (comportamento de antes, preservado, só que configurável em vez de
  hardcoded). Botão "Testar conexão" roda uma busca de verdade com os valores do formulário antes
  de salvar, mesmo padrão do MCP/providers customizados. 8 testes unitários novos (roundtrip do
  config, parsing do HTML do DuckDuckGo com e sem resultados, compat com config antigo sem o campo
  `searxng_url`). **Testado ao vivo**: rodei o "Testar conexão" em modo Automático e recebi 10
  resultados reais; troquei pra Brave sem chave configurada e recebi o erro certo ("informe a
  chave de API da Brave") em vez de um 500 genérico; voltei pra Automático, salvei, e no chat pedi
  pro agente buscar a versão mais recente do Rust — ele chamou `web_search` de verdade (visível na
  lista de tarefas), voltou com resultados reais do DuckDuckGo e respondeu corretamente com a
  versão certa, tudo sem Docker nem SearXNG rodando na máquina.
- **Busca `Auto` virou multi-engine (inspirada no motor de agregação de verdade do SearXNG),
  e o agente ganhou autonomia pra decidir quantas buscas fazer numa chamada** — a entrega
  anterior desta mesma tarefa usava só o DuckDuckGo no modo `Auto`; a pedido do usuário fui
  estudar o código-fonte do SearXNG (`searx/results.py` pro merge/ranking, `searx/engines/
  duckduckgo.py`/`brave.py`/`mojeek.py` pros parsers de cada motor) em vez de só reusar a API
  JSON dele. `search_auto` (`agent/websearch.rs`) agora dispara DuckDuckGo + a página pública do
  Brave (não a API paga) + Mojeek em paralelo (`tokio::join!`), normaliza cada URL (sem `www.`,
  sem barra final, sem parâmetros de tracking tipo `utm_*`/`fbclid`) pra deduplicar entre motores,
  e rankeia por pontuação somada `1/(posição+1)` de cada motor que trouxe aquele resultado — mesma
  ideia do `weight/position` do SearXNG: um resultado que aparece bem colocado em duas fontes sobe
  acima de um que só uma fonte trouxe. Se um motor cair ou bloquear o scraping, os outros sustentam
  a busca em vez do recurso falhar inteiro. Além disso, `web_search` trocou o parâmetro `query`
  (string única) por `queries` (array) — a description da ferramenta agora diz explicitamente pro
  modelo decidir quantas queries mandar na mesma chamada (uma búsqueda direta, várias quando o
  pedido tem múltiplos ângulos ou a primeira busca não trouxe o suficiente), com um teto de 5 por
  chamada contra abuso. 6 testes unitários novos (parser do Brave, parser do Mojeek, normalização
  de URL, merge com dedup e ranking por consenso). **Testado ao vivo**: rodei a busca real (fora
  do app, via teste ignorado) e confirmei 12 resultados deduplicados combinando as 3 fontes; no
  chat pedi pro agente "pesquisar as versões mais recentes do Rust e do Python" — ele decidiu
  sozinho mandar `web_search({"queries":["latest stable rust version","latest stable python
  version"]})` numa única chamada (visível na lista de tarefas) e respondeu com as duas versões
  certas (Rust 1.97.1, Python 3.14.6), confirmando tanto a agregação multi-fonte quanto a
  autonomia de decidir o número de buscas.
- **Bug crítico corrigido: nenhuma chave de API era realmente salva no Windows** (`Cargo.toml`) —
  descoberto testando por que um provider customizado (Qwen) conectava certo em "Testar conexão"
  mas dava `401 Unauthorized` toda vez que uma sessão de verdade tentava listar/usar os modelos.
  Causa raiz: a dependência `keyring = "3"` estava sem nenhuma feature especificada, e a partir da
  major 3 dessa crate não existe feature padrão — sem indicar o backend explicitamente, ela cai
  num "mock store" silencioso (tudo parece funcionar, `set_password` nunca erra, mas
  `get_password` nunca acha nada, porque nada é escrito de verdade). Isso não era um problema só
  do Qwen: **toda** chave que já foi "salva" no Cerne Code no Windows (OpenRouter, qualquer
  provider customizado) nunca tinha sido persistida de fato — só parecia funcionar porque
  "Testar conexão" sempre usa a chave recém-digitada direto, sem passar pelo cofre. Corrigido
  adicionando a feature certa: `keyring = { version = "3", features = ["windows-native"] }`.
  **Testado ao vivo**: reproduzi o roundtrip quebrado via IPC direto (`set_openrouter_key` seguido
  de `has_openrouter_key` devolvendo `false` mesmo logo depois de salvar, e o mesmo padrão criando
  um provider customizado de teste) antes da correção; depois de adicionar a feature e recompilar,
  o mesmo roundtrip passou a devolver `true` — tanto pro OpenRouter quanto por um provider
  customizado novo. Quem já tinha "salvo" uma chave antes desta correção precisa reentrar ela uma
  vez (Configurações → Editar o provider → digitar a chave de novo → Salvar) pra ela realmente ir
  pro cofre desta vez.
- **Links em respostas do agente abrem no navegador padrão do SO, não dentro do app** (comando
  `open_external_url` novo em `lib.rs`, reusa o `tauri-plugin-opener` já usado por "Abrir pasta de
  skills"; `MarkdownContent.vue` intercepta clique em `<a>`) — antes, clicar num link de uma
  resposta (ex: fontes de uma busca na web) navegava a janela do WebView inteira pra aquela URL,
  substituindo a UI do Cerne Code. Achado testando o Qwen customizado ao vivo. **Testado ao vivo**:
  cliquei num link real de uma resposta anterior (fonte de previsão do tempo) — a janela do app
  permaneceu intacta (`location.href` continuou apontando pro próprio app), confirmando que o
  clique foi interceptado em vez de navegar a UI pra fora.
- **Contexto errado (8.2k) pra providers customizados** (`CustomProviderConfig.context_length`
  novo em `providers/custom.rs`, campo opcional na tela) — o `/models` de um endpoint
  OpenAI-compatible genérico (Qwen/DashScope, Claude via shim, etc.) não tem um campo padrão de
  tamanho de contexto — só a extensão própria do `/models` do OpenRouter tem isso — então
  `resolve_context_length` sempre caía no fallback `DEFAULT_CONTEXT_LENGTH` (8192), bem menor que
  o real pra modelos modernos. Agora dá pra informar manualmente o tamanho de contexto de uma
  conexão customizada, usado direto sem precisar consultar a API. **Testado ao vivo**: criei uma
  conexão de teste com `context_length: 131072` apontando pra um endpoint que nem responde no
  formato certo — `resolve_context_length` devolveu `131072` mesmo assim, confirmando que o
  override é aplicado antes de qualquer tentativa de consultar a rede.
- **Override de contexto virou editável direto no indicador da tela (não só via Configurações),
  botão de enviar virou quadrado de parar durante execução, e a timeline do chat passou a mostrar
  passos em linguagem natural em vez de bolhas vazias** — três pedidos do usuário depois de usar o
  Qwen customizado de verdade. `ContextGauge.vue` agora é clicável: abre um popover com input +
  Salvar/Cancelar, chamando o novo comando `update_session_context_length`
  (`sessions::update_context_length`) — resolve tanto o problema de compactação prematura (sessão
  cuja compactação usa `context_length` da sessão, presa em 8192 antes disso) quanto dá acesso
  rápido sem precisar ir em Configurações editar o provider inteiro. O botão de enviar
  (`ComposerBar.vue`) agora troca pra um quadrado vermelho (`stop`) enquanto
  `sessionStore.status !== 'idle'`, chamando o `cancelTurn()` que já existia (antes só acessível
  pelo botão "Cancelar" da lista de tarefas). A timeline do chat (`ChatView.vue` +
  `TaskStepGroup.vue` novo) intercala mensagens com um resumo por linha de cada tool call do turno
  (`web_search` → "Buscou na web", `run_command` → "Executou um comando", etc., mapeamento em
  `taskLabels.ts`), expansível por clique pra ver o rótulo cru e o resultado — e as mensagens de
  assistant vazias (só `tool_calls`, sem texto) pararam de renderizar como bolha em branco. Pra
  isso, `TaskItem` ganhou um campo `turn` (quantas mensagens de usuário existiam quando aquela
  tarefa nasceu), calculado uma vez no início de `run_turn`. O painel lateral "Tarefas desta
  sessão" continua exatamente como estava, sem nenhuma mudança. 1 teste unitário novo
  (`update_context_length_persists_and_can_be_reset_to_automatic`).
  **Bug real encontrado e corrigido durante o teste ao vivo**: o popover de editar contexto abria
  certinho mas o botão "Salvar" não fazia nada — nenhum erro visível, só ficava preso no modo de
  edição. Investigando com captura de console via CDP (não só clique cego), achei a causa:
  `type="number"` no input faz o `v-model` do Vue 3 converter o valor automaticamente pra `Number`
  (comportamento nativo desse tipo de input, diferente de um input de texto comum), então
  `inputValue.value.trim()` quebrava com "trim is not a function" — erro que o Vue captura e só
  loga no console do handler, sem re-lançar, por isso nada aparecia pra mim nem pro usuário.
  Corrigido com `String(inputValue.value ?? "").trim()`. **Testado ao vivo depois da correção**:
  cliquei no indicador numa sessão antiga (llama.cpp, sem tocar na sessão real do usuário que
  estava com um build rodando em paralelo), editei pra `131072`, salvei, recarreguei o app inteiro
  e confirmei que persistiu (`1.6k / 131.1k`); mandei uma mensagem que força `run_command` e
  capturei o botão de enviar virando o quadrado vermelho durante "Pensando..."; e a timeline
  mostrou `✓ Executou um comando` como uma linha só (sem bolha vazia), expansível clicando nela
  pra ver `run_command({"command":"timeout 20"})` + o resultado completo.

## Testado só por bateria automatizada, ainda não ao vivo no app

Estas mudanças passam nos testes (`cargo test`) mas não foram exercitadas de
ponta a ponta na UI de verdade (via sessão real + modelo), diferente de
outras entradas acima marcadas "testado ao vivo":

- **28 linguagens do `ast_grep`/`ast_edit`** — só as 12 originais (rust,
  typescript, javascript, python, etc.) foram testadas ao vivo; as 16 novas
  (c, cpp, csharp, css, dart, elixir, haskell, hcl, kotlin, nix, php, scala,
  solidity, swift + o resto) só têm cobertura de teste unitário
  (`ast_tools.rs::tests`), não uma chamada de tool real disparada por um
  modelo numa sessão.
- **Paralelismo do `walk_cache`** (`WalkBuilder::build_parallel()`) — testado
  com árvore sintética de 40 arquivos em teste unitário; não testado com um
  projeto real grande de verdade (onde o ganho de paralelismo importa mais).
- **Fallback fuzzy do `edit_file` — níveis 2 e 3** (unicode/aspas
  tipográficas e similaridade Levenshtein) — o nível 1 (espaço/indentação) já
  foi validado ao vivo numa sessão real; os níveis 2 e 3 só têm teste
  unitário/end-to-end sintético, não uma edição real disparada por um modelo
  que efetivamente precisou desses fallbacks pra funcionar.
- **Detecção de encoding via `chardetng`** (Shift-JIS e outras codificações
  além de Windows-1252/UTF-16) — só o caso Windows-1252 foi testado ao vivo
  (seção acima); Shift-JIS/GBK/EUC-KR/etc. só têm teste unitário sintético
  (texto japonês gerado em memória), não um arquivo real desses editado pela
  UI.
- **Guarda de loop** (`is_doom_loop`) — só tem teste unitário puro (lista de chamadas construída
  a mão). Forçar um modelo real a repetir a mesma ferramenta+argumentos 3x seguidas de propósito
  não é algo confiável de reproduzir sob demanda numa sessão ao vivo (depende do modelo realmente
  travar, não dá pra simular clicando na UI) — fica pra quando acontecer organicamente numa
  sessão real, ou se vier a valer a pena escrever um provider de teste que force isso.

## Pesquisa: suporte real a imagem/áudio/vídeo por provider (antes de implementar upload multimodal)

Antes de estender o anexo de arquivos (seção acima) pra imagem/áudio/vídeo, pesquisei o que cada
provider que o Cerne Code já suporta realmente entrega em termos de multimodal — o alerta original era
específico: dá pra configurar um modelo com vision teórico (ex.: gemma3/gemma4) e mesmo assim não
ter vision de verdade, porque a peça que falta não é o modelo, é a configuração do runtime. Achados
por provider, com fonte:

- **Ollama** — o endpoint nativo `POST /api/show` retorna um array `capabilities` (ex.:
  `["completion", "vision"]`) calculado a partir da metadata real do modelo (`vision.block_count`
  no GGUF) — ou seja, dá pra perguntar pro próprio Ollama se um modelo tem vision, sem
  heurística por nome. **Mas** o formato de mensagem da API REST nativa não é o array
  `content`/`image_url` da OpenAI: é um campo `images` separado com base64
  (`{ role, content, images: ["<base64>"] }`) — se o provider Ollama do Cerne Code usa o endpoint
  OpenAI-compatible (`/v1/chat/completions`, que é o que `providers/mod.rs` já usa pros outros
  3 providers), precisa confirmar que esse endpoint aceita `image_url` no formato OpenAI antes de
  reusar o mesmo código de mensagem multimodal dos outros providers — pode exigir um caminho
  separado pro Ollama. ([Vision - Ollama](https://docs.ollama.com/capabilities/vision),
  [PR #10066 - api: return model capabilities from the show endpoint](https://github.com/ollama/ollama/pull/10066))
- **OpenRouter** — `GET /api/v1/models` retorna `architecture.input_modalities` por modelo
  (array tipo `["file", "image", "text"]`), então também dá pra descobrir vision de verdade
  via API, sem lista mantida a mão. Formato de mensagem é o `content` array padrão OpenAI com
  `{"type": "image_url", "image_url": {"url": "..."}}` (URL pública ou base64) — mais simples
  de integrar já que o Cerne Code já fala OpenAI-compatible com esse provider. Limite de imagens por
  request varia por modelo/provider por trás do OpenRouter, não é um número fixo global.
  ([OpenRouter Image Inputs](https://openrouter.ai/docs/guides/overview/multimodal/image-understanding),
  [OpenRouter Models](https://openrouter.ai/docs/guides/overview/models))
- **llama.cpp (forks TurboQuant e PrismML)** — confirmado que os dois forks que o Cerne Code já usa
  mantêm o subsistema multimodal do upstream (`libmtmd`, suporta imagem e áudio), cada um com seu
  próprio `docs/multimodal.md` espelhando o do `ggml-org/llama.cpp`. **Isso não resolve o alerta
  original** — o ponto central continua válido e é estrutural, não uma limitação dos forks:
  - Modelo (`.gguf`) e projetor multimodal (`mmproj-*.gguf`) são **arquivos separados**; o
    `llama-server` só carrega vision se receber `--mmproj arquivo.gguf` explicitamente além do
    `-m modelo.gguf`. O Cerne Code configura presets via `models.ini` por fork (`providers/llama_cpp.rs`,
    `LlamaForkConfig::models_ini`) — pra saber se um preset específico tem vision de verdade,
    precisaria inspecionar se aquele preset no `.ini` referencia um `mmproj`, não inferir isso do
    nome do modelo base. Um preset de "gemma4" no `models.ini` sem linha de `mmproj` não tem vision,
    mesmo o modelo base suportando — exatamente o cenário que foi citado como preocupação.
  - A própria documentação do llama.cpp avisa que o suporte multimodal via HTTP API do
    `llama-server` **"is a work in progress"** e recomenda `llama-mtmd-cli` como caminho mais
    confiável — vale testar contra a versão real instalada antes de confiar no endpoint HTTP.
  - Contenção de VRAM é real: em hardware limitado, decodificação especulativa (MTP/NextN, que o
    TurboQuant já usa) e o projetor multimodal podem não caber juntos na GPU (exemplo encontrado:
    Qwen3.6-27B Q6_K + MTP already uses ~22.6 GiB numa RTX 3090 de 24 GiB, sem sobrar espaço pro
    mmproj de ~1.1 GiB) — outro motivo pra não assumir vision "funciona" só porque o binário
    suporta e o preset aponta pro mmproj certo.
  ([llama.cpp multimodal docs](https://github.com/ggml-org/llama.cpp/blob/master/docs/multimodal.md),
  [How to use --mmproj](https://github.com/ggml-org/llama.cpp/discussions/22190),
  [TurboQuant fork](https://github.com/AtomicBot-ai/atomic-llama-cpp-turboquant),
  [PrismML fork](https://github.com/PrismML-Eng/llama.cpp))
- **LM Studio** — o endpoint OpenAI-compatible (`/v1/models`) **não expõe modalidade** — pra saber
  se um modelo é vision de verdade é preciso consultar o endpoint nativo `/api/v0/models`, que
  retorna `type: "vlm"` pros modelos com vision (achei um bug real e concreto de outro agente de
  código open-source, o `oh-my-pi`, que detectava vision errado justamente por só olhar o endpoint
  OpenAI-compatible — a correção proposta lá foi exatamente cruzar os dois endpoints, o mesmo
  princípio que o Ollama já resolve nativamente). Além disso, há um bug documentado e específico:
  o endpoint `/v1/chat/completions` do LM Studio **rejeita** o formato padrão de data URI base64
  da OpenAI em `image_url` (`data:image/...;base64,...`) com erro "'url' field must be a base64
  encoded image" — dá pra tentar contornar usando o endpoint alternativo `/v1/responses` com
  `"type": "input_image"`, mas isso precisa ser testado contra a versão do LM Studio realmente
  instalada antes de confiar, exatamente como o alerta original pedia.
  ([Image Input - LM Studio](https://lmstudio.ai/docs/typescript/llm-prediction/image-input),
  [OpenAI Compatibility Endpoints](https://lmstudio.ai/docs/developer/openai-compat),
  [API rejects base64 data URI - lmstudio-bug-tracker#1752](https://github.com/lmstudio-ai/lmstudio-bug-tracker/issues/1752),
  [oh-my-pi#2945 - vision model detection](https://github.com/can1357/oh-my-pi/issues/2945))

**Conclusão pra quando for implementar upload de imagem/áudio/vídeo**: não existe um caminho único
de "detectar vision" pros 4 providers — cada um expõe (ou não expõe) essa informação de um jeito
diferente (Ollama e OpenRouter dão pra perguntar via API de verdade; LM Studio precisa cruzar dois
endpoints; llama.cpp precisa inspecionar a config do preset, não o nome do modelo). E mesmo quando
a detecção disser "sim, suporta", ainda vale testar contra a instalação real do usuário antes de
prometer isso na UI — pelo menos LM Studio e llama.cpp têm gotchas documentados (bug de formato de
imagem num, "work in progress" no outro) que não aparecem só olhando a lista de modelos.

## O que falta (próximos passos, fora do escopo desta primeira entrega)

- Modo Manual de execução só cobre o loop principal do turno — tool calls disparadas de dentro
  de um sub-agente (`task`) ou do verificador (`verify_completion`) rodam sem passar pelo gate de
  permissão, mesmo numa sessão em modo Manual.
- MCP: só transporte stdio por enquanto — SSE/HTTP streamable ficam pra depois se algum servidor
  real que valha a pena só suportar esses. `disconnect_all()` não está conectado a nenhum hook de
  shutdown do app ainda (cleanup observado vem do SO/runtime ao encerrar o processo, não de um
  shutdown gracioso explícito).
- Colapsar múltiplas entradas de pending-edit pro mesmo arquivo na UI — hoje, editar o mesmo
  arquivo 2+ vezes antes de aceitar mostra 2+ cartões de diff (um por chamada de `edit_file`);
  como `to_sandbox_path` é determinístico por arquivo, aceitar qualquer um deles já aplica o
  estado cumulativo mais recente (não há mais risco de perda de dado, ver decisão sobre
  `read_current_content` acima), mas os cartões extras ficam meio confusos até serem
  individualmente aceitos/rejeitados. Rough edge cosmético, não bug de correção.
- Ícone do app (`src-tauri/icons/`) — o scaffold trouxe placeholders
  genéricos do Tauri, vale trocar antes de gerar um instalador de verdade.
- `npm run tauri build` pra gerar o instalador (`.msi`/`.exe`) — só testado
  em modo dev até agora.

Ver `C:\Users\ru\.claude\plans\drifting-conjuring-breeze.md` pro plano
completo desta primeira entrega.
