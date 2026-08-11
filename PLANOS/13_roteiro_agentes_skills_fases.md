# Roteiro de Agentes, Skills e Ferramentas — Fases e Tarefas

> Documento de referência para implementar agentes e skills no Cerne Code de forma progressiva.
> Cada fase tem tarefas concretas, perguntas que podem ser feitas ao usuário via `ask` no composer,
> e repositórios GitHub prontos para estudar/copiar estratégias.
>
> **Análise de código-fonte real** dos repositórios clonados em `F:\AgentesESkills\` está na seção
> "Análise Detalhada dos Repositórios" ao final deste documento.

---

## Análise Detalhada dos Repositórios (Código Real)

> Esta seção contém achados diretos da leitura do código-fonte de cada repositório.
> Trechos literais são marcados com ``` e caminhos de arquivo são citados.

### 1. MetaGPT (`F:\AgentesESkills\MetaGPT`)

#### Roles disponíveis
| Role | Arquivo | Goal | Tools |
|------|---------|------|-------|
| ProductManager | `metagpt/roles/product_manager.py` | Criar PRD ou pesquisa de mercado | Browser, Editor, SearchEnhancedQA |
| Architect | `metagpt/roles/architect.py` | Design de sistema e APIs | Editor |
| Engineer | `metagpt/roles/engineer.py` | Escrever e revisar código | WriteCode, WriteCodeReview, WriteTasks |
| QaEngineer | `metagpt/roles/qa_engineer.py` | Escrever testes robustos | WriteTest, RunCode, DebugError |
| ProjectManager | `metagpt/roles/project_manager.py` | Gerenciar tarefas e timeline | - |
| Researcher | `metagpt/roles/researcher.py` | Pesquisa aprofundada | SearchEnhancedQA |
| Teacher | `metagpt/roles/teacher.py` | Ensino/tutoria | - |
| Sales | `metagpt/roles/sales.py` | Vendas e prospecção | - |

#### ⭐ Prompt de Product Manager (LITERAL — `metagpt/prompts/product_manager.py`)
```
You are a product manager AI assistant specializing in product requirement
documentation and market research analysis. Your work focuses on the analysis
of problems and data. You should always output a document.

## Mode 1: PRD Creation
Required Fields:
1. Language & Project Info (match user's language, snake_case project name)
2. Product Definition (IMPORTANT):
   - Product Goals: 3 clear, orthogonal goals
   - User Stories: 3-5 scenarios "As a [role], I want [feature] so that [benefit]"
   - Competitive Analysis: 5-7 products with pros/cons
   - Competitive Quadrant Chart (Required, Mermaid syntax)
3. Technical Specifications:
   - Requirements Analysis
   - Requirements Pool with P0/P1/P2 priorities
   - UI Design Draft
   - Open Questions

PRD Guidelines:
- Use Must/Should/May language
- Include measurable criteria
- Prioritize: P0 Must-have, P1 Should-have, P2 Nice-to-have
```

**O que aproveitar**: A estrutura de PRD com prioridades P0/P1/P2 e user stories é diretamente útil como skill `business_analyst` no Cerne. O formato Mermaid para quadrant chart é um diferencial.

#### ⭐ Prompt de Code Review (LITERAL — `metagpt/actions/write_code_review.py`)
```
Role: You are a professional software engineer, and your main task is to review
and revise the code. Ensure code conforms to google-style standards, is elegantly
designed and modularized, easy to read and maintain.

## Code Review: Ordered List
1. Is the code implemented as per the requirements? If not, how to achieve it?
2. Is the code logic completely correct? If there are errors, indicate how to fix.
3. Does the existing code follow the "Data structures and interfaces"?
4. Are all functions implemented?
5. Have all necessary pre-dependencies been imported?
6. Are methods from other files being reused correctly?

## Code Review Result: LGTM or LBTM
(If no bugs, answer LGTM and stop. ONLY ANSWER LGTM/LBTM.)
```

**O que aproveitar**: O checklist de 6 pontos é conciso e testado. O veredito binário LGTM/LBTM é similar ao APROVADO/REFUTADO do Cerne — pode ser adotado como alternativa mais curta.

#### ⭐ Prompt de QA/Test Writer (LITERAL — `metagpt/actions/write_test.py`)
```
Role: You are a QA engineer; design, develop and execute PEP8 compliant,
well-structured, maintainable test cases. Focus on ensuring product quality
through systematic testing.

Attention:
1. Use '##' to split sections, not '#'
2. ALWAYS SET A DEFAULT VALUE, ALWAYS USE STRONG TYPE AND EXPLICIT VARIABLE
3. YOU MUST FOLLOW "Data structures and interfaces". DO NOT CHANGE ANY DESIGN.
4. Think before writing: What should be tested? What edge cases could exist?
5. CAREFULLY CHECK THAT YOU DON'T MISS ANY NECESSARY TEST CASES.
```

**O que aproveitar**: As regras de "think before writing" e "don't miss test cases" são prompts engineering testados. A restrição "DO NOT CHANGE ANY DESIGN" é crucial para QA — o testador não deve refatorar.

#### Pipeline/Orquestração
- **Padrão**: `_watch()` + `publish_message()` = pub/sub entre roles
- **ReactMode**: `BY_ORDER` (sequencial fixo) vs reactivo
- **Handoff**: Via `Message` com `cause_by` (qual action gerou) e `send_to` (destinatário)
- **QaEngineer**: Tem `test_round_allowed: int = 5` — limite de rounds de teste/debug

**O que aproveitar**: O padrão de `test_round_allowed` é exatamente o "limite de rounds" proposto na Fase 3. O `_watch` + `publish_message` é pub/sub — diferente do Cerne que usa chamadas diretas, mas o conceito de "quem observa quem" vale como design doc.

#### ❌ O que é repetido/não vale copiar
- A infraestrutura de `Role`, `Action`, `Message` é Python/pydantic — não portável para Rust
- O sistema de `Environment` e `Team` é over-engineered para o caso do Cerne
- Memory/RAG/DocumentStore são módulos pesados que o Cerne não precisa agora

---

### 2. CrewAI + crewAI-examples (`F:\AgentesESkills\crewAI`, `F:\AgentesESkills\crewAI-examples`)

#### Estrutura de Agent/Task/Crew
```python
# agents.yaml (config declarativa!)
lead_market_analyst:
  role: Lead Market Analyst
  goal: Conduct amazing analysis of products and competitors
  backstory: As the Lead Market Analyst at a premier digital marketing firm...

chief_marketing_strategist:
  role: Chief Marketing Strategist
  goal: Synthesize insights to formulate marketing strategies
  backstory: Known for crafting bespoke strategies that drive success

creative_content_creator:
  role: Creative Content Creator
  goal: Develop compelling content for social media campaigns
  backstory: Excel in crafting narratives that resonate with audiences

chief_creative_director:
  role: Chief Creative Director
  goal: Oversee work to ensure best possible, aligned with product goals
  backstory: You ensure your team crafts the best possible content
```

**O que aproveitar**: O formato YAML declarativo para definir agentes é excelente. Pode ser adaptado como formato alternativo ao SKILL.md do Cerne — um `AGENT.yaml` com `role`, `goal`, `backstory`, `tools`. O campo `backstory` é um diferencial do CrewAI que dá personalidade ao agente.

#### Output estruturado com Pydantic
```python
class MarketStrategy(BaseModel):
    name: str = Field(..., description="Name of the market strategy")
    tatics: List[str] = Field(..., description="List of tactics")
    channels: List[str] = Field(..., description="List of channels")
    KPIs: List[str] = Field(..., description="List of KPIs")

# Na task:
output_json=MarketStrategy  # Força saída estruturada!
```

**O que aproveitar**: O padrão `output_json=PydanticModel` é a forma mais elegante de forçar saída estruturada. No Cerne, isso equivale a usar `response_format` com JSON schema na API call.

#### Processos de orquestração
- `Process.sequential` — tasks executam em ordem
- `Process.hierarchical` — manager agent delega (comentado no código, disponível)
- `context=[task1, task2]` — tasks recebem output de tasks anteriores como contexto

**O que aproveitar**: O `context` entre tasks é o handoff estruturado que propomos na Fase 3. Simples e eficaz.

#### Exemplos disponíveis em `crews/`
| Exemplo | Relevância pro Cerne |
|---------|---------------------|
| starter_template | ✅ Template base para criar crews |
| marketing_strategy | ✅ Analista + Estrategista + Criativo (modelo de negócio) |
| recruitment | ✅ Match de perfil a vagas (análise estruturada) |
| stock_analysis | ⚠️ Financeiro, menos relevante |
| trip_planner | ⚠️ Planejamento de viagem, pouco reuse |
| game-builder-crew | ⚠️ Nicho específico |
| screenplay_writer | ⚠️ Criativo, pouco reuse |
| markdown_validator | ✅ Validação de documentos (similar a QA) |

#### ❌ O que é repetido/não vale copiar
- O framework CrewAI inteiro é Python — não portável
- A integração com LangChain/LangGraph é dependência pesada
- Memory/persistência é genérica

---

### 3. AutoGen (`F:\AgentesESkills\autogen`)

#### ⭐ Termination Conditions (LITERAL — `conditions/_terminations.py`)
AutoGen tem **6 condições de terminação** combináveis:

| Condição | Como funciona | Relevância pro Cerne |
|----------|--------------|---------------------|
| `StopMessageTermination` | Para quando recebe StopMessage | ✅ Equivalente ao TAREFA_CONCLUIDA |
| `MaxMessageTermination` | Para após N mensagens | ✅ Equivalente ao MAX_AGENTIC_STEPS |
| `TextMentionTermination` | Para quando texto específico aparece | ✅ Equivalente aos loop breakers |
| `TokenUsageTermination` | Para quando tokens excedem limite | ⭐ NOVO — não temos no Cerne! |
| `FunctionalTermination` | Para baseado em função customizada | ✅ Extensível |
| `TimeoutTermination` | Para após tempo máximo | ⭐ NOVO — safety net útil |

**O que aproveitar**: `TokenUsageTermination` e `TimeoutTermination` são duas condições que o Cerne NÃO tem e seriam valiosas como safety nets adicionais no loop do agente. Implementar como opções no `LoopConfig` proposto no `agent_loop_propostas.md`.

#### Padrões de GroupChat
- `RoundRobinGroupChat` — todos falam em ordem
- `SelectorGroupChat` — modelo escolhe quem fala
- `MagenticOneGroupChat` — orquestrador central decide

**O que aproveitar**: O `SelectorGroupChat` (modelo escolhe próximo falante) é o padrão mais próximo do "gerente dinâmico" da Fase 5. Mas requer modelo forte — não funciona bem com modelos locais pequenos.

#### ❌ O que é repetido/não vale copiar
- A infraestrutura de `Agent`, `Team`, `Message` é Python/async — não portável
- Integração com Azure/Docker é específica do ecossistema Microsoft
- AutoGen Studio é UI separada, não aplicável

---

### 4. PR-Agent (`F:\AgentesESkills\pr-agent`)

#### ⭐⭐ PROMPT DE CODE REVIEW COMPLETO (OURO PURO)
Arquivo: `pr_agent/settings/pr_reviewer_prompts.toml`

Este é o prompt de code review mais completo e testado em produção que encontrei.
Trechos-chave literais:

```
System: You are PR-Reviewer, a language model designed to review a Git Pull Request.
Your task is to provide constructive and concise feedback for the PR.
The review should focus on new code added in the PR code diff (lines starting with '+'),
and only on issues introduced by this PR.

Determining what to flag:
- For clear bugs and security issues, be thorough. Do not skip a genuine problem
  just because the trigger scenario is narrow.
- For lower-severity concerns, be certain before flagging. If you cannot confidently
  explain why something is a problem with a concrete scenario, do not flag it.
- Each issue must be discrete and actionable, not a vague concern.
- Do not speculate that a change might break other code unless you can identify
  the specific affected code path from the diff context.
- Do not flag intentional design choices or stylistic preferences unless they
  introduce a clear defect.
- When confidence is limited but potential impact is high (data loss, security),
  report it with explicit note on what remains uncertain.

Constructing comments:
- Be direct about why something is a problem and the realistic scenario.
- Communicate severity accurately. Do not overstate impact.
- Keep each issue description concise.
- Use matter-of-fact, helpful tone. Avoid accusatory language, excessive praise,
  or filler phrases like 'Great job', 'Thanks for'.
```

#### ⭐ Saída estruturada YAML (Pydantic schema)
```yaml
review:
  estimated_effort_to_review_[1-5]: 3
  score: 89
  relevant_tests: "No"
  key_issues_to_review:
    - relevant_file: directory/xxx.py
      issue_header: Possible Bug
      issue_content: ...
      start_line: 12
      end_line: 14
  security_concerns: "No"
  todo_sections: "No"
  can_be_split:
    - relevant_files: [...]
      title: ...
  ticket_compliance_check:
    - ticket_url: ...
      ticket_requirements: ...
      fully_compliant_requirements: ...
      not_compliant_requirements: ...
      requires_further_human_verification: ...
```

**O que aproveitar**: Este schema YAML é o MELHOR template encontrado para code review estruturado. Campos especialmente valiosos:
- `key_issues_to_review` com `start_line`/`end_line` → permite linkar direto no código
- `ticket_compliance_check` → verifica se PR atende requisitos do ticket (Fase 3!)
- `can_be_split` → sugere dividir PR grande em menores
- `estimated_effort_to_review_[1-5]` → métrica de complexidade
- `security_concerns` separado de bugs gerais

#### ⭐ Ticket Compliance Check (`tools/ticket_pr_compliance_check.py`)
Extrai tickets de:
- PR description (GitHub issues, JIRA)
- Branch name (padrão `feature/123-fix-bug`)
- Limita a 3 tickets por review

Regex patterns:
```python
GITHUB_TICKET_PATTERN = r'(https://github[^/]+/[^/]+/[^/]+/issues/\d+)|(\b(\w+)/(\w+)#(\d+)\b)|(#\d+)'
JIRA_PATTERN = r'\b[A-Z]{2,10}-\d{1,7}\b'
BRANCH_ISSUE_PATTERN = r"(?:^|/)(\d{1,6})(?=-|$)"
```

**O que aproveitar**: A extração automática de ticket/issue do branch name é genial e simples. Pode ser implementada como utility function no Cerne para alimentar o analista de requisitos na Fase 3.

#### ❌ O que é repetido/não vale copiar
- Git providers (GitHub/GitLab/Azure) — o Cerne opera localmente
- Infraestrutura de CLI/Docker/GitHub Action
- Identity/secret providers

---

### 5. Open Interpreter (`F:\AgentesESkills\open-interpreter`)

#### AGENTS.md (322 linhas de instruções para coding agent)
Arquivo: `AGENTS.md`

Este é um AGENTS.md real de produção, escrito em Rust/codex-rs. Contém regras valiosas:

```markdown
## Code Review Rules

### Model visible context
1. No history rewrite - context must be built up incrementally
2. Avoid frequent changes to context that cause cache misses
3. No unbounded items - everything injected must have bounded size and hard cap
4. No items larger than 10K tokens
5. Highlight new individual items >1k tokens as P0
6. All injected fragments must be defined as structs implementing ContextualUserFragment trait
```

**O que aproveitar**: As regras de contexto são diretamente aplicáveis ao Cerne:
- Limite de 10K tokens por item injetado
- Tudo deve ter tamanho bounded
- Items >1K tokens precisam de review manual
- Contexto incremental (não reescrita)

#### Formato de Skills (`.agents/skills/`)
Não encontrado neste repo (é codex-rs, não open-interpreter clássico). Mas o Composio usa o mesmo formato.

#### ❌ O que é repetido/não vale copiar
- Bazel build system — específico do projeto
- Sandbox/Seatbelt — macOS específico
- TUI code — interface terminal, não aplicável ao Cerne GUI

---

### 6. Composio (`F:\AgentesESkills\composio`)

#### Formato de Skills (`.agents/skills/`)
Encontradas 17 skills reais no formato SKILL.md:

| Skill | Descrição |
|-------|-----------|
| bug-fixing | Fix defects with reproduction, root-cause, regression tests |
| cli-command | Create/manage CLI commands |
| cli-e2e | End-to-end CLI testing |
| cli-release | Release process for CLI |
| cross-sdk-parity | Ensure parity between Python/TS SDKs |
| docs-decisions | Documentation architecture decisions |
| eve | AI assistant persona |
| good-docs-audit | Audit documentation quality |
| good-docs-writing | Write good documentation |
| python-providers | Python provider implementation |
| python-release | Python release process |
| python-sdk | Python SDK development |
| python-testing | Python testing patterns |
| repo-guidance | Repository navigation guidance |
| skill-maintenance | Maintain and update skills |
| typescript-providers | TypeScript provider implementation |
| typescript-sdk | TypeScript SDK development |

#### Exemplo de SKILL.md (LITERAL — `.agents/skills/bug-fixing/SKILL.md`)
```yaml
---
name: bug-fixing
description: Fix defects in the Composio SDK repository with focused
  reproduction, root-cause analysis, regression tests, and narrow verification.
  Use when the user reports a bug, failing test, CI regression, runtime defect,
  or incorrect SDK behavior. Do not use for new feature design or broad refactors.
---

# Bug Fixing

Use this skill for defect work.

Read `references/regression-testing.md` before editing code so the fix includes
the right reproduction and tests.
```

**O que aproveitar**: 
1. O formato é IDÊNTICO ao SKILL.md do Cerne — confirma que estamos no caminho certo
2. A `description` é exemplar: diz QUANDO usar E QUANDO NÃO usar ("Do not use for...")
3. Referência a `references/` sub-pasta — skills podem ter material de apoio
4. Skills de `good-docs-audit` e `good-docs-writing` são templates prontos para adaptar

#### ❌ O que é repetido/não vale copiar
- O Composio como plataforma de integrações é SaaS — não aplicável localmente
- Os conectores específicos (Gmail, Slack etc.) são melhor obtidos via MCP servers

---

### 7. awesome-ai-agents (`F:\AgentesESkills\awesome-ai-agents`)

Lista curada com 5591 linhas e centenas de projetos categorizados.
Categorias relevantes encontradas:
- General purpose / Build your own / Multi-agent
- Coding assistants
- Data analysis
- Customer support
- Research

**O que aproveitar**: Usar como índice de referência quando precisar de um agente específico. Não contém código reutilizável diretamente.

---

## Mapa de Reuso: O Que Copiar de Onde

| Necessidade no Cerne | Melhor fonte | Arquivo/caminho | Prioridade |
|---------------------|-------------|-----------------|-----------|
| Prompt de Code Review | PR-Agent | `pr_agent/settings/pr_reviewer_prompts.toml` | 🔴 P0 |
| Schema de review estruturado | PR-Agent | Mesmo arquivo (YAML schema) | 🔴 P0 |
| Ticket compliance check | PR-Agent | `pr_agent/tools/ticket_pr_compliance_check.py` | 🟡 P1 |
| Prompt de QA/Test Writer | MetaGPT | `metagpt/actions/write_test.py` | 🔴 P0 |
| Prompt de Business Analyst/PRD | MetaGPT | `metagpt/prompts/product_manager.py` | 🟡 P1 |
| Formato declarativo de Agent | CrewAI | `crewAI-examples/.../config/agents.yaml` | 🟡 P1 |
| Output estruturado Pydantic | CrewAI | `crewAI-examples/.../crew.py` | 🟢 P2 |
| Termination conditions | AutoGen | `autogen-agentchat/.../conditions/_terminations.py` | 🟡 P1 |
| Token budget termination | AutoGen | Mesmo arquivo (`TokenUsageTermination`) | 🟡 P1 |
| Timeout termination | AutoGen | Mesmo arquivo (`TimeoutTermination`) | 🟢 P2 |
| Regras de contexto (10K cap) | Open Interpreter | `AGENTS.md` | 🟡 P1 |
| Formato SKILL.md validado | Composio | `.agents/skills/*/SKILL.md` | ✅ Já usamos |
| Description com "when NOT to use" | Composio | `.agents/skills/bug-fixing/SKILL.md` | 🔴 P0 |
| Lista de agentes open-source | awesome-ai-agents | `README.md` | 🟢 Referência |

---

## O Que É Repetido Entre Repos (Não Reinventar)

| Padrão | Presente em | Conclusão |
|--------|------------|-----------|
| System prompt com role/goal/constraints | MetaGPT, CrewAI, PR-Agent | ✅ Universal — adotar como padrão |
| Saída estruturada (YAML/JSON/Pydantic) | PR-Agent, CrewAI, MetaGPT | ✅ Preferir YAML (mais legível) ou JSON schema |
| Limite de steps/mensagens | AutoGen, MetaGPT, Cerne | ✅ Já temos — adicionar token/timeout |
| Handoff entre agentes via mensagem | MetaGPT, CrewAI, AutoGen | ⚠️ Cada um faz diferente — Cerne usa chamada direta (mais simples) |
| Guarda de profundidade (no recursão) | Cerne, MetaGPT (implícito) | ✅ Cerne já implementou explicitamente |
| Pub/sub entre roles | MetaGPT, AutoGen | ❌ Over-engineered pro Cerne — manter chamadas diretas |
| Memory/RAG persistente | MetaGPT, CrewAI, AutoGen | ❌ Prematuro — filesystem é memória suficiente por agora |
| Docker/sandbox isolation | AutoGen, Open Interpreter | ⚠️ Cerne usa sandbox de edição — suficiente por agora |

---

---

## Visão Geral das Fases

| Fase | Nome | Objetivo | Complexidade |
|------|------|----------|-------------|
| 1 | Skills Básicas | Usuário cria e usa skills simples | Baixa |
| 2 | Prompts Prontos como "Agentes Leves" | Cards no composer que ativam comportamentos especializados | Baixa |
| 3 | Pipeline Dev → QA → Analista | Orquestração determinística com sub-agentes | Média |
| 4 | Skills de Produtividade | Organizar pastas, verificar spam, tutor de inglês | Média |
| 5 | Multi-Agente Gerenciado | Gerente de negócios coordena equipe de agentes | Alta |
| 6 | Skill Store / Comunidade | Usuários compartilham e instalam skills de terceiros | Alta |

---

## Fase 1: Skills Básicas ✅ (Já Implementado)

### O que já existe no Cerne
- Editor de skills em Configurações → Skills (`SkillEditorModal.vue`)
- Escopos global (`{app_data}/skills/`) e por projeto (`<projeto>/.cerne/skills/`)
- Catálogo listado no início da sessão, corpo carregado sob demanda via `load_skill`
- Comandos Tauri: `create_skill`, `read_skill`, `save_skill`, `list_skills`, `open_skills_folder`

### Tarefas para consolidar
- [ ] Documentar o formato `SKILL.md` na ajuda do app (`help.pt-BR.md` etc.)
- [ ] Adicionar 3-5 skills de exemplo embarcadas no instalador
- [ ] Permitir importar skill de URL ou arquivo local

### Perguntas ao usuário (via `ask` no composer)
> Nenhuma necessária nesta fase — a criação é self-service pela UI.

### Repositórios para estudar skills prontas
| Repo | O que copiar | Link |
|------|-------------|------|
| OpenInterpreter `.agents/skills/` | Formato de skills compatível com AGENTS.md | https://github.com/openinterpreter/openinterpreter |
| Claude Code SKILL.md spec | Formato frontmatter name/description + corpo Markdown | (documentação oficial Anthropic) |
| awesome-ai-agents | Lista curada com 300+ recursos de agentes e skills | https://github.com/e2b-dev/awesome-ai-agents |
| kyrolabs/awesome-agents | Lista de ferramentas e frameworks open-source | https://github.com/kyrolabs/awesome-agents |

---

## Fase 2: Prompts Prontos como "Agentes Leves"

### Conceito
Os `READY_PROMPTS` (`src/content/prompts.ts`) já são cards clicáveis no composer.
Expandir isso para "modos" que simulam papéis sem criar agentes separados.

### Tarefas
- [ ] Criar prompts prontos para cada papel:
  - 🔍 **Code Reviewer** — analisa diff/arquivo, retorna bugs + sugestões
  - 🧪 **QA Tester** — gera casos de teste a partir de requisito/código
  - 📋 **Analista de Negócios** — transforma pedido bruto em requisito estruturado
  - 🇬🇧 **English Tutor** — corrige gramática, sugere vocabulário, simula conversação
  - 📁 **File Organizer** — planeja organização de pasta com preview
- [ ] Cada prompt pronto ativa um system prompt temporário + filtra ferramentas
- [ ] Adicionar campo `system_prompt_override` opcional em `ReadyPrompt`

### Perguntas ao usuário (via `ask`)
```
Pergunta: "Qual modo você quer usar agora?"
Opções:
  - 🔍 Revisar código
  - 🧪 Gerar testes
  - 📋 Analisar requisito
  - 🇬🇧 Praticar inglês
  - 📁 Organizar arquivos
  - 💬 Conversa normal
```

### Repositórios para estudar prompts de papéis
| Repo | O que copiar | Link |
|------|-------------|------|
| MetaGPT roles/ | Prompts de Product Manager, Architect, Engineer, QA | https://github.com/FoundationAgents/MetaGPT |
| CrewAI examples | Templates de Business Analyst, Researcher, Writer | https://github.com/crewAIInc/crewAI-examples |
| PR-Agent review prompts | Prompts de code review testados em produção | https://github.com/qodo-ai/pr-agent |

---

## Fase 3: Pipeline Dev → QA → Analista (Orquestração Determinística)

### Conceito
Workflow fixo em código Rust. O LLM nunca decide o fluxo — só responde dentro da etapa.
Baseado no padrão do `verifier.rs` que já existe no Cerne.

### Arquitetura
```
Usuário envia requisito
        │
        ▼
┌─────────────────┐
│  Agente DEV      │ ← tools: ler, editar, comandos
│  Implementa      │
└────────┬────────┘
         │ resumo + arquivos alterados
         ▼
┌─────────────────┐
│  Agente QA       │ ← tools: só ler + run_command (testes)
│  Testa           │
└────────┬────────┘
         │ relatório de testes
         ▼
┌─────────────────┐
│  Agente ANALISTA │ ← tools: só ler
│  Verifica reqs   │
└────────┬────────┘
         │
    ┌────┴────┐
    │Aprovado?│
    ├─SIM────► Entrega ao usuário
    └─NÃO────► Volta pro DEV com pendências (máx. N rounds)
```

### Tarefas
- [ ] Criar `pipeline.rs` com máquina de estados dev→qa→analista
- [ ] Definir structs de handoff entre etapas (resumo estruturado, não texto livre)
- [ ] Usar saída JSON ou tool call forçado para vereditos (APROVADO/REFUTADO)
- [ ] Limite de rounds configurável (default: 3)
- [ ] Eventos de UI: `"🛠️ Dev implementando..."`, `"🧪 QA testando..."`, `"📋 Analista conferindo..."`
- [ ] Integrar como novo `ReadyPrompt` ou comando `/pipeline`

### Perguntas ao usuário (via `ask`)
```
Pergunta: "Antes de iniciar o pipeline, confirme o escopo:"
Opções:
  - ✅ Sim, pode começar
  - ✏️ Quero ajustar o requisito primeiro
  - ⚙️ Configurar limite de rounds / ferramentas

Pergunta (se analista rejeitar):
"O analista encontrou pendências. Como proceder?"
Opções:
  - 🔄 Dev corrige automaticamente (round X/3)
  - 👀 Quero ver os detalhes antes
  - ❌ Parar pipeline
```

### Repositórios para estudar orquestração
| Repo | O que copiar | Link |
|------|-------------|------|
| MetaGPT | Pipeline PM→Architect→Engineer→QA com handoffs estruturados | https://github.com/FoundationAgents/MetaGPT |
| ChatDev | Empresa virtual CEO→CTO→Programmer→Reviewer→Tester | https://github.com/OpenBMB/ChatDev |
| AutoGen | Multi-agent conversation framework da Microsoft | https://github.com/microsoft/autogen |
| CrewAI | Framework leve de crews com roles/tasks/tools | https://github.com/crewAIInc/crewAI |
| verifier.rs (Cerne) | Padrão interno de verificador adversarial já implementado | `src-tauri/src/agent/verifier.rs` |

---

## Fase 4: Skills de Produtividade

### 4a. Organizar Pastas (`file_organizer`)
- Classifica arquivos por extensão/data/tamanho
- Gera preview antes de mover
- Permite desfazer
- **Segurança**: nunca apaga, só move; confirmação obrigatória

### 4b. Verificar Spam / E-mail (`email_triage`)
- Conecta via MCP server (Gmail/Outlook/IMAP)
- Classifica: spam / importante / promocional / financeiro
- Modo somente leitura primeiro; ações só após confirmação
- **Segurança**: nunca apaga automaticamente

### 4c. Tutor de Inglês (`english_tutor`)
- Modos: conversação, correção gramatical, vocabulário para trabalho, simulação de entrevista
- Corrige e explica erro, sugere alternativa
- Adapta ao nível do usuário (A1-C2)

### Tarefas
- [ ] Criar skill `file_organizer` com prompt + handler
- [ ] Criar skill `email_triage` com prompt + integração MCP
- [ ] Criar skill `english_tutor` com prompt adaptativo
- [ ] Adicionar como prompts prontos no composer

### Perguntas ao usuário (via `ask`)
```
# File Organizer
Pergunta: "Encontrei 87 arquivos na pasta Downloads. Vou organizar em:
Documentos, Imagens, Vídeos, Instaladores, Outros. Continuar?"
Opções: [✅ Sim] [✏️ Ajustar categorias] [❌ Cancelar]

# Email Triage
Pergunta: "Classifiquei 23 e-mails. 5 parecem spam, 3 urgentes, 15 newsletters.
Quer ver o resumo antes de agir?"
Opções: [👀 Ver resumo] [🗑️ Arquivar spam] [⭐ Marcar urgentes]

# English Tutor
Pergunta: "Qual seu nível de inglês?"
Opções: [A1-A2 Básico] [B1-B2 Intermediário] [C1-C2 Avançado]

Pergunta: "O que quer praticar?"
Opções: [💬 Conversação] [✍️ Gramática] [💼 Vocabulário de trabalho] [🎤 Simulação de entrevista]
```

### Repositórios para estudar
| Repo | O que copiar | Link |
|------|-------------|------|
| Open Interpreter | Execução local segura de scripts de organização | https://github.com/openinterpreter/openinterpreter |
| Composio | Conectores prontos para Gmail, Slack, Notion, etc. | https://github.com/ComposioHQ/composio |
| LangChain Tools | Dezenas de tools prontas (filesystem, email, DB) | https://github.com/langchain-ai/langchain |

---

## Fase 5: Multi-Agente Gerenciado

### Conceito
Um agente "Gerente de Negócios" que:
1. Recebe o objetivo do usuário
2. Planeja quais skills/agentes acionar
3. Delega e coleta resultados
4. Decide se precisa de mais iterações
5. Entrega resultado final

### Diferença da Fase 3
- Fase 3 = pipeline **fixo** (dev→qa→analista)
- Fase 5 = gerente **dinâmico** (decide quem chamar com base no pedido)

### Tarefas
- [ ] Criar agente "gerente" com system prompt de planejamento
- [ ] Dar ao gerente acesso à lista de skills disponíveis (catálogo)
- [ ] Gerente chama `task` para delegar a agentes especializados
- [ ] Gerente sintetiza resultados e decide próximos passos
- [ ] Limite de delegações (evita loop infinito)
- [ ] Transparência: UI mostra quem está trabalhando e o quê

### Perguntas ao usuário (via `ask`)
```
Pergunta: "Entendi seu objetivo. Posso abordar assim:
1. Analista define requisitos
2. Dev implementa
3. QA valida
4. Eu reviso e entrego
Ou prefere outro fluxo?"
Opções: [✅ Seguir esse plano] [✏️ Ajustar] [🤖 Deixar você decidir tudo]

Pergunta (durante execução):
"O QA encontrou 2 problemas. O gerente sugere voltar ao dev.
Quer acompanhar ou deixar automático?"
Opções: [🤖 Automático] [👀 Quero aprovar cada passo]
```

### Repositórios para estudar
| Repo | O que copiar | Link |
|------|-------------|------|
| MetaGPT | Gerente de projeto que delega dinamicamente | https://github.com/FoundationAgents/MetaGPT |
| AutoGen | Agentes que conversam e negociam tarefas | https://github.com/microsoft/autogen |
| CrewAI | Crews com manager agent + workers | https://github.com/crewAIInc/crewAI |
| milanimcgraw/Multi-Agent-Systems-with-crewAI | Exemplos reais de crews multi-papel | https://github.com/milanimcgraw/Multi-Agent-Systems-with-crewAI |

---

## Fase 6: Skill Store / Comunidade

### Conceito
Usuários compartilham skills empacotadas. Outros instalam com um clique.

### Tarefas
- [ ] Definir formato de pacote de skill (pasta + metadata)
- [ ] Criar registry/index local ou remoto
- [ ] UI de busca e instalação de skills
- [ ] Sistema de avaliação/review
- [ ] Verificação de segurança (permissões declaradas)

### Perguntas ao usuário (via `ask`)
```
Pergunta: "Quer explorar skills da comunidade?"
Opções: [🔍 Buscar skills] [📦 Instalar de arquivo] [➕ Criar nova]

Pergunta (ao instalar):
"Esta skill pede permissão para: ler arquivos, executar comandos.
Confia neste autor?"
Opções: [✅ Instalar] [❌ Cancelar] [👀 Ver código antes]
```

---

## Referência Rápida: Repositórios GitHub por Categoria

### Equipes de Software (Multi-Agente)
| Projeto | Descrição | Link |
|---------|-----------|------|
| MetaGPT | Simula empresa de software (PM, Architect, Engineer, QA) | https://github.com/FoundationAgents/MetaGPT |
| ChatDev | Empresa virtual (CEO, CTO, Programmer, Reviewer, Tester) | https://github.com/OpenBMB/ChatDev |
| CrewAI | Framework leve para crews com roles/tasks | https://github.com/crewAIInc/crewAI |
| AutoGen | Multi-agent conversation framework (Microsoft) | https://github.com/microsoft/autogen |

### Code Review e QA
| Projeto | Descrição | Link |
|---------|-----------|------|
| PR-Agent (Qodo) | Code review automático em PRs GitHub/GitLab | https://github.com/qodo-ai/pr-agent |
| PR-Agent (legacy) | Versão open-source mantida pela comunidade | https://github.com/The-PR-Agent/pr-agent |

### Automação Local e Produtividade
| Projeto | Descrição | Link |
|---------|-----------|------|
| Open Interpreter | Agente local que executa código no PC | https://github.com/openinterpreter/openinterpreter |
| Composio | Conectores prontos (Gmail, Slack, Notion, GitHub) | https://github.com/ComposioHQ/composio |
| LangChain | Biblioteca de tools e chains para agentes | https://github.com/langchain-ai/langchain |

### Listas Curadas (Exploração)
| Projeto | Descrição | Link |
|---------|-----------|------|
| awesome-ai-agents (e2b) | Lista de agentes autônomos | https://github.com/e2b-dev/awesome-ai-agents |
| awesome-agents (kyrolabs) | Ferramentas e produtos open-source | https://github.com/kyrolabs/awesome-agents |
| awesome-ai-agents (aloth) | Frameworks, tools, papers, recursos | https://github.com/aloth/awesome-ai-agents |
| awesome_ai_agents (jim-schwoebel) | 1500+ recursos relacionados a agentes | https://github.com/jim-schwoebel/awesome_ai_agents |

### Exemplos Práticos de Crews
| Projeto | Descrição | Link |
|---------|-----------|------|
| crewAI-examples | Exemplos oficiais end-to-end | https://github.com/crewAIInc/crewAI-examples |
| Multi-Agent-Systems-with-crewAI | Crews de finanças, suporte, pesquisa | https://github.com/milanimcgraw/Multi-Agent-Systems-with-crewAI |
| Awesome-AI-Agents-HUB-for-CrewAI | Projetos multi-agente com CrewAI | https://github.com/OneDuckyBoy/Awesome-AI-Agents-HUB-for-CrewAI |

---

## Princípios de Segurança (Todas as Fases)

1. **Permissões por skill** — cada skill declara o que precisa (leitura, escrita, execução, rede)
2. **Confirmação para ações destrutivas** — apagar, mover em massa, enviar e-mail, executar comando
3. **Preview / dry-run** — mostrar o que será feito antes de executar
4. **Logs** — registrar quem pediu, qual skill, parâmetros, resultado, timestamp
5. **Undo** — permitir desfazer sempre que possível
6. **Sandbox** — execução isolada, limite de tempo/memória, sem root/admin
7. **Guarda de profundidade** — sub-agentes não podem delegar recursivamente (já implementado no Cerne)
8. **Limite de rounds** — máximo de iterações em pipelines para evitar loops infinitos

---

## Estratégia de Modelo: Local vs API

| Uso | Recomendado | Motivo |
|-----|-------------|--------|
| Chat simples, classificação de intenção | Modelo local | Privacidade, custo zero, offline |
| Resumos, respostas rápidas | Modelo local | Latência baixa |
| Planejamento complexo, code review | API forte (Claude/GPT) | Raciocínio multi-etapa confiável |
| Análise de negócio, geração de testes | API forte | Saída estruturada consistente |
| Pipeline multi-agente | Híbrido | Local para triagem, API para etapas críticas |

---

## Checklist de Implementação Progressiva

```
Fase 1 ☑ Skills básicas (já feito)
  └─ ☐ Documentar formato SKILL.md na ajuda
  └─ ☐ Skills de exemplo embarcadas

Fase 2 ☐ Prompts prontos como agentes leves
  └─ ☐ Code Reviewer
  └─ ☐ QA Tester
  └─ ☐ Analista de Negócios
  └─ ☐ English Tutor
  └─ ☐ File Organizer

Fase 3 ☐ Pipeline Dev → QA → Analista
  └─ ☐ pipeline.rs com máquina de estados
  └─ ☐ Handoffs estruturados
  └─ ☐ Vereditos via saída JSON/tool call
  └─ ☐ Eventos de UI por etapa

Fase 4 ☐ Skills de produtividade
  └─ ☐ file_organizer
  └─ ☐ email_triage (via MCP)
  └─ ☐ english_tutor

Fase 5 ☐ Multi-agente gerenciado
  └─ ☐ Agente gerente com catálogo de skills
  └─ ☐ Delegação via task
  └─ ☐ Limite de delegações

Fase 6 ☐ Skill store / comunidade
  └─ ☐ Formato de pacote
  └─ ☐ Registry e UI de instalação
  └─ ☐ Verificação de segurança
```
