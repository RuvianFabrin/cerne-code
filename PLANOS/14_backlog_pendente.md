# Backlog Pendente — Cerne Code

> Lista de tarefas planejadas mas ainda não implementadas.
> Atualizado em cada sessão de desenvolvimento.

---

## Implementadas nesta sessão (para teste)

| # | Tarefa | Status |
|---|--------|--------|
| T1 | AskCard com renderização Markdown | ✅ APROVADO |
| T2 | Default Manual + Pensamento Desligado | ✅ APROVADO |
| T3 | Esconder janela CMD ao executar comandos | ✅ APROVADO |
| T4 | Seletor de MCP no Composer | ✅ APROVADO |
| T5 | Ajuda atualizada com novas funcionalidades | ✅ APROVADO |
| T6 | Nova sessão sem pasta obrigatória | ✅ APROVADO |
| T7 | Pastas com modo leitura vs leitura+escrita | ✅ APROVADO |
| T8 | Caminho colado no composer vira pasta (pergunta modo) | ✅ APROVADO |
| T9 | Auto-nomear sessão via LLM na primeira mensagem | ✅ APROVADO |
| T10 | Ícone da sessão muda conforme modo (chat→code) | ✅ APROVADO |
| T15 | Remover nudge/TAREFA_CONCLUIDA/Continuando automaticamente | ✅ APROVADO |
| T18 | Nova sessão sem modal — criar direto ao clicar + | ✅ APROVADO |
| T19 | Sempre caminhos absolutos | ✅ APROVADO |
| T20 | Caminho completo nos diffs | ✅ APROVADO |
| T21 | Modal de disclaimer na primeira abertura (botão libera em 3s) | ✅ APROVADO |
| T22 | Seção Sobre com créditos das ferramentas usadas | ✅ APROVADO |
| T23 | Botão de pasta 📁 no topo do composer (sempre visível) | ✅ APROVADO (corrigido BUG: pasta extra sem project_root agora permite edição) |
| Shell | Detecção automática: pwsh7 > powershell5 > cmd (Windows) | ✅ APROVADO |

---

## Pendentes (planejadas, não implementadas)

### Prioridade Alta

| # | Tarefa | Descrição | Complexidade |
|---|--------|-----------|-------------|
| T16 | Ferramenta `create_powerpoint` | Criar/editar arquivos PPTX. Copiar lógica do python-pptx para Rust (ou usar crate `pptx`). Incluir: criar slides, adicionar texto/imagens/tabelas, formatar. Similar ao `create_excel`/`create_word`/`create_pdf` que já existem. | Média |
| T17 | IA cria ferramentas Python customizadas (UV manager) | O LLM pode criar scripts Python como "ferramentas" que ficam disponíveis em sessões futuras. Fluxo: LLM gera script → salva em pasta de tools do Cerne → gera instruções de uso (SKILL.md automático) → inclui nas instruções de novas sessões. Usar UV para gerenciar dependências Python. O usuário vê as ferramentas criadas na UI e pode editar/remover. | Alta |

### Prioridade Média

| # | Tarefa | Descrição | Complexidade |
|---|--------|-----------|-------------|
| T11 | Visualização de agentes/ferramentas no chat | Quando o agente usa sub-agentes (`task`) ou verificador (`verify_completion`), mostrar no chat de forma visual (card expansível). Ao clicar, abre modal com detalhes completos: quais ferramentas foram usadas, output de cada uma, tempo gasto, resultado final. | Alta |
| T12 | Painel lateral flutuante de arquivos | Botão no canto superior direito abre painel flutuante com: dropdown para escolher pastas da sessão, árvore de arquivos expandível, botão "+" para enviar conteúdo de um arquivo pro chat (como anexo textual), botão "x" para fechar o painel. | Alta |
| T14 | Background jobs com callback automático | Quando um processo em background termina, o sistema injeta o resultado automaticamente no próximo turno do LLM, eliminando polling (`check_background_output` repetido) e economizando ~1000-1500 tokens por build longo. Implementação: ao encerrar, emitir evento `agent:background_done` e injetar mensagem no histórico. | Média |

### Prioridade Baixa

| # | Tarefa | Descrição | Complexidade |
|---|--------|-----------|-------------|
| T13 | Agentes e Skills podem usar MCPs do usuário | Sub-agentes (`task`) e verificador (`verify_completion`) atualmente não têm acesso aos MCP servers configurados. Adicionar os MCPs ao toolset deles (respeitando o filtro `enabled_mcp_servers` da sessão). | Baixa |

---

## Fases futuras (do roteiro `13_roteiro_agentes_skills_fases.md`)

| Fase | Nome | Status |
|------|------|--------|
| Fase 3 | Pipeline Dev → QA → Analista (orquestração determinística) | ⏳ Planejado |
| Fase 4 | Skills de produtividade (file_organizer, email_triage, english_tutor) | ⏳ Planejado |
| Fase 5 | Multi-agente gerenciado (gerente dinâmico) | ⏳ Planejado |
| Fase 6 | Skill Store / Comunidade | ⏳ Planejado |

---

## Repositórios clonados para referência

Todos em `F:\AgentesESkills\` (pode excluir quando quiser):

| Repo | O que tem de útil |
|------|------------------|
| MetaGPT | Prompts de PM/Architect/Engineer/QA, pipeline pub/sub |
| CrewAI + examples | Formato YAML declarativo de agentes, output estruturado Pydantic |
| AutoGen | Termination conditions (token budget, timeout), GroupChat patterns |
| PR-Agent | ⭐ Prompt de code review completo, schema YAML, ticket compliance |
| Open Interpreter | AGENTS.md real, regras de contexto (10K cap) |
| Composio | 17 skills SKILL.md reais, formato validado |
| awesome-ai-agents | Lista curada de 300+ recursos |

---

## Análise detalhada dos repos

Ver `PLANOS/13_roteiro_agentes_skills_fases.md` → seção "Análise Detalhada dos Repositórios"
