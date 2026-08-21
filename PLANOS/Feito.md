# Feito — Cerne Code

> Tudo que foi implementado E confirmado por você na janela real. Histórico técnico completo
> (decisões, arquivos/funções exatos, bugs reais encontrados e como foram corrigidos) continua em
> `PLANOS/14_backlog_pendente.md` — aqui é só o índice, pra saber rápido o que já está pronto sem
> precisar reler tudo.

---

## Fases do roteiro (Agentes & Skills) — T1 a T48

| # | Item |
|---|------|
| T1 | AskCard com renderização Markdown |
| T2 | Default Manual + Pensamento Desligado |
| T3 | Esconder janela CMD ao executar comandos |
| T4 | Seletor de MCP no Composer |
| T5 | Ajuda atualizada com novas funcionalidades |
| T6 | Nova sessão sem pasta obrigatória |
| T7 | Pastas com modo leitura vs leitura+escrita |
| T8 | Caminho colado no composer vira pasta (pergunta modo) |
| T9 | Auto-nomear sessão via LLM na primeira mensagem |
| T10 | Ícone da sessão muda conforme modo (chat→code) |
| T15 | Remover nudge/TAREFA_CONCLUIDA/Continuando automaticamente |
| T18 | Nova sessão sem modal — criar direto ao clicar + |
| T19 | Sempre caminhos absolutos |
| T20 | Caminho completo nos diffs |
| T21 | Modal de disclaimer na primeira abertura (botão libera em 3s) |
| T22 | Seção Sobre com créditos das ferramentas usadas |
| T23 | Botão de pasta no topo do composer (sempre visível) |
| — | Detecção automática de shell: pwsh7 > powershell5 > cmd (Windows) |
| T24 | Personas: prompts prontos cadastráveis como system prompt selecionável |
| T25 | Fix: passo de tool call ao vivo não atualizava sem recarregar |
| T26 | Fix: `llama-server`/jobs em background ficavam órfãos ao fechar o app |
| T27 | Fase A1: UUID de rastreamento de execução de agente/skill |
| T28 | Fix + Fase A2: texto de sub-agente/verificador vazava no chat principal |
| T30 | Fase A6: descrição de skill cortada + tool `read_skill_details` |
| T31 | Fase A5: modal batelado de aprovação de agentes/skills no modo Manual |
| T32 | Fase A4: `task`s em paralelo via API quando 2+ no mesmo turno |
| T33 | Fase F1: Configurações vira modal |
| T34 | Fase C1: painel de jobs em segundo plano com push em tempo real |
| T35 | Fase C2: painel de execuções de agente/skill somente leitura |
| T36 | Fase B1: painel "Agentes & Skills" |
| T37 | Fase D2: navegador de arquivos no composer |
| T38 | Fase D1: visualizador de diff agregado da sessão + trocar pasta |
| T47 | Fase B2: skills de exemplo semeadas na 1ª execução |
| T48 | Fase A3: Persona ganha allowlist de ferramentas |
| T14 | Background jobs com callback automático |
| T29 | Pastas na lista de sessões (2 níveis, expansível) |
| T16 | Ferramenta `create_pptx` (PowerPoint, com imagens) |
| T17 | IA cria ferramentas Python customizadas (gerenciadas via `uv`) |
| B3 | Fase B3: Persona ganha allowlist de skills |
| E1–E5 | Fase E (Composer): prompt de código, toggle de MCPs num modal só, ícone de visão com cache, atalho `/` (skills/personas/MCPs/prompts prontos), decisão Fable-não-vira-skill |
| — | Fase 3: pipeline determinístico Dev → QA → Analista (`run_pipeline`) |
| — | Fase 4: skills `file-organizer`/`email-triage` + Persona `english-tutor` documentada |
| — | Fase 5: skill `project-manager` (multi-agente gerenciado) |

Detalhe técnico completo de cada linha em `14_backlog_pendente.md`, seção "Implementadas nesta
sessão".

## Busca na web

- Reestruturação de `search_auto`: DuckDuckGo sozinho primeiro (com retry), só escala pra
  Brave+Mojeek em paralelo se vier fraco/vazio/falhar — reduz bloqueio anti-bot no caminho feliz.
- Semáforo limitando buscas concorrentes no app inteiro (protege contra rajada de `task`s/
  sub-agentes simultâneos).
- Novos provedores com chave de API: Serper, Exa, Google Custom Search, Bing/Azure.
- **Timeout de 20s em toda chamada HTTP de busca** (`websearch.rs`) — antes nenhuma tinha timeout
  nenhum, podendo travar um turno pra sempre se o provider não respondesse (bug real: duas sessões
  travaram juntas em "Buscou na web" quando o DuckDuckGo bloqueou por rate-limit).

## Modelo/provider

- Removida a seção confusa "Provider ativo" de Configurações — toda escolha de modelo em qualquer
  sessão agora vira o default global de verdade (`providerStore.setActiveSelection`).
- Contexto lembrado por modelo entre sessões, com bug real corrigido: o valor lembrado não
  disparava pra conexões de API (só local) porque a criação de sessão já preenchia o contexto
  automaticamente antes do hook do frontend rodar — corrigido movendo a prioridade pro backend.
- Bug corrigido: Ctx errado no navegador de modelos (prioridade de cache invertida, chute
  hardcoded vencendo do valor real da OpenRouter).
- Bug de performance corrigido: "criar sessão" ficando lento (refazia fetch do catálogo inteiro da
  OpenRouter, ~414 modelos, toda vez) — agora usa cache em memória, uma leitura/escrita só.
- Selos de capacidade (imagem/tools/áudio) do modelo mostrados no seletor, sempre visíveis.
- Redesign do `ProviderPicker`: removido modo "recolhido"/resumo e o ícone de favoritar (favoritar
  continua só no navegador de modelos de Configurações); dropdown de modelo mais largo.

## MCP

- Modal único "MCPs (N/M)" com checkboxes em vez de um botão por servidor.
- Conectores MCP pré-curados (10 servidores conhecidos, comando+args prontos): Filesystem, GitHub,
  Brave Search, Slack, PostgreSQL, SQLite, Puppeteer, Memory, Google Maps, EverArt — com miniaturas
  reais (Simple Icons, embutidas sem dependência de rede).
- Suporte a servidor MCP remoto (HTTP streamable) + conectores Notion (local, `NOTION_TOKEN`) e
  Linear (remoto de verdade, Personal API Key como bearer token).
- Bug real corrigido: pacote npm do conector Notion estava com o escopo errado
  (`@makenotion/...` → `@notionhq/notion-mcp-server`, o certo).
- Erros de teste de conexão MCP (local e remoto) agora mostram a cadeia de causa completa em vez
  de só a camada mais externa (`describe_error_chain`).
- Espaçamento inconsistente entre ícone/texto nas linhas de MCP corrigido (layout com gap fixo em
  vez de `justify-content: space-between` reagindo ao tamanho da descrição).

## Memória, skills, segurança

- Skills que se auto-melhoram (`improve_skill`).
- Memória entre sessões (`MEMORY.md` global, ferramenta `remember`).
- Checagem de vulnerabilidade via OSV (`osv.rs`).
- Rótulo "Agentes" em vez de "Personas" no painel (Persona já era o "agente nomeado" do roteiro).

## Voz (TTS/STT)

- Ler resposta em voz alta + microfone no composer, via OpenRouter.
- Modelo TTS default corrigido (`gpt-4o-mini-tts` não existe mais na OpenRouter → trocado pro
  Kokoro, mais barato e confirmado existente), com voz configurável e timeout aumentado (cold
  start do provider serverless).
- Detecção automática de idioma pro TTS (heurística leve pt/en/es + Unicode pra zh), trocando a
  voz do Kokoro automaticamente.
- Botão de "ler trecho selecionado" — seleciona texto em qualquer mensagem, aparece um
  autofalante flutuante no final da seleção.
- Configurações → Voz: escolha de provider/modelo/voz pra TTS e STT, cada um independente,
  reaproveitando qualquer conexão já configurada.
- **Integração com Voicebox local** (app open source separado, TTS/STT 100% local via API REST
  própria) — motor alternativo ao OpenAI-compatible, configurável em Configurações → Voz. Bug real
  corrigido: com o "Autoplay on generate" do Voicebox ligado, o Cerne baixava E tocava o mesmo
  áudio de novo (soava duplicado) — corrigido pra só disparar a geração e deixar o Voicebox tocar
  sozinho.
- STT em português via Voicebox: campo de "Idioma da transcrição" configurável (dica ISO 639-1
  pro Whisper, que já é multilíngue nativamente).
- Avisos de HTML no console (`vue-i18n`) silenciados — strings estáticas do próprio app, sem
  risco real de XSS.

## Sidebar / sessões

- Bolinha piscando indicando sessão processando em background, com bug real corrigido: pedido de
  aprovação (modo Manual)/pergunta/plano de agente de uma sessão em background era DESCARTADO se
  o usuário estivesse em outra sessão no momento — a sessão ficava travada pra sempre esperando
  uma resposta que nunca chegaria. Corrigido: viraram mapas por sessão em vez de um valor único
  global.
- Polling desnecessário corrigido: `list_agent_executions`/`list_background_jobs` rodavam a cada
  3-4s pra sempre, mesmo sem nada rodando — agora só ligam quando existe algo `running` de
  verdade.
- Marcador por mensagem do usuário na lateral do chat (pilha compacta, clique rola até a
  mensagem), com 2 bugs reais corrigidos: a faixa cobria a tela inteira em vez de só a área de
  mensagens, e o clique não funcionava (template ref pegando o anchor errado de um componente
  multi-raiz).
- Padding vertical das linhas de sessão/pasta reduzido.

## Redesign do chat/composer (2026-08-20)

- Seletor de modelo + ícone de visão subiram pro topo da área de chat (canto esquerdo); navegar
  arquivos/ver diff foram pro canto direito — saíram de dentro da caixa do composer.
- Menu "+" agrupa: anexar arquivo, Pastas extra, Modo de execução, Raciocínio, Persona, Método
  Fable, MCP.
- Resumo em texto (`Modelo: X, Modo: Y, Raciocínio: Z`) fora da caixa do composer, abaixo dela.
- Indicador de contexto simplificado: só a barra, hover mostra os tokens, clique continua abrindo
  a edição manual.
- Largura útil do chat/composer aumentada (820px → 960px).
- Bug real corrigido: `Teleport` do Vue falhava silenciosamente sem a prop `defer` (alvo e quem
  teleporta montam na mesma passada síncrona) — topo do chat ficava vazio.
