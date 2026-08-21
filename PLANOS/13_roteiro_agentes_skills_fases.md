# Roteiro de Agentes, Skills e Ferramentas — Fases e Tarefas

> Documento de referência para implementar agentes e skills no Cerne Code de forma progressiva.
> Cada fase tem tarefas concretas, perguntas que podem ser feitas ao usuário via `ask` no composer,
> e repositórios GitHub prontos para estudar/copiar estratégias.
>
> **Análise de código-fonte real** dos repositórios clonados em `F:\AgentesESkills\` está na seção
> "Análise Detalhada dos Repositórios" ao final deste documento.
>
> **Revisão de 2026-08-14**: documento reestruturado em duas partes.
> - **Parte I** (fases 1-6, conteúdo original) — *o que* os agentes/skills fazem: papéis simulados,
>   pipeline dev→QA→analista, skills de produtividade, gerente multi-agente, skill store.
> - **Parte II** (fases A-F, nova) — *como* o sistema funciona por baixo e nas telas: rastreamento
>   por UUID, execução isolada/paralela vs. fila local, modos YOLO/Manual aplicados a agentes/skills,
>   telas de catálogo e execução em tempo real, visualizador de diff + navegador de arquivos, e
>   integração no composer (fable, MCPs, ícone de visão, atalho `/`, modais de Ajuda/Config/Sobre).
> - Repositório novo analisado nesta revisão: **picoClaw** (`F:\AgentesESkills\picoclaw`), framework
>   de agente em Go da Sipeed — tem formato `SKILL.md` idêntico ao do Cerne e um sistema de
>   sub-turnos (`SubTurnConfig`) que é a referência mais próxima do que a Parte II pede.

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

### 7. picoClaw (`F:\AgentesESkills\picoclaw`)

> Clonado em 2026-08-14 de `https://github.com/sipeed/picoclaw`. Apesar da Sipeed ser fabricante de
> hardware (LicheeRV, MaixCAM), o picoClaw **não é um SDK de placa** — é um framework de agente de IA
> completo em Go, com o pitch de rodar em hardware embarcado baratíssimo ($10, 10MB RAM). O que
> importa pro Cerne são os *padrões*, não o código (Go não é portável para Rust).

#### ⭐⭐ Formato de skill IDÊNTICO ao do Cerne
`workspace/skills/<nome>/SKILL.md`, frontmatter YAML com `name`/`description` (+ `metadata` opcional
de compatibilidade com o nanobot). Skills reais no repo: `agent-browser`, `github`, `hardware`,
`picoclaw-agent`, `skill-creator`, `summarize`, `tmux`, `weather`.

```yaml
---
name: weather
description: Get current weather and forecasts with verified location matching (no API key required).
homepage: https://wttr.in/:help
metadata: {"nanobot":{"emoji":"🌤️","requires":{"bins":["curl"]}}}
---
```

**O que aproveitar**: confirma que o formato `SKILL.md` do Cerne está alinhado com o padrão de
mercado (mesmo usado pelo Claude Code). Diferenças a considerar:
- Loader (`pkg/skills/loader.go`) resolve em 3 níveis — **workspace (projeto) > global > builtin**
  (linhas 66-95) — o Cerne só tem 2 (`{app_data}/skills` e `<projeto>/.cerne/skills`). Um nível
  "builtin" (skills embarcadas no instalador, somente leitura) fecha a tarefa pendente da Fase 1
  ("Adicionar 3-5 skills de exemplo embarcadas").
- `MaxNameLength = 64` / `MaxDescriptionLength = 1024` (`loader.go:24-26`) — o Cerne não valida
  tamanho hoje; útil para não deixar a `description` estourar o orçamento de contexto do catálogo.
- Parser aceita YAML aninhado no frontmatter; o parser do Cerne (`skills.rs::split_frontmatter`) é
  flat `key: value` linha a linha — suficiente para os campos atuais, mas não daria pra copiar o
  `metadata: {...}` do exemplo acima sem trocar de parser.

#### ⭐ `skill-creator` como skill "meta" (`workspace/skills/skill-creator/SKILL.md`, 359 linhas)
Ensina o próprio agente a criar/editar outras skills. Define a convenção "Anatomy of a Skill":
```
skill-name/
├── SKILL.md (required)
│   ├── name: (required)
│   └── description: (required)
└── Bundled Resources (optional)
    ├── scripts/     — código executável (Python/Bash/etc.)
    ├── references/  — documentação carregada sob demanda
    └── assets/      — arquivos usados no output (templates, ícones, fontes)
```
E "Progressive Disclosure" em 3 níveis: metadata sempre em contexto (~100 palavras) → corpo
carregado só quando a skill dispara (<5k palavras) → recursos de `scripts/`/`references/`/`assets/`
sob demanda. Regra de nome: `^[a-z0-9]+(-[a-z0-9]+)*$`, <64 chars.

**O que aproveitar**: o Cerne hoje só tem `SKILL.md` solto (sem `scripts/`/`references/`/`assets/`).
Adotar a convenção de subpastas é uma extensão natural e de baixo risco — não quebra skills
existentes (elas continuam funcionando como só-`SKILL.md`), só adiciona uma opção.

#### ⭐⭐ `AgentFrontmatter` — definição de agente nomeado (`pkg/agent/definition.go:30-39`)
```go
type AgentFrontmatter struct {
    Name        string
    Description string
    Tools       []string
    Model       string
    MaxTurns    *int
    Skills      []string
    MCPServers  []string
}
```
**O que aproveitar**: é exatamente o que falta no Cerne para "usuário criar seus próprios agentes"
— hoje o Cerne tem 1 único `SYSTEM_PROMPT` hardcoded em `agent/mod.rs`, sem conceito de múltiplos
agentes nomeados com toolset/model/skills/MCP próprios. Este struct é o modelo de dados de
referência para o `AGENT.md` proposto na Fase A3 (Parte II).

#### ⭐⭐⭐ `SubTurnConfig` — sub-turnos síncronos e assíncronos (`pkg/tools/subagent.go:19-34`)
```go
type SubTurnConfig struct {
    Model, SystemPrompt, ActualSystemPrompt string
    Tools []tools.Tool
    MaxTokens int
    Temperature float64
    Async bool           // true = spawn (fire-and-forget), false = subagent (bloqueante)
    Critical bool        // sobrevive ao fim do turno pai
    Timeout time.Duration
    MaxContextRunes int  // 0=auto, -1=sem limite, >0=explícito
    TargetAgentID string // delega pra outro agente nomeado
}
```
Documentado em `docs/architecture/subturn.md`: profundidade máxima **3 níveis**, concorrência
máxima **5 sub-turns simultâneos** por turno pai (semáforo com timeout de 30s), sessão efêmera
limitada a 50 mensagens.

**O que aproveitar**: é a referência mais próxima do que foi pedido para a Fase A (Parte II) —
"agente pode chamar outro agente sem perder referência" e "execução isolada, local em fila / API em
paralelo". Hoje `subagent.rs` do Cerne só tem 1 nível (bloqueia recursão totalmente, nem 2 níveis) e
é sempre síncrono/bloqueante, sem equivalente ao `Async`/`spawn` (fire-and-forget) nem ao `Critical`
(sobreviver ao turno pai). O `TargetAgentID` é o mecanismo de "chamar outro agente nomeado" — o
Cerne não tem isso porque só existe 1 agente.

#### ⭐ Ferramenta `spawn` assíncrona com allowlist (`pkg/tools/spawn.go:65-67,103-108,128-157`)
`SetAllowlistChecker` restringe para quais `agent_id` um agente pode delegar. Relevante só depois
que o Cerne tiver múltiplos agentes nomeados (Fase B3).

#### ⭐ Hooks tipados (`pkg/agent/hooks.go:27-45`)
`HookAction`: `Continue`, `Modify`, `Respond` (bypassa aprovação — risco de segurança documentado no
próprio comentário do código-fonte), `DenyTool`, `AbortTurn`, `HardAbort`. Mais granular que o
`ExecutionMode::Manual` do Cerne (hoje só aprovar/negar). Não é prioridade imediata, mas vale como
inspiração se o modo Manual precisar de mais nuance no futuro (ex.: "modificar argumentos antes de
rodar" em vez de só aprovar/negar).

#### ❌ O que é repetido/não vale copiar
- Todo código de hardware embarcado (`pkg/devices/`, `pkg/audio/*`, `pkg/isolation/platform_linux.go`)
- Camada de canais de chat (`pkg/channels/telegram|discord|slack|wecom|matrix|qq|whatsapp`, dezenas)
  — o Cerne é app desktop single-user, não bot multi-canal
- `pkg/evolution/*` (auto-evolução de skills via LLM observando padrões de uso) — ideia interessante
  de longuíssimo prazo, complexidade desproporcional ao estágio atual do Cerne
- `pkg/seahorse/*` (compactação de contexto via SQLite FTS5) — o Cerne já tem `maybe_compact` em
  `agent/mod.rs`, mais simples e suficiente
- `pkg/routing/` (roteamento automático de modelo) — fora de escopo, usuário escolhe manualmente
- `pkg/auth/oauth.go`, `pkg/credential/` — o Cerne já tem esquema mais simples de API key

---

### 8. awesome-ai-agents (`F:\AgentesESkills\awesome-ai-agents`)

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
| Skill com `scripts/`/`references/`/`assets/` | picoClaw | `workspace/skills/skill-creator/SKILL.md` | 🟡 P1 |
| Skills builtin (3º nível, somente leitura) | picoClaw | `pkg/skills/loader.go:66-95` | 🟡 P1 |
| Definição de agente nomeado (`AGENT.md`) | picoClaw | `pkg/agent/definition.go:30-39` | 🔴 P0 |
| Sub-turno síncrono/assíncrono + delegação por ID | picoClaw | `pkg/tools/subagent.go:19-34` | 🔴 P0 |
| Limites de profundidade/concorrência de sub-turno | picoClaw | `docs/architecture/subturn.md` | 🟡 P1 |
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
- [x] **CONCLUÍDO — 2026-08-16 (Fase B2/T47)**: 3 skills de exemplo (`weather`, `summarize`,
  `skill-creator`) semeadas na primeira execução via `skills::ensure_global_skills_dir`
  (`skills_examples/*.md`, `include_str!`). A própria `skill-creator` já ensina o formato
  `SKILL.md` dentro do catálogo, cobrindo parte do item acima (documentação embutida, não só na
  ajuda).
- [ ] Permitir importar skill de URL ou arquivo local — **replanejado como Fase 6 (MVP)**, ver
  seção "Fase 6" abaixo: é a mesma funcionalidade, só reenquadrada como o primeiro passo real de
  "skill store" em vez de item solto da Fase 1.

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

### Replanejamento — 2026-08-16: `ReadyPrompt` não é mais o mecanismo certo
Quando esta fase foi esboçada, `ReadyPrompt` (`content/prompts.ts`) era o único mecanismo de
"comportamento pré-configurado" do Cerne. Hoje não é mais — `Persona` (Fase A3/T24/T48/B3,
`personas.rs`) já faz EXATAMENTE o que os dois primeiros itens abaixo pediam: `system_prompt_override`
já existe (é `Persona.content`), e "filtra ferramentas" já existe (`Persona.tools`) — desde a B3, até
"filtra skills" (`Persona.skills`). `ReadyPrompt` continua certo pra outra coisa (inserir um TEXTO de
pedido pronto no composer, ex. "resuma esta página"), não pra mudar o COMPORTAMENTO da sessão inteira
— são dois mecanismos com propósitos diferentes que só pareciam a mesma coisa antes da Persona existir.
- [x] **Já implementado como Persona**: `system_prompt_override` = `Persona.content`; filtro de
  ferramentas = `Persona.tools`; filtro de skills = `Persona.skills`. Nenhuma tarefa nova de backend.
- [ ] **Único item real que falta**: cadastrar as personas de exemplo abaixo (conteúdo, não código) —
  🔍 Code Reviewer, 🧪 QA Tester, 📋 Analista de Negócios, 🇬🇧 English Tutor, 📁 File Organizer. Ver
  decisão de escopo logo abaixo sobre SE vale semear isso por padrão.
- [ ] **Decisão a tomar antes de criar as personas de exemplo**: seguir o mesmo padrão de "semear só
  na primeira execução" que as skills de exemplo usam (`ensure_global_skills_dir`), ou deixar 100%
  self-service (usuário cria a própria persona pelo `PersonaEditorModal.vue`, que já é trivial)? A
  favor de semear: reduz fricção de descoberta ("o que dá pra fazer aqui?"). Contra: infla a lista de
  Personas de instalação nova com conteúdo opinativo que o usuário pode nem querer, e roda o risco de
  ficar desatualizado (mesmo trade-off já documentado pras skills de exemplo). Recomendo NÃO semear
  automaticamente dessa vez — em vez disso, documentar essas 5 personas como exemplos copiáveis na
  ajuda do app (mesmo lugar do item pendente da Fase 1 "documentar formato"), e deixar o usuário
  colar/adaptar se quiser. Mais barato, sem efeito colateral de poluir a lista de ninguém.

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

### Tarefas (visão original, mantida como referência) — ✅ TODAS IMPLEMENTADAS (2026-08-16)
- [x] Criar `pipeline.rs` com máquina de estados dev→qa→analista
- [x] Definir structs de handoff entre etapas — resumo estruturado é o próprio relatório final de
  cada etapa (texto livre, mas com formato pedido no prompt de cada uma), não um struct tipado —
  decisão consciente, ver detalhe granular abaixo.
- [x] Usar saída JSON ou tool call forçado para vereditos — implementado como convenção de texto
  (primeira linha "APROVADO"/"REFUTADO", mesmo padrão que `verify_completion` já usava e provou
  funcionar), não JSON estruturado nem tool call forçado.
- [x] Limite de rounds configurável (default: 3) — `pipeline::DEFAULT_MAX_ROUNDS`, parâmetro
  `max_rounds` opcional na tool.
- [x] Eventos de UI: `agent:pipeline_status` com os 3 textos exatos do desenho original.
- [x] Integrar como item do menu `/` (Fase E2) — resolvido de graça pelo E2 já estar pronto,
  substitui a decisão pendente "ReadyPrompt ou comando /pipeline?".

### Replanejamento granular — 2026-08-16 (plano original, ver bloco "Implementado" abaixo dele)

Quando isso foi esboçado, o Cerne não tinha nada parecido com sub-agente. Hoje tem dois: `task`
(`agent/subagent.rs`, agente de propósito geral com toolset quase completo) e `verify_completion`
(`agent/verifier.rs`, verificador ADVERSARIAL read-only que já devolve exatamente APROVADO/REFUTADO +
evidência real, nunca confiando em "parece certo"). O pipeline Dev→QA→Analista É estruturalmente 3
chamadas encadeadas desses padrões, com uma diferença crucial do resto do Cerne: aqui a SEQUÊNCIA é
fixa em código Rust, não escolhida pelo LLM turno a turno — o LLM do topo pode decidir COMEÇAR o
pipeline (como já decide chamar `task`), mas não decide o que acontece depois de começar.

**Tarefa 3.1 — Etapa DEV [BE]** ✅ IMPLEMENTADO — 2026-08-16
- [x] Reaproveitado `subagent::run` como estava, ZERO mudança no arquivo — `pipeline.rs` monta o
  prompt ("Implemente: {requisito}...") e chama direto.

**Tarefa 3.2 — Etapa QA [BE]** ✅ IMPLEMENTADO
- [x] Reaproveitado `verifier::run` DIRETO, ZERO mudança no arquivo — `dev_report` vira
  `task_summary`, um texto fixo pedindo pra rodar teste/build vira `how_to_verify`.

**Tarefa 3.3 — Etapa ANALISTA [BE, novo]** ✅ IMPLEMENTADO
- [x] `agent/analyst.rs` criado copiando `verifier.rs` quase 1:1 (mesmo toolset, mesmo formato de
  veredito), só trocando o prompt pra focar em cobertura de requisito. Decisão de copiar em vez de
  generalizar `verifier.rs`, exatamente como planejado, mantida.

**Tarefa 3.4 — Orquestração determinística [BE, novo]** ✅ IMPLEMENTADO
- [x] `agent/pipeline.rs::run(...)` — assinatura final ficou mais rica que o esboço original
  (recebe `cfg`/`api_key`/`model`/`extra_read_paths`/`enabled_mcp_servers` também, pra poder chamar
  `subagent::run`/`verifier::run`/`analyst::run` de verdade), mas o laço é exatamente o desenhado:
  DEV → QA → (REFUTADO → volta pro DEV) → ANALISTA → (REFUTADO → volta pro DEV) → entrega.
  `max_rounds` default 3 (`pipeline::DEFAULT_MAX_ROUNDS`), nunca trava silenciosamente.
- [x] Tool `run_pipeline(requirement, max_rounds?)` criada em `agent/tools.rs`, dispatch tratado à
  parte em `agent/mod.rs` (mesmo padrão de `task`/`verify_completion`).
- [x] Guarda de profundidade: `run_pipeline` excluída de `subagent_tool_specs()` (DEV não pode
  disparar outro pipeline) — teste de regressão atualizado (`parent_names.len() - 4`).

**Tarefa 3.5 — Rastreamento e eventos de UI [BE+FE]** ✅ IMPLEMENTADO
- [x] `parent_id` usado pela primeira vez de verdade — `start_agent_execution` ganhou parâmetro
  `parent_id: Option<&str>`, cada etapa do pipeline é filha da execução do pipeline inteiro.
  `AgentExecutionsPanel.vue` não precisou de mudança nenhuma, como previsto.
- [x] Evento `agent:pipeline_status` implementado com exatamente os 3 textos do desenho original —
  `ChatView.vue::statusLabel` mostra com prioridade sobre o status genérico enquanto ativo.

**Tarefa 3.6 — Gatilho no composer [FE]** ✅ IMPLEMENTADO, com um ajuste em relação ao plano
- [x] Item novo no menu `/` (`kind: "pipeline"`, só aparece com projeto associado), como planejado.
- **Ajuste na implementação**: o plano original dizia "chama `run_pipeline` direto... com o texto já
  digitado como `requirement`" — na prática isso não faz sentido com o resto do design do menu `/`:
  o popover só fica aberto enquanto o texto é exatamente `/` + a query de busca (sem espaço), então
  no momento de selecionar o item o usuário ainda não digitou o requisito nenhum, só filtrou o menu.
  Implementado como as outras opções do menu (skill/persona): seleciona → insere uma frase-modelo
  ("Use o pipeline Dev → QA → Analista para: ") e deixa o cursor pronto pro usuário completar — o
  agente principal é quem de fato chama `run_pipeline` ao processar essa mensagem, não o frontend.

**Perguntas em aberto — decididas na implementação**
- Em modo Manual, `ask` extra entre rounds? **Não implementado** — `ExecutionMode::Manual` já pausa
  em toda tool call do DEV a cada round, seria redundante. Recomendação original seguida.
- Limite de tempo/tokens por etapa além de `max_rounds`? **Não implementado** — doom-loop detection
  + `MAX_AGENTIC_STEPS` de cada sub-chamada (`subagent`/`verifier`/`analyst` já têm seus próprios
  limites de passos) cobrem loop patológico dentro de uma etapa. Recomendação original seguida.

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

### Tarefas (visão original, mantida como referência)
- [ ] Criar skill `file_organizer` com prompt + handler
- [ ] Criar skill `email_triage` com prompt + integração MCP
- [ ] Criar skill `english_tutor` com prompt adaptativo
- [ ] Adicionar como prompts prontos no composer

### Replanejamento granular — 2026-08-16

As três já não precisam de "handler" nenhum em Rust — toda a infraestrutura que faltava quando isso
foi esboçado (ferramentas de arquivo, MCP genérico, allowlist de tools/skills por persona, e agora
até ferramentas Python sob demanda via T17) já existe. As três viram essencialmente CONTEÚDO
(SKILL.md ou Persona), não código novo — com uma ressalva de arquitetura por item abaixo.

**4a. File Organizer — é skill, não persona** ✅ IMPLEMENTADO — 2026-08-16
- [x] `src-tauri/src/skills_examples/file_organizer.md`, seedada via `EXAMPLE_SKILLS` (`skills.rs`),
  mesmo mecanismo do B2/T47. Ensina exatamente o fluxo planejado: `list_dir` → classificar → `ask`
  confirmando o plano → mover via `run_command`.
- [x] **Undo via T17**: implementado como conteúdo da skill, exatamente como planejado — instrui
  usar `create_python_tool` pra gerar um organizador reutilizável com manifesto de undo
  (`origem→destino` + flag `--reverse`), em vez de qualquer mecanismo novo no Cerne.
- [x] Segurança confirmada sem mudança, como previsto.

**4b. Email Triage — depende 100% de MCP do usuário, fora do escopo do Cerne em si** ✅ IMPLEMENTADO
- [x] `src-tauri/src/skills_examples/email_triage.md`, seedada do mesmo jeito. Ensina o fluxo
  assumindo MCP de e-mail já conectado, verifica `mcp__*` disponíveis antes de agir e para com aviso
  claro se não tiver nenhum.
- [x] Fora de escopo documentado dentro da própria skill (não implementei lista de servidores MCP de
  e-mail conhecidos na ajuda — reduzido: a skill já deixa claro o pré-requisito, uma lista curada de
  servidores de terceiros ficaria desatualizada rápido e não é crítica pro fluxo funcionar).

**4c. English Tutor — é PERSONA, não skill** ✅ IMPLEMENTADO
- [x] Documentado (NÃO semeado automaticamente, mesma decisão da Fase 2) — nova seção "## Personas"
  adicionada aos 4 arquivos de ajuda (`content/help.*.md`, primeira vez que Persona é documentada na
  ajuda do app) com o formato completo + exemplo copiável do Tutor de Inglês.
- [x] Conteúdo da persona segue exatamente o desenho: nível A1-C2 perguntado via `ask` na primeira
  mensagem (sem campo estruturado novo em `models.rs`), correção com explicação, modos de prática,
  `tools: ["web_search"]` só.

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

### Tarefas (visão original, mantida como referência)
- [ ] Criar agente "gerente" com system prompt de planejamento
- [ ] Dar ao gerente acesso à lista de skills disponíveis (catálogo)
- [ ] Gerente chama `task` para delegar a agentes especializados
- [ ] Gerente sintetiza resultados e decide próximos passos
- [ ] Limite de delegações (evita loop infinito)
- [ ] Transparência: UI mostra quem está trabalhando e o quê

### Replanejamento granular — 2026-08-16

**Achado principal**: item por item, os 6 pontos acima já são verdade HOJE, sem nenhuma linha de
código nova — é a diferença estrutural entre Fase 3 (sequência FIXA, precisou de `pipeline.rs` novo)
e Fase 5 (sequência DINÂMICA, o LLM do topo decide quem chamar). O "gerente" já É o agente principal
de qualquer sessão: ele já vê o catálogo de skills no system prompt (Fase A6), já pode chamar `task`
quantas vezes quiser inclusive em paralelo quando o provider é de API (Fase A4/T32), já sintetiza
resultado e decide o próximo passo (é literalmente o que um agente com ferramentas faz), já é
limitado contra loop infinito (`MAX_AGENTIC_STEPS`, doom-loop detection), e já é transparente (Fase
C2, `AgentExecutionsPanel.vue` mostra cada `task` em tempo real).

- [x] **IMPLEMENTADO — 2026-08-16**: `src-tauri/src/skills_examples/project_manager.md`, seedada via
  `EXAMPLE_SKILLS` (`skills.rs`) — virou skill, não Persona (decisão: coordenar delegação é pontual
  pra um pedido específico, não um modo que deveria valer a sessão inteira). Ensina exatamente o
  padrão planejado: decompor em sub-tarefas independentes, `task` por sub-tarefa, sintetizar,
  repetir se precisar. Conteúdo puro, zero código novo, como previsto.
- [x] **IMPLEMENTADO**: nuance sobre lotes pequenos incluída no corpo da skill, junto de uma seção
  "quando NÃO usar" (pedidos com sub-tarefas em cadeia/dependentes não se beneficiam de delegação
  paralela) que o plano original não tinha explicitado.

### Caminho futuro: encaixe com a Fase G (sessões paralelas orquestradas)

A Fase G (`start_agent_session`/`check_agent_session`, documentada mas **não implementada** — ver
seção própria mais abaixo neste arquivo) descreve algo mais visível que `task`: cada sub-tarefa vira
uma SESSÃO de verdade (aparece na sidebar, tem histórico, pode ser reaberta), em vez de um sub-agente
efêmero e invisível. Se a Fase G for implementada um dia, o "Gerente de Projeto" desta Fase 5 seria o
primeiro caso de uso natural pra ela — hoje ele delega via `task` (efêmero, correto, mas menos
visível); com a Fase G ele delegaria via `start_agent_session` (cada trabalhador vira uma sessão
navegável). Não é bloqueante — a Fase 5 funciona hoje só com `task` — mas vale a pena revisitar essa
skill quando/se a Fase G acontecer.

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

### Tarefas (visão original, mantida como referência)
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

### Replanejamento — 2026-08-16: esta é a fase mais especulativa do roteiro inteiro

Diferente das Fases 3-5 (que só precisavam reconectar peças que já existem), Fase 6 é a única que
genuinamente precisa de decisões de produto/hospedagem que ninguém tomou ainda — "registry remoto"
implica alguém mantendo um servidor, "sistema de avaliação/review" implica contas de usuário. Isso é
um salto grande pra um app local-first sem equipe de backend dedicada. Proponho 3 degraus, do mais
barato pro mais caro, e recomendo parar no primeiro até haver sinal real de demanda.

**Degrau 1 (MVP, zero infraestrutura nova) — "Importar skill de URL"** ✅ IMPLEMENTADO — 2026-08-16
- [x] Fecha o item pendente da Fase 1 ("permitir importar skill de URL ou arquivo local").
- [x] **[FE]** `SkillImportModal.vue` novo, botão "Importar de URL" ao lado de "+ Nova skill" na aba
  Skills do `AgentsSkillsPanel.vue` — busca via novo comando `fetch_skill_from_url` (backend faz o
  fetch, não o navegador, evita qualquer questão de CORS/CSP), mostra o conteúdo COMPLETO num
  textarea editável, salva via novo comando `import_skill` (backend valida o frontmatter e o `name`
  obrigatório, reaproveitando `split_frontmatter` que já existia).
- [x] **Segurança implementada como planejado**: preview obrigatório antes de salvar (nunca "instala
  direto"), `fetch_skill_from_url` reaproveita a mesma validação de URL pública que `web_fetch` já
  usa (bloqueia localhost/rede interna). Deliberadamente fora do escopo: ferramentas Python (T17) no
  fluxo de import — mesma distinção de risco já registrada abaixo continua valendo.

**Degrau 2 (se o Degrau 1 pegar) — índice curado, ainda sem conta de ninguém**
- [ ] Um `index.json` hospedado num repo que o próprio Cerne mantém (GitHub Pages ou raw do
  próprio repositório do projeto) listando `{name, description, author, raw_url}` — o app busca essa
  lista e mostra um catálogo navegável/buscável, cada item usa o mesmo fluxo de importar-de-URL do
  Degrau 1 por baixo. Ainda sem contas de usuário, sem review, sem hospedar nada além de um arquivo
  JSON estático — qualquer um pode propor entrada via PR no repo do índice.

**Degrau 3 (loja de verdade) — fora de escopo realista por enquanto**
- [ ] Registry com submissão própria, avaliação/review, contas — **não recomendo perseguir isso**
  sem uma decisão explícita do projeto de assumir manutenção de backend/hospedagem contínua. Deixo
  documentado só pra não perder a ideia, não como próximo passo.

**Nuance importante que o desenho original não tinha (porque é anterior ao T17)**: se um dia o
Degrau 1/2 for estendido pra compartilhar não só skills (texto) mas **ferramentas Python** (T17,
`create_python_tool`/scripts reais que RODAM no computador do usuário via `uv run`), a superfície de
risco muda de categoria — de "texto que pode tentar manipular o LLM" pra "código arbitrário de
terceiro rodando de verdade". Se isso for cogitado no futuro, precisa de um aviso bem mais forte
("este script foi baixado de fora — leia antes de rodar, ele executa código de verdade no seu
computador") do que o preview de texto que basta pra skill. Recomendo NÃO incluir ferramentas Python
no escopo de compartilhamento até essa distinção ser resolvida explicitamente.

---

# Parte II — Infraestrutura de Execução e UX (Fases A-F)

> Adicionado em 2026-08-14. Enquanto a Parte I define **que papéis/pipelines** os agentes simulam,
> a Parte II define **como o sistema roda por baixo e nas telas**: rastreamento de chamadas
> aninhadas, isolamento de contexto, fila vs. paralelismo, modos YOLO/Manual, telas de catálogo e
> execução em tempo real, visualizador de diff, e integração no composer.
>
> As fases A-F são majoritariamente **independentes** das fases 1-6 da Parte I — podem ser
> implementadas em paralelo ou antes, já que 1-6 descreve comportamento (o que os agentes fazem)
> e A-F descreve mecanismo (como qualquer agente/skill roda e aparece na tela). A ordem sugerida
> dentro da Parte II segue dependência técnica: A (backend core) → B/C/D (telas que dependem de A)
> → E/F (composer e modais, quase independentes do resto).
>
> Convenção de tarefas: cada task marca **[BE]** (backend Rust), **[FE]** (frontend Vue) ou **[BE+FE]**.

## Visão Geral das Fases (Parte II)

| Fase | Nome | Objetivo | Complexidade |
|------|------|----------|-------------|
| A | Infraestrutura de Execução | UUID de rastreamento, isolamento de contexto, fila local vs. paralelo API, YOLO/Manual para agentes/skills | Alta |
| B | Catálogo de Agentes/Skills | Tela de listagem/seleção/uso atual + criação de agentes próprios + import picoClaw | Média |
| C | Painel de Execução em Tempo Real | Expandir painel de tarefas em segundo plano + sessão de agente/skill visualizável (read-only, cancelável) | Média |
| D | Repositório: Diff + Navegador de Arquivos | Visualizador de diff completo, seletor de pasta, navegador de arquivos clicável | Média |
| E | Composer: Unificação e Novos Elementos | Fable como skill?, atalho `/`, modal de MCPs por sessão, ícone de visão cacheado, copiar código | Baixa-Média |
| F | Modais de Ajuda/Configurações/Sobre | Trocar view por modal sem perder rascunho do composer | Baixa |

---

## Fase A: Infraestrutura de Execução

### Contexto atual (código real, ver "Análise Detalhada" B.1 mais abaixo neste doc)
- `agent/subagent.rs`: `task` é sempre síncrono/bloqueante, 1 nível só (bloqueia recursão via
  toolset sem `task`), sem UUID próprio — reusa `session_id` da sessão pai só para emitir eventos.
- `agent/background.rs`: jobs em `HashMap<String, JobHandle>` **global ao app** (não por sessão),
  leitura é **pull** (`check_background_output`), sem evento push em tempo real.
- `models.rs::ExecutionMode`: `Manual` (pausa toda tool call) / `Auto` / `Yolo` — já existe, mas se
  aplica a *tool calls individuais*, não ao nível "vou chamar o agente Code Reviewer agora".
- `providers/mod.rs::chat_stream`: 1 request HTTP por vez, sequencial — não há pool concorrente.

### Tarefa A1 — UUID de rastreamento de execução [BE]
- [ ] Adicionar `execution_id: Uuid` (crate `uuid` já é dependência comum em projetos Tauri; se não
  estiver em `Cargo.toml`, adicionar) a toda invocação de agente/skill — gerado em `task`/`load_skill`.
- [ ] Adicionar `parent_execution_id: Option<Uuid>` para reconstruir a árvore de chamadas (agente A
  chama skill B chama agente C → cada nó sabe seu pai).
- [ ] Propagar `execution_id` nos eventos Tauri emitidos durante aquela execução (`agent:tool_call`,
  `agent:tool_result`, `agent:status` etc. ganham um campo `execution_id` opcional).
- [ ] Struct nova em `models.rs`, ex. `AgentExecution { id: Uuid, parent_id: Option<Uuid>, kind: AgentExecutionKind /* Task | Skill | Verifier */, name: String, status: ExecutionStatus /* Queued | Running | Done | Failed | Cancelled */, started_at_ms, finished_at_ms: Option<u64> }`.
- [ ] Registro em memória (similar a `BackgroundJobs`) — `AgentExecutions(Mutex<HashMap<Uuid, AgentExecution>>)` no estado do app, para a UI consultar "quem está rodando agora" (requisito da Fase B1).

### Tarefa A2 — Sessão isolada por execução [BE]
- [ ] Cada execução de agente/skill roda com seu próprio histórico de mensagens (`Vec<ChatMessage>`)
  não compartilhado com a sessão pai além do necessário para o resultado final — já é
  essencialmente assim em `subagent.rs::run()`, mas falta expor esse histórico para leitura pela UI
  em tempo real (hoje só o resultado final volta pra sessão pai).
- [ ] Emitir eventos de streaming da sub-sessão num canal próprio nomeado por `execution_id` (padrão
  já usado em `maybe_compact` para o resumo de compactação: canal sintético `{session_id}::compact`
  — replicar como `{session_id}::exec::{execution_id}`).
- [ ] Frontend: novo listener Tauri por `execution_id` quando o usuário abre a visualização daquela
  execução (não precisa manter todos os canais abertos o tempo todo — só quando o painel C está
  visível para aquela execução específica).

### Tarefa A3 — Definição de agente nomeado (baseado no `AgentFrontmatter` do picoClaw) [BE+FE]
- [ ] Formato de arquivo `AGENT.md` (ou reaproveitar `SKILL.md` com campos extras — decisão a
  validar: um agente é "uma skill com system prompt próprio + restrição de tools", então pode ser o
  mesmo arquivo com frontmatter estendido, evitando duplicar toda a infraestrutura de
  scan/load/scope que já existe em `skills.rs`).
- [ ] Campos: `name`, `description`, `system_prompt` (corpo do arquivo, como já é), `tools: Vec<String>`
  (allowlist; vazio = todas), `model: Option<String>` (override do modelo da sessão), `max_turns:
  Option<u32>`, `skills: Vec<String>` (skills que esse agente pode carregar).
- [ ] Reusar `global_agents_dir()`/`project_agents_dir()` no mesmo padrão de `skills.rs`
  (`{app_data}/agents/`, `<projeto>/.cerne/agents/`).
- [ ] Comandos Tauri espelhando os de skill: `list_agents`, `create_agent`, `read_agent`,
  `save_agent`, `delete_agent` (skills hoje não têm `delete_skill` — adicionar nos dois ao mesmo
  tempo, é um gap real hoje, ver Fase B backlog).
- [ ] `AgentEditorModal.vue` espelhando `SkillEditorModal.vue`, com campos extras para tools/model/
  max_turns (checkboxes de tools disponíveis, select de modelo).

### Tarefa A4 — Fila local vs. paralelo em API [BE]
- [ ] **Local (`llama.cpp`/Ollama/LM Studio)**: fila FIFO sem limite de itens enfileirados, 1
  execução ativa por vez — reaproveitar o padrão de mutex único já usado para o loop principal
  (mesma razão prática documentada na memória do usuário: "uma GPU só, uma coisa por vez"). Fila
  implementada como `VecDeque<QueuedExecution>` protegida por `Mutex`, worker único consumindo.
- [ ] **API (OpenRouter/provedores remotos)**: permitir `tokio::spawn` concorrente de várias
  execuções (`task`/skills), respeitando um limite configurável (ex. `max_parallel_api_executions`,
  default conservador tipo 3) para não estourar rate limit sozinho.
- [ ] Detectar automaticamente qual caminho usar a partir de `ProviderConfig.kind` (mesmo campo já
  usado em `apply_reasoning`, `providers/mod.rs:181`).
- [ ] **Aviso ao usuário**: antes de disparar execuções paralelas via API, mostrar aviso (uma vez
  por sessão ou com opção "não perguntar de novo") de que isso consome mais tokens e pode esbarrar
  em rate limit — via `ask` (Manual) ou toast informativo (Yolo/Auto).

### Tarefa A5 — YOLO vs Manual aplicado a agentes/skills (não só tools) [BE+FE]
- [ ] **YOLO**: agente decide livremente quais agentes/skills usar, sem perguntar — já é o
  comportamento natural de `ExecutionMode::Yolo` hoje, só formalizar que isso também cobre a
  decisão de chamar `task`/`load_skill`.
- [ ] **Manual**: antes de rodar `task`/`load_skill`, mostrar modal listando quais agentes/skills o
  LLM pretende usar (nome + descrição curta) — hoje o modo Manual já pausa em toda tool call via
  `request_permission` (`agent/mod.rs:645-660`); a diferença aqui é agrupar múltiplas
  chamadas de agente/skill previstas num mesmo turno num único modal, em vez de 1 popup por
  chamada — evita fadiga de clique quando o LLM planeja usar 3 skills em sequência.
- [ ] Novo evento `agent:agents_skills_plan` (Manual apenas) — emitido quando o modelo retorna tool
  calls que incluem `task`/`load_skill`, antes de executar qualquer uma; frontend mostra modal
  único "Vou usar: 🔍 Code Reviewer, 🧪 QA Tester — confirma?" com Aprovar tudo / Aprovar um a um /
  Cancelar.

### Tarefa A6 — Descrição concisa + "ler mais" [BE+FE]
- [ ] Backend: catálogo exposto ao LLM (`skills::list_skills` + futuro `list_agents`) já só manda
  nome+descrição (progressive disclosure já existe, ver `agent/mod.rs:309-372`) — confirmar que a
  `description` tem um teto de tamanho (adotar `MaxDescriptionLength` do picoClaw, ~200-300
  caracteres é mais realista para custo de contexto que os 1024 do picoClaw) para não deixar
  descrições longas demais no catálogo.
- [ ] Ferramenta explícita `read_skill_details(name)` / `read_agent_details(name)` para o LLM "ler
  mais" sob demanda quando a descrição curta não for suficiente — separado do `load_skill` (que já
  carrega o corpo pra *uso*; esta nova tool é só para o LLM decidir *se* vale usar, sem comprometer
  a chamada).
- [ ] Frontend (tela de catálogo, Fase B1): mesma ideia para o usuário — mostrar descrição curta na
  lista, com "ver mais" abrindo o `SKILL.md`/`AGENT.md` completo.

---

## Fase B: Catálogo de Agentes e Skills (tela)

### Tarefa B1 — Tela/painel "Agentes & Skills" [FE]
- [x] `AgentsSkillsPanel.vue` — CONCLUÍDO (2026-08-15): acessível via ícone "Agentes & Skills" na
  barra lateral (abre como modal, mesmo padrão da Fase F). Duas abas: Skills (global+projeto, via
  `list_skills` existente) e Personas (via `list_personas`) — ainda não é "Agentes" de verdade porque
  a A3 (agente nomeado com tools/model) não foi terminada, só Personas (prompt+nome) existem hoje.
- [x] Nome, escopo (global/projeto), descrição — CONCLUÍDO. **Badge "em uso agora" parcial**: só
  implementado pra Personas (compara com `session.persona_id`, sinal limpo e direto) — pra Skills
  não tem equivalente hoje, porque `load_skill` é uma leitura síncrona dentro do turno que não cria
  um `AgentExecution` rastreável (só `task`/`verify_completion` criam, e não carregam nome de skill).
  Documentado como gap no componente.
- [x] Busca por nome/descrição + abas Skills/Personas — CONCLUÍDO.
- [x] Ação "usar agora" — CONCLUÍDO, com abordagens diferentes por tipo: **Persona** tem efeito
  imediato (`updateSessionPersona`, sem precisar do LLM). **Skill** não pode ser forçada direto (é o
  LLM que decide chamar `load_skill` dentro do turno) — em vez disso, prepara uma mensagem no
  composer (`sessionStore.setDraft`) pedindo pra usar aquela skill, pro usuário revisar e enviar.

### Tarefa B2 — Reaproveitar skills do picoClaw [BE conteúdo, não código]
- [x] Avaliar e adaptar as skills do picoClaw que fazem sentido standalone — CONCLUÍDO (2026-08-16):
  `weather` e `skill-creator` adaptadas (removida dependência de `curl`/wttr.in do picoClaw — usam
  `web_search`/`web_fetch`, que o Cerne já tem embutido); `summarize` escrita do zero pro mesmo
  padrão (não existe skill equivalente direta no picoClaw). `github`/`tmux` ficaram de fora (baixa
  prioridade Windows-first); `hardware`/`agent-browser`/`picoclaw-agent` ficaram de fora (específicas
  do domínio do picoClaw, não aproveitáveis como estão).
- [x] Empacotar como skills de exemplo embarcadas — CONCLUÍDO, **reduzido**: em vez do loader de
  3 níveis (global/projeto/builtin somente-leitura) cogitado no roteiro original, implementei
  semeadura simples — `ensure_global_skills_dir` (`skills.rs`) grava as 3 skills de exemplo
  (`src-tauri/src/skills_examples/*.md`, via `include_str!`) na pasta global **só na primeira vez**
  que o app roda (mesmo gate do `_README.md`). Ficam editáveis/deletáveis como qualquer skill
  normal — não são somente-leitura nem tratadas como uma pasta especial "builtin". Mais simples de
  implementar e já fecha as duas tarefas pendentes da Fase 1 ("skills de exemplo embarcadas" e,
  indiretamente, "documentar formato SKILL.md" — a skill `skill-creator` já cobre isso dentro do
  próprio catálogo). O loader de 3 níveis de verdade (skills builtin somente-leitura, distintas de
  global/projeto) fica como possível iteração futura se um dia fizer falta.
- Novo teste `ensure_global_skills_dir_seeds_example_skills_once` (confirma seeding na 1ª vez e que
  não recria uma skill que o usuário deletou manualmente depois).

### Tarefa B3 — Usuário cria seus próprios agentes [FE]
- [x] **IMPLEMENTADO — 2026-08-16, reduzido**: em vez de um sistema paralelo `AgentEditorModal.vue` +
  `AGENT.md`/`agents.rs` novo do zero, reaproveitei a mesma decisão já tomada no T24 (Persona =
  versão pragmática de "agente nomeado", evita duplicar CRUD/UI pro usuário gerenciar dois conceitos
  quase idênticos). `Persona` ganhou `skills: Vec<String>` (allowlist de skills que a persona pode
  carregar via `load_skill`, `#[serde(default)]`, retrocompatível) — junto com `tools` (T48), fecha
  as duas peças do formulário que dava pra implementar com segurança. `PersonaEditorModal.vue` ganhou
  uma segunda grade de checkboxes com o catálogo de skills de verdade (`api.listSkills`), não uma
  lista curada fixa como a de tools.
- [ ] **`model` (override de modelo por agente) e `max_turns`**: continuam DELIBERADAMENTE fora de
  escopo, mesma decisão de risco já registrada no T48 — trocar o modelo no meio do fluxo de uma
  sessão existente pode mandar um modelo que não existe nesse provider ou incompatível com o
  `reasoning_effort`/context window já configurado, e isso quebraria silenciosamente sem o usuário
  poder testar ao vivo nesta rodada. Fica pra uma iteração com mais cautela/teste ao vivo.
- [x] `AgentEditorModal.vue`/preview de frontmatter — **não implementado como tal**: não existe um
  `AGENT.md` gerado (a persona continua sendo JSON em `personas.json`, mesmo formato desde o T24), e
  o formulário reaproveita o `PersonaEditorModal.vue` já existente em vez de um modal novo dedicado.

### Perguntas ao usuário (via `ask`)
```
Pergunta: "Encontrei estas skills do picoClaw que podem ser úteis: weather, summarize,
skill-creator. Quer importar alguma agora?"
Opções: [✅ Importar todas] [☑️ Escolher quais] [❌ Não agora]
```

---

## Fase C: Painel de Execução em Tempo Real

### Contexto atual
- Não existe painel dedicado a "tarefas em segundo plano" — jobs de `run_command(background=true)`
  aparecem como `TaskItem` comum dentro de `TaskStepGroup.vue`, sem distinção visual de "ainda
  rodando em background" vs. "comando síncrono já terminado".
- Leitura de background job é pull (`check_background_output`), sem push em tempo real.
- Sub-agente (`task`) não tem visualização própria — só aparece como uma tool call que demorou mais.

### Tarefa C1 — Expandir painel de tarefas em segundo plano [BE+FE]
- [x] **[BE]** Emitir evento push `agent:background_output` quando o buffer de um job muda — CONCLUÍDO
  (2026-08-14): `spawn_reader` ganhou `app: Option<AppHandle>` (via `BackgroundJobs::new`, chamado no
  `setup` do app; `None` nos testes) e emite `agent:background_output` a cada linha nova, rate-limitado
  a 1/200ms (`BACKGROUND_OUTPUT_EMIT_INTERVAL`) via um `Instant` compartilhado entre os readers de
  stdout/stderr do mesmo job. Também expus `list_background_jobs`/`stop_background_job` como comandos
  Tauri de verdade — antes só existiam como tool do LLM, o frontend não tinha NENHUMA visibilidade
  direta sobre jobs em background.
- [x] **[FE]** Painel expandido tipo Claude Code — CONCLUÍDO (2026-08-14): `BackgroundJobsPanel.vue`,
  badge "N processos (M rodando)" clicável que expande a lista, cada linha abre um `Dialog` com o
  output completo (atualiza ao vivo via `agent:background_output`), spinner enquanto roda.
- [x] **[FE]** Botão cancelar por job — CONCLUÍDO (2026-08-14): botão de parar por linha + no modal de
  detalhe, chamando `stop_background_job` (mesmo `stop()` do backend, já mata a árvore de processos via
  `taskkill /T /F`).
- **Não verificado**: `cargo test --lib` não rodou nesta sessão — o binário de teste falha ao iniciar
  (`STATUS_ENTRYPOINT_NOT_FOUND`) mesmo após `cargo clean` completo, aparentemente um problema de
  ambiente Windows (AV/DLL) nesta máquina, não do código (`cargo check --tests`, que type-checka o
  código de teste sem precisar rodar o binário, passou limpo). Vale rodar `cargo test` de novo depois,
  fora desta sessão, pra confirmar os 134 testes existentes + nenhuma regressão em `background.rs`.

### Tarefa C2 — Sessão de agente/skill visualizável [BE+FE]
- [x] **[BE]** Usar o `execution_id` e o canal `{session_id}::exec::{execution_id}` — CONCLUÍDO
  (2026-08-14): nenhuma mudança de backend nova foi necessária, a infra da A1/A2 já expunha tudo
  (`list_agent_executions`, `execution_id` em `agent:tool_call`/`agent:tool_result`, canal sintético
  já usado por `subagent::run`/`verifier::run`).
- [x] **[FE]** `AgentExecutionsPanel.vue` — CONCLUÍDO (2026-08-14): badge "N agentes/skills (M
  rodando)" acima do composer, expande numa lista (nome, ícone por tipo, status); clicar numa linha
  abre um modal somente leitura que assina `chat:token`/`chat:thinking_token` (filtrando pelo canal
  sintético) e `agent:tool_call`/`agent:tool_result` (filtrando por `execution_id`) — mostra o texto
  sendo gerado e os passos de ferramenta em tempo real, sem nenhum input do usuário.
- [ ] **[FE]** Botão "cancelar" por execução — **NÃO IMPLEMENTADO** (gap documentado): hoje `task`/
  `verify_completion` rodam dentro do mesmo turno da sessão pai, sem um handle de abort próprio —
  só existe cancelamento do turno inteiro (`cancel_turn`, botão de parar do composer). Cancelar uma
  execução individual exigiria dar a cada `task`/`verify_completion` seu próprio `JoinHandle`
  rastreado por `execution_id` (mudança de arquitetura similar em escopo à A4). O modal do painel
  deixa isso explícito num aviso ("cancelamento individual ainda não é suportado").
- [ ] **[FE]** Posição na fila enquanto `Queued` — **NÃO IMPLEMENTADO** (gap documentado): o registro
  `AgentExecution` marca status `running` assim que a execução começa (não existe um estado
  intermediário "na fila" hoje — a fila local é implícita, é só a ordem do loop sequencial de tool
  calls do turno, sem uma estrutura de dados própria de fila com posição rastreável).

### Perguntas ao usuário (via `ask`)
```
Pergunta (modo Manual, execução paralela via API): "Vou rodar 3 skills em paralelo via API —
isso usa mais tokens e pode esbarrar em rate limit. Continuar?"
Opções: [✅ Sim, paralelo] [🔁 Prefiro em fila (1 por vez)] [❌ Cancelar]
```

---

## Fase D: Repositório — Diff Completo + Navegador de Arquivos

### Contexto atual
- `DiffReview.vue` mostra diff **unified, por arquivo pendente individual** (aceitar/rejeitar 1
  edição por vez) — não existe visão agregada "todas as mudanças da sessão num só diff".
- `ExtraReadPaths.vue` é uma lista de pastas autorizadas (com toggle read/read-write), não um
  seletor de "pasta de trabalho" nem uma árvore de arquivos navegável.
- Não existe componente de file explorer em `src/components/`.

### Tarefa D1 — Visualizador de diff de repositório + seletor de pasta [FE, com apoio BE]
- [x] **[FE]** `RepoDiffViewer.vue` — CONCLUÍDO (2026-08-15): agrega todos os `TaskItem` de
  write_file/edit_file/ast_edit com diff (`sessionStore.tasks`, já carregado — não precisou de
  comando de agregação novo), agrupados por arquivo, lista à esquerda + diff colorido à direita.
  `splitDiffDetail`/`diffLines` foram extraídos de `TaskStepGroup.vue` pra `src/diffUtils.ts`
  (módulo compartilhado, evita duplicar o parser).
- [x] **[FE]** Seletor de pasta de trabalho — CONCLUÍDO: botão "Trocar pasta" no topo do
  `RepoDiffViewer.vue`, reaproveita o diálogo nativo (`@tauri-apps/plugin-dialog`), chama a nova
  action `sessionStore.updateProjectRoot`.
- [x] **[BE]** Comando `update_session_project_root` — CONCLUÍDO: não existia NENHUM jeito de trocar
  o `project_root` de uma sessão já criada (só dava pra definir na criação) — `sessions::update_project_root`
  + comando Tauri, mesmo padrão de `update_extra_read_paths`. O `diff_summary(session_id)` cogitado
  no roteiro não foi necessário — os dados já estavam disponíveis no frontend via `sessionStore.tasks`/
  `pendingEdits`.
- **Gap documentado**: o viewer não distingue "edição já aceita" de "rejeitada" no histórico — só
  sabe se ainda está PENDENTE (existe em `pendingEdits`) ou não; uma vez resolvida, não guarda qual
  dos dois destinos teve (mostra badge "pendente" só enquanto aplicável).

### Tarefa D2 — Navegador de arquivos clicável [FE, com apoio BE]
- [x] **[BE]** Comando `list_dir_entries(path)` — CONCLUÍDO (2026-08-15): não-recursivo de propósito
  (lista só 1 nível por vez, carregado sob demanda pelo frontend conforme expande — evita travar em
  pastas gigantes tipo `node_modules`). Sem restrição extra de `extra_read_paths` — mesmo espírito de
  "leitura livre" que a tool `list_dir` do agente já tem hoje (quem navega é o usuário, não o LLM).
  Esconde ocultos (`.git`, `.cerne`, etc.).
- [x] **[FE]** `FileBrowser.vue` + `FileTreeNode.vue` (nó recursivo, lazy-load ao expandir) —
  CONCLUÍDO. Clicar num arquivo, ou no ícone "inserir" de uma pasta, chama `sessionStore.setDraft`
  (mesmo mecanismo usado pelo painel B1) com o caminho absoluto e fecha o modal.
- [x] **[FE]** Botão 📁 no composer (`ComposerBar.vue`, ao lado de `ExtraReadPaths`) abre o navegador
  — CONCLUÍDO. Pasta inicial: `project_root` da sessão (ou primeira `extra_read_path`); botão "Trocar
  pasta" reaproveita o diálogo nativo já usado em `ExtraReadPaths.vue`.

---

## Fase E: Composer — Unificação e Novos Elementos

### Tarefa E1 — "Método Fable" é uma Skill? [investigação + decisão]
**Achado da investigação de código**: hoje o "Método Fable" **não é uma skill** — é um mecanismo
totalmente separado: `Session.fable_method: bool` (`models.rs:338-344`, default `false`), um botão
dedicado no composer com ícone `route` (`ComposerBar.vue:519-529`), e o texto embutido via
`include_str!("fable_method.md")` injetado no **system prompt inteiro** quando ligado
(`agent/mod.rs:89-94`). É um *toggle binário por sessão*, não um catálogo de itens que o LLM escolhe
sob demanda — funciona mais como o `Session.reasoning_effort` (uma configuração de sessão) do que
como uma skill (que é carregada seletivamente via `load_skill` quando o LLM decide usá-la).

**Recomendação**: **não forçar a unificação**. Fable e Skills resolvem problemas diferentes — Fable
é "mude o comportamento da sessão inteira", Skill é "ferramenta que o LLM escolhe pontualmente". Se
a unificação de UI for só visual (um mesmo local no composer para "ativar comportamentos
especiais"), faz sentido agrupar visualmente Fable + botão de abrir catálogo de skills/agentes (B1)
no mesmo menu/dropdown — mas manter os mecanismos de backend separados, já que convertê-lo em skill
tiraria a garantia de "sempre no system prompt desde o primeiro turno" que o Fable tem hoje.
- [x] **Encerrado — 2026-08-16, sem mudança de código**: task condicional a "se aprovado", sem
  aprovação explícita do usuário até agora — mantido como investigação/decisão fechada (não forçar
  unificação), Fable continua no lugar de sempre no composer.

### Tarefa E2 — Atalho `/` no composer [opinião + decisão]
**Minha opinião**: faz sentido, com ressalva. `/` como prefixo para abrir um menu de
skills/agentes/MCPs/ferramentas é um padrão já familiar (Slack, Discord, Claude Code, Notion) e
resolve bem a descoberta ("o que dá pra fazer aqui?") sem poluir a barra de botões do composer, que
já está carregada (`ProviderPicker`, `ExtraReadPaths`, anexo, `ExecutionMode`, `reasoning_effort`,
Fable, toggles de MCP, `ContextGauge`). A ressalva é **não sobrepor com o uso normal de texto** — o
usuário pode legitimamente querer digitar uma mensagem que começa com `/` (caminho de arquivo tipo
`/home/user/...` em sistemas Unix, ou só uma barra por acaso). Mitigação padrão do mercado: só abrir
o menu se `/` for o **primeiro caractere digitado** na caixa vazia, e fechar/ignorar assim que o
usuário digitar um espaço ou continuar sem selecionar uma opção do menu (deixa de ser um atalho e
vira texto normal).
- [x] **IMPLEMENTADO — 2026-08-16**: `/` no início do composer vazio abre popover filtrável com
  skills, personas, MCPs (com estado ligado/desligado) e ready prompts — cada um com ação própria ao
  selecionar (`ComposerBar.vue::selectSlashItem`). Fecha sozinho ao aparecer espaço/quebra de linha
  ou ao selecionar um item.
- [x] **IMPLEMENTADO**: navegação por teclado completa (setas cima/baixo trocam item ativo, Enter
  seleciona, Esc fecha sem mexer no texto) via `onKeydown`.

### Tarefa E3 — MCPs desabilitados por padrão + modal de seleção por sessão [BE+FE]
- [x] **IMPLEMENTADO — 2026-08-16**: `sessions::create_session` agora nasce com
  `enabled_mcp_servers: Some(vec![])` — só afeta sessão criada a partir de agora, sessões antigas
  continuam `None` (= todos habilitados), retrocompatível.
- [x] **IMPLEMENTADO**: botões individuais viraram um único botão "MCPs (N/M)" que abre `Dialog` com
  checkboxes (`ComposerBar.vue`).
- [x] **IMPLEMENTADO**: continua usando o mesmo mecanismo de `enabled_mcp_servers` por sessão, só a
  UI mudou.

### Tarefa E4 — Ícone de "olho" com cache de teste de visão [FE, backend já existe]
- [x] **Confirmado**: nenhuma mudança de backend precisou — `api.checkVisionSupport` já existia.
- [x] **IMPLEMENTADO — 2026-08-16**: ícone dedicado no `composer-toolbar` com 3 estados visuais
  (não testado/cinza, suporta imagem/verde, não suporta/vermelho).
- [x] **IMPLEMENTADO**: cache client-side em `localStorage`, chaveado por
  `provider::model::fork_ou_custom_provider_id` (não por sessão nem só por nome de modelo — inclui o
  fork/provider customizado porque o mesmo nome de modelo pode se comportar diferente entre dois
  endpoints locais).
- [x] **IMPLEMENTADO**: clique no ícone força reteste (`retestVisionSupport`, ignora o cache).

### Tarefa E5 — Botão de copiar em blocos de código [status: majoritariamente feito]
Ver `PLANOS/10_botao_copiar_codigo.md` — **já implementado e aprovado** (`MarkdownContent.vue`).
Falta só a parte de prompt engineering pedida agora:
- [x] **IMPLEMENTADO — 2026-08-16**: `SYSTEM_PROMPT` (`agent/mod.rs`) ganhou a instrução — um bloco
  de código por comando, exceto quando precisam rodar juntos em sequência de um único paste.

---

## Fase F: Modais de Ajuda/Configurações/Sobre

### Contexto atual
- **Settings** é uma **view inteira**, alternada em `App.vue` via `view = ref<"chat"|"settings">
  ("chat")` (linha 15) e `<ChatView v-if="view==='chat'"> / <Settings v-else>` (linhas 64-65) — sair
  do chat pra configurações troca a tela inteira, perdendo o que estiver visível no composer se
  algo não for persistido em rascunho.
- **Help** e **About** já são modais (`HelpModal.vue`/`AboutModal.vue`, `Dialog` do PrimeVue),
  controlados por refs `showHelp`/`showAbout` em `App.vue` — esses dois já seguem o padrão pedido,
  só falta migrar Settings.

### Tarefa F1 — Converter Settings de view para modal [FE]
- [ ] Trocar `view: Ref<"chat"|"settings">` por `showSettings: Ref<boolean>` no mesmo padrão de
  `showHelp`/`showAbout`.
- [ ] `Settings.vue` passa a ser renderizado como `<Dialog>` (PrimeVue), provavelmente maximizado
  ou quase tela-cheia dado o volume de conteúdo (abas existentes) — usar `maximizable` do PrimeVue
  `Dialog` em vez de `visible` simples, para não forçar redesenho de tudo em modal pequeno.
- [ ] Conferir que nenhuma navegação interna de `Settings.vue` dependia de ser uma rota/view própria
  (ex. deep-link direto pra aba de Skills) — se depender, replicar via prop `initialTab` no `Dialog`.

### Tarefa F2 — Garantir que nada quebra [FE]
- [ ] Testar fluxo: digitar algo no composer → abrir Configurações (agora modal) → fechar → texto
  do composer intacto (esse é o motivo principal da mudança pedida).
- [ ] Testar abertura de `SkillEditorModal.vue` **dentro** do modal de Settings (modal sobre modal)
  — PrimeVue suporta, mas conferir z-index/foco/Esc não fecha os dois de uma vez.
- [ ] Conferir que o `Sidebar.vue` (`@open-help`, `@open-about`) ganha também `@open-settings` se
  ainda não usar esse padrão de evento.

---

## Checklist de Implementação Progressiva (Parte II)

```
Fase A ☐ Infraestrutura de Execução
  └─ ☑ A1 UUID de rastreamento (execution_id / parent_execution_id) — CONCLUÍDO (2026-08-14): `models::AgentExecution` + `AppState.agent_executions` (registro em memória), `execution_id` gerado em `task`/`verify_completion`, propagado nos eventos `agent:tool_call`/`agent:tool_result`, comando `list_agent_executions`. `parent_id` sempre `None` na prática hoje (guarda de profundidade estrutural impede aninhamento) — campo já pronto pra quando isso mudar. Ainda falta consumir isso no frontend (UI de catálogo é Fase B)
  └─ 🔶 A2 Sessão isolada por execução + canal de streaming próprio — PARCIAL (2026-08-14): `agent:tool_call`/`agent:tool_result` já entregam IN/OUT completos ao vivo por tool call (T25), e o canal sintético `{session_id}::exec::{execution_id}` já existe e corrige um vazamento real (T28: texto do sub-agente/verificador estava vazando pro streaming do chat principal). Falta só o consumidor: um listener/painel de UI que abra esse canal quando o usuário quiser acompanhar uma execução (isso é Fase C)
  └─ 🔶 A3 Definição de agente nomeado (AGENT.md / frontmatter estendido) — PARCIAL, avançado
     (2026-08-16): Personas (T24) cobrem "prompt pronto selecionável injetado no system prompt";
     agora também têm `tools: Vec<String>` (T48) — allowlist opcional de ferramentas, filtra o
     toolset da sessão inteira quando preenchida (`ask` sempre mantido). Ainda falta `model`
     (override de modelo por persona — avaliado e adiado por risco, ver T48) e `max_turns`/`skills`
     associadas — o conceito continua sendo "Persona" (system prompt + allowlist), não um
     `AGENT.md` de verdade com múltiplos agentes nomeados coexistindo
  └─ ☑ A4 Fila local (sequencial, sem limite) vs. paralelo API (com limite + aviso) — CONCLUÍDO (2026-08-14): local nunca precisou de código extra (o turno já roda numa cadeia sequencial de awaits por construção — "fila de 1" de graça). Quando 2+ chamadas de `task` caem no mesmo turno E o provider não é local, elas rodam em paralelo de verdade via `futures_util::future::join_all` (mesma tokio task, sem `tokio::spawn`) — resultado pré-computado antes do loop sequencial, que só consome. Aviso: no modo Manual entra como nota no modal batelado da A5 (`AgentsSkillsPlan.parallel`); em Auto/YOLO emite `agent:parallel_execution_info` (sem toast consumindo ainda, ver `14_backlog_pendente.md`). Sem limite configurável de paralelismo (`max_parallel_api_executions`) nesta primeira versão — todas as `task` elegíveis do turno rodam juntas; se algum dia um turno pedir dezenas de sub-agentes de uma vez, adicionar um teto vira tarefa futura
  └─ 🔶 A5 Modal de confirmação YOLO/Manual para agentes/skills (não só tools) — REDUZIDO/CONCLUÍDO (2026-08-14): modo Manual agora pergunta uma vez só, batelado (evento `agent:agents_skills_plan` + `AgentsSkillsPlanCard.vue`), antes de rodar `task`/`load_skill`/`verify_completion` do turno — sem repetir popup individual pra cada um. Reduzido em relação ao desenho original: só "Aprovar todos"/"Recusar todos", sem "aprovar um a um" (ver nota no código, `agent/mod.rs::request_agents_skills_plan`) — YOLO já não pergunta nada (comportamento existente, sem mudança)
  └─ ☑ A6 Descrição concisa + "ler mais" (tool read_skill_details/read_agent_details) — CONCLUÍDO (2026-08-14): catálogo de skills no system prompt agora corta descrição em `SKILL_CATALOG_DESC_MAX_CHARS` (200 chars), com aviso pro LLM chamar a nova tool `read_skill_details(name)` quando precisar da descrição inteira antes de decidir. `read_agent_details` fica pra quando existir agente nomeado de verdade (A3)

Fase B ☐ Catálogo de Agentes/Skills
  └─ ☐ B1 Painel de listagem/seleção/uso atual
  └─ ☐ B2 Importar skills do picoClaw (weather, summarize, skill-creator prioritárias)
  └─ ☐ B3 Editor de agente próprio (AgentEditorModal.vue)

Fase C ☐ Painel de Execução em Tempo Real
  └─ ☐ C1 Painel de background tasks expandido + evento push
  └─ ☐ C2 Visualização read-only + cancelamento de execução de agente/skill

Fase D ☐ Diff de Repositório + Navegador de Arquivos
  └─ ☐ D1 RepoDiffViewer.vue + seletor de pasta de trabalho
  └─ ☐ D2 FileBrowser.vue clicável → insere caminho no composer

Fase E ☐ Composer
  └─ ☐ E1 Fable: decisão tomada (manter separado, unificar só visualmente)
  └─ ☐ E2 Atalho "/": decisão tomada (implementar com mitigação de conflito de texto)
  └─ ☐ E3 Botão único "MCPs" + modal de checkboxes, default desabilitado
  └─ ☐ E4 Ícone de olho com cache por modelo
  └─ ☐ E5 Instrução de "um bloco de código por comando" no system prompt

Fase F ☐ Modais de Ajuda/Configurações/Sobre
  └─ ☑ F1 Settings vira Dialog maximizável — CONCLUÍDO (2026-08-14): `App.vue` não alterna mais `view: "chat"|"settings"` (removido); `Sidebar.vue` perdeu a prop `view`, botão de Configurações agora emite `open-settings` igual Ajuda/Sobre; `Settings.vue` ganhou `visible`/`update:visible` e o template inteiro entrou num `<Dialog modal maximizable>` do PrimeVue (mesmo padrão de `HelpModal.vue`/`AboutModal.vue`)
  └─ ☑ F2 Testes de regressão (rascunho do composer, modal-sobre-modal) — CONCLUÍDO (2026-08-14): confirmado pelo usuário na janela real (`npm run tauri dev`) — modal de Configurações abre por cima sem trocar de tela, rascunho do composer sobrevive ao abrir/fechar, `SkillEditorModal`/`PersonaEditorModal` abrem corretamente por cima do modal de Settings (modal-sobre-modal), e o botão de maximizar funciona
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
| picoClaw (Sipeed) | Framework de agente Go para hardware barato — skills `SKILL.md`, sub-turnos, MCP nativo | https://github.com/sipeed/picoclaw |

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

# Fase G: Sessões Paralelas Orquestradas

> Pedido pelo usuário em 2026-08-16. Ideia central: o LLM principal vira um **orquestrador** que
> cria SESSÕES completas e independentes (não sub-agentes efêmeros como `task`) pra fazerem
> trabalho em paralelo, rastreadas por UUID, e confere o progresso delas sob demanda — sem bloquear
> o próprio turno esperando. Exemplo dado pelo usuário: uma sessão monta o frontend Angular, outra
> o backend Spring REST completo, uma terceira confere se teve erro, uma quarta confere a
> integração entre elas.

> **✅ IMPLEMENTADA — 2026-08-16** (G1/G2/G3 completos, G4 reduzido — ver detalhe abaixo). Feita
> sem supervisão ao vivo (usuário foi dormir, pediu pra continuar até o fim) — `cargo check --lib`/
> `--tests` (172 testes, incluindo os novos desta fase) e `npx vue-tsc --noEmit` limpos, mas
> **não testada na janela real**. Recomendo fortemente testar isto com cuidado extra antes de
> confiar em produção: é a feature mais arriscada implementada nesta sessão (sessão chamando
> `run_turn` recursivamente, spawn desacoplado, timing em memória).
>
> **Detalhe da implementação**:
> - **G1**: 4 tools novas (`orchestration_tool_specs()` em `agent/tools.rs`) — `start_agent_session`
>   (cria `Session` de verdade via `sessions::create_session`, marca `parent_session_id` via novo
>   `sessions::update_parent_session_id`, herda `execution_mode` do pai, dispara o turno
>   desacoplado), `check_agent_session` (olha `state.running_turns`; rodando → devolve dica de
>   espera, terminado → devolve a última mensagem `assistant` via `sessions::load_messages`),
>   `list_agent_sessions` (filtra sessões com `parent_session_id == session_id atual`),
>   `stop_agent_session` (`JoinHandle::abort`, mesmo mecanismo de `cancel_turn`). Guarda de
>   profundidade: `orchestration_tool_specs()` só entra no toolset quando
>   `session.parent_session_id.is_none()` (`agent/mod.rs`, montagem do toolset). `check_agent_session`/
>   `list_agent_sessions` entram em `DOOM_LOOP_EXEMPT_TOOLS` desde o início, como planejado.
>   - **Achado real durante a implementação**: `run_turn` chamando a si mesma diretamente (dentro do
>     próprio corpo da função, pra disparar a sessão filha) trava o rustc numa checagem de `Send`
>     cíclica ("future cannot be sent between threads safely") — `Box::pin` no ponto de recursão
>     NÃO resolve (tentado, ainda falhou). Solução: extrair o `spawn`+`.await` de `run_turn` pra uma
>     função comum separada, não-`async fn` (`spawn_orchestrated_turn` em `agent/mod.rs`) — quebra o
>     ciclo porque a dependência vira de mão única, mesmo formato que `send_message` (`lib.rs`) já
>     usa com sucesso pra disparar `run_turn` sem essa recursão.
> - **G2**: `AppState.orchestrated_sessions: Mutex<HashMap<String, OrchestratedSessionInfo>>`
>   (`started_at_ms`/`first_response_ms`) — `first_response_ms` é a duração do PRIMEIRO turno
>   inteiro (não o instante exato da 1ª mensagem assistant dentro dele, que exigiria instrumentar
>   `run_turn` por dentro) — aproximação deliberada, já que só existe um turno rodando logo após a
>   criação. `check_agent_session` sugere `first_response_ms * 1.2` (ou 12s default se ainda não
>   tem medição). `SYSTEM_PROMPT` reforça pra seguir a sugestão em vez de checar em loop apertado.
> - **G3**: nada de código novo — local (llama.cpp) já serializa via `ensure_llama_ready` existente,
>   API já roda em paralelo de verdade (cada `start_agent_session` é um `tauri::async_runtime::spawn`
>   independente, mesmo padrão que a Fase A4 já validou). Confirma o que o roteiro já previa.
> - **G4 (reduzido)**: sessão orquestrada aparece na lista normal da sidebar sem trabalho extra
>   (é uma `Session` de verdade) — evento novo `agent:session_created` (emitido pelo backend ao
>   criar, já que não passa pelo fluxo normal de criação) insere na lista local do
>   `stores/session.ts` sem precisar recarregar tudo. `SidebarSessionItem.vue` ganhou um badge
>   pequeno (ícone `account_tree`) com tooltip "Criada por: {título do pai}" quando
>   `session.parent_session_id` existe. **NÃO implementado** (escopo reduzido, ver perguntas em
>   aberto do roteiro): agrupar sob a sessão pai tipo árvore (ligar com T29 quando fizer sentido) e
>   integração com `AgentExecutionsPanel.vue` (listar sessões orquestradas ativas com botão "abrir")
>   — ambos marcados como "opcional"/"avaliar" no desenho original, ficam pra depois se o usuário
>   sentir falta.
>
> **G4.1 — árvore expansível na sidebar, pedido pelo usuário testando ao vivo em 2026-08-17
> (aprovado o resto da Fase G: "ficou muito legal")**: hoje as sessões filhas (criadas via
> `start_agent_session`) ficam soltas na lista normal, misturadas com as sessões do usuário — só o
> badge/tooltip diferencia. Pedido: a sessão que CRIOU filhas ganha um ícone próprio na linha dela
> (diferente do badge que já existe nas filhas); clicar nesse ícone expande, mostrando as sessões
> filhas indentadas logo abaixo, como se fossem subpastas — e se uma filha também gerou outras
> (Fase G permite 1 nível de recursão hoje, mas o usuário quer a árvore preparada pra qualquer
> profundidade caso isso mude), as netas aparecem indentadas mais um nível ainda, embaixo da mãe
> delas, formando uma árvore de verdade. Clicar no ícone de novo recolhe tudo. Enquanto colapsada,
> as sessões filhas NÃO aparecem soltas na lista principal (hoje aparecem sempre, achatadas).
> **✅ IMPLEMENTADO — 2026-08-17** (adiado inicialmente, retomado no mesmo dia a pedido do
> usuário — "pode implementar na ordem de importância que você achar relevante"). Novo componente
> `SidebarSessionTree.vue` (auto-recursivo via nome do arquivo, o próprio Vue SFC já suporta um
> componente se referenciar dentro do próprio template) resolve a profundidade arbitrária sem
> reaproveitar o T29 (que é fixo em 2 níveis) — cada nó calcula os próprios filhos
> (`sessions.filter(s => s.parent_session_id === node.id)`) e se renderiza de novo pra cada um
> quando expandido, então netos/bisnetos funcionam sem código extra. Estado de expandido/recolhido
> por sessão vive em `Sidebar.vue` (`expandedSessions`, mesmo padrão de `Set` + localStorage já
> usado pras pastas — `isSessionExpanded`/`toggleSessionExpanded`), passado pra árvore via prop.
> Sessão com `parent_session_id` nunca aparece mais solta na lista/pasta dela mesma
> (`sessionsIn`/`looseSessions` agora filtram via `isNestedChild`) — só fica visível quando a
> sessão-mãe é expandida, EXCETO se a mãe foi excluída (órfã volta a aparecer solta, senão sumiria
> da sidebar sem jeito de abrir). Modo de busca continua achatado de propósito (não teria sentido
> esconder um resultado de busca atrás de um toggle). **✅ Testado na janela real — 2026-08-18**:
> usuário confirmou ("Testado, pode dar como concluido").
> - **Perguntas em aberto do roteiro, resolvidas na implementação**: aviso ativo ao terminar → NÃO
>   implementado, ficou só polling (`check_agent_session`), exatamente a recomendação já registrada
>   aqui embaixo. Limite de sessões orquestradas simultâneas → não implementado, sem teto. `project_root`
>   da sessão orquestrada → qualquer pasta que o LLM passar em `project_root` no `start_agent_session`,
>   sem restringir a subpasta do projeto pai (default herda a do pai se omitido).
> - **Testes novos**: `sessions.rs` (`update_parent_session_id_persists_and_roundtrips`,
>   `new_sessions_have_no_parent_session_id_by_default`,
>   `old_session_without_parent_session_id_field_still_deserializes`), `agent/tools.rs`
>   (`orchestration_tool_specs_has_the_four_session_tools`). Sem teste de integração pra
>   `start_agent_session`/`check_agent_session` em si (precisaria de um `AppHandle`/app Tauri de
>   verdade rodando, difícil de montar em unit test — mesma limitação já documentada pra
>   `background.rs` no T14).

## Por que isso é diferente da `task` que já existe

| | `task` (já existe, Fase A/B do roteiro) | Sessão orquestrada (esta fase, nova) |
|---|---|---|
| Bloqueia o turno que chamou? | Sim — mesmo com A4 (paralelo), o turno espera TODAS as `task`s do turno terminarem antes de continuar | **Não** — o orquestrador dispara e segue (responde ao usuário, cria outra sessão, faz outra coisa), só confere depois |
| Tem histórico próprio persistido? | Não — conversa efêmera, só existe na memória da chamada (`agent/subagent.rs`, doc do módulo) | **Sim** — é uma `Session` de verdade (`sessions.rs`), aparece na lista de sessões, pode ser reaberta e vista ao vivo pela UI normal |
| Pode ter provider/modelo diferente do pai? | Não — reusa sempre `cfg`/`session.model` do turno pai | **Sim** — pode escolher provider/modelo próprio (ex: uma sessão local, outra via API) |
| Visível na UI hoje? | Via `AgentExecutionsPanel.vue` (Fase C2), somente leitura, efêmero | Via a **lista de sessões normal** (sidebar) — é uma sessão de verdade, não precisa de painel especial |
| Guarda de profundidade | Sub-agente não tem a tool `task` (não pode recursar) | Mesma ideia: sessão orquestrada não ganha a tool de criar OUTRA sessão orquestrada (nível único) |

## Tarefa G1 — Backend: criar e rastrear sessões orquestradas

- [ ] **[BE]** Duas ferramentas novas pro agente principal (fora do toolset de sessões
  orquestradas, pra manter o nível único de recursão):
  - `start_agent_session({ description, prompt, project_root?, provider?, model? })` → cria uma
    `Session` de verdade (reaproveita `sessions::create_session`, mesmo caminho que `create_session`
    do `lib.rs` já usa), manda `prompt` como primeira mensagem, e dispara `agent::run_turn` **sem
    esperar** — mesmo padrão exato que `send_message` (`lib.rs:884-932`) já usa pro chat normal:
    `tauri::async_runtime::spawn`, handle guardado em `state.running_turns` (já existe, usado hoje
    só por `cancel_turn`). Devolve `{ "session_id": "<uuid>" }` na hora — o UUID já É o id da sessão
    (`Session.id`, já é UUID — não precisa inventar um `execution_id` novo aqui, diferente de
    `task`/`verify_completion` na Fase A1).
  - `check_agent_session({ session_id })` → olha se `session_id` ainda está em
    `state.running_turns` (rodando) ou não (terminou); se terminou, devolve a última mensagem do
    assistente (`sessions::load_messages`, pega a última `role: "assistant"`); se ainda rodando,
    devolve status + uma dica de quanto esperar (ver G2 abaixo).
  - `list_agent_sessions()` (opcional, mas ajuda o orquestrador não perder o fio) → lista as sessões
    cujo `parent_session_id` é a sessão atual, com status de cada uma.
  - `stop_agent_session({ session_id })` (opcional) → aborta, mesmo mecanismo do `cancel_turn`
    (`lib.rs:934-...`, `JoinHandle::abort`).
- [ ] **[BE]** `models::Session` ganha `parent_session_id: Option<String>` (`#[serde(default)]`,
  retrocompatível) — marca "essa sessão foi criada por outra sessão orquestrando", pra UI conseguir
  mostrar de onde veio e pro `list_agent_sessions` filtrar.
- [ ] **[BE]** Guarda de profundidade: toolset de uma sessão orquestrada = toolset normal (todas as
  ferramentas de projeto, incluindo `task`/`verify_completion` — ela é uma sessão completa, não um
  sub-agente capado) **menos** `start_agent_session`/`check_agent_session`/`list_agent_sessions` —
  não pode orquestrar recursivamente. Mesmo espírito do guard que já existe pra `task` em
  `subagent_tool_specs()` (`agent/subagent.rs`).
- [ ] **[BE]** Registrar `check_background_output`-like: `check_agent_session`/`list_agent_sessions`
  entram em `DOOM_LOOP_EXEMPT_TOOLS` (`agent/mod.rs`, criado no T39/2026-08-15) desde o início — é
  esperado o orquestrador chamar `check_agent_session` várias vezes com o mesmo `session_id`
  enquanto espera, isso não é loop travado.

## Tarefa G2 — Espera adaptativa baseada na 1ª resposta

Pedido específico do usuário: o tempo de espera entre uma checagem e outra deve se basear em quanto
tempo a PRIMEIRA resposta daquela sessão orquestrada levou — se levou 30s, o orquestrador deveria
esperar em torno de 30s (+ uma folga) antes de checar de novo, não ficar checando a cada segundo.

- [ ] **[BE]** Rastrear, por sessão orquestrada (registro em memória, `AppState`, mesmo espírito do
  `agent_executions` da Fase A1): `started_at_ms` e `first_response_at_ms` (timestamp de quando a
  primeira mensagem do assistente foi salva — dá pra pegar isso de `run_turn`, no primeiro
  `sessions::save_messages` com uma mensagem `assistant`).
- [ ] **[BE]** `check_agent_session` devolve, junto do status, uma dica textual calculada:
  - Se `first_response_at_ms` já existe: sugestão de espera ≈ `(first_response_at_ms - started_at_ms) * 1.2`
    (30s de exemplo do usuário vira ~36s de sugestão).
  - Se ainda não existe (primeira resposta não chegou nem na sessão orquestrada ainda): usar um
    default conservador (ex: 10-15s) até ter um número real medido.
  - Isso é só uma DICA na observação textual da ferramenta (ex: "ainda rodando, primeira resposta
    desta sessão levou 28s — sugiro checar de novo em uns 35s") — o Cerne não tem como forçar o
    modelo a "dormir" entre chamadas de ferramenta, quem decide o ritmo de polling é o próprio LLM.
    Reforçar essa orientação no system prompt (junto de onde já explica `task`/`check_background_output`
    hoje, `agent/mod.rs::SYSTEM_PROMPT`).

## Tarefa G3 — Concorrência: local em fila, API em paralelo

Já é exatamente o modelo que a Fase A4 (`task`s em paralelo) implementou — reaproveitar o mesmo
raciocínio, não inventar um novo:
- [ ] **[BE]** Se `provider` da sessão orquestrada for local (llama.cpp/Ollama/LM Studio),
  `ensure_llama_ready` já serializa por natureza (só um fork sobe por vez, `lib.rs::ensure_llama_ready`
  já mata qualquer outro fork rodando antes de subir um novo — ver comentário "Only one local
  llama.cpp fork can realistically run at a time"). Ou seja: **2 sessões orquestradas locais ao
  mesmo tempo já vão brigar pelo mesmo processo/porta hoje** — precisa decidir se isso vira uma fila
  de verdade (só a próxima sessão local começa quando a anterior — local — terminar) ou se só
  documentar a limitação e deixar o usuário evitar múltiplas sessões locais simultâneas na prática.
  Bate com a memória do usuário ("uma GPU só, uma coisa por vez").
- [ ] **[BE]** Se `provider` for de API, várias sessões orquestradas rodam de verdade em paralelo
  (cada uma já é um `tauri::async_runtime::spawn` independente) — mesmo aviso de custo/rate-limit
  que a Fase A4 já implementou pro modo Manual (`AgentsSkillsPlanCard.vue`) vale aqui também.

## Tarefa G4 — Frontend: visibilidade de sessões orquestradas

- [ ] **[FE]** Sessão orquestrada já aparece na lista normal da sidebar (é uma `Session` de
  verdade) — sem trabalho extra pra "ver ao vivo", o usuário só clica nela como qualquer sessão e
  acompanha o streaming normal. O que falta é só **deixar claro que ela foi criada por outra
  sessão**: ícone/badge pequeno + tooltip "criada por: <nome da sessão pai>" (usa o
  `parent_session_id` novo).
- [ ] **[FE]** Avaliar se faz sentido AGRUPAR sessões orquestradas sob a sessão pai na sidebar, tipo
  uma árvore recolhível — reaproveitando a ideia (e talvez até o componente) do **T29** (pastas na
  lista de sessões, também pendente, também 2 níveis) em vez de inventar uma segunda forma de
  agrupar sessão na sidebar. Ligar isso com o T29 quando for implementar os dois.
  - Existe ainda a Fase C2, o painel de execuções de agente/skill.
- [ ] **[FE]** Opcional: `AgentExecutionsPanel.vue` (Fase C2) também lista sessões orquestradas
  ativas, com botão "Abrir sessão" (troca a sessão atual pra ela) em vez do modal somente-leitura
  que usa pra `task`/`verify_completion` — já que aqui dá pra ver o de verdade ao vivo, não precisa
  duplicar a experiência.

## Perguntas em aberto (decidir antes de implementar)

- Quando uma sessão orquestrada termina, o pai **precisa** ser avisado ativamente (evento/mensagem
  injetada automaticamente no próximo turno do pai, como o **T14** já cogitado pra background jobs),
  ou fica 100% no modelo de polling (o próprio orquestrador decide quando chamar
  `check_agent_session`)? O pedido do usuário ("ao terminar uma sessão, retorna para o LLM
  principal") soa mais como aviso ativo — mas isso precisaria injetar mensagem numa sessão que pode
  já estar processando outro turno, o que é mais complexo. Recomendo começar só com polling (mais
  simples, reaproveita tudo que já existe) e avaliar aviso ativo como uma v2.
- Limite de quantas sessões orquestradas uma sessão pai pode ter abertas ao mesmo tempo?
- `project_root` da sessão orquestrada: sempre uma SUBPASTA do projeto pai (ex: `frontend/`,
  `backend/`), ou pode ser qualquer pasta que o usuário/LLM escolher? Isso muda como a sandbox de
  edição se comporta pra cada uma.

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

## Checklist de Implementação Progressiva (Parte I)

> Replanejado em 2026-08-16 com base no que já existe de verdade no código (ver "Replanejamento
> granular" dentro de cada fase acima) — nada abaixo foi implementado ainda nesta rodada, só
> detalhado/planejado. Vários itens da visão original já são consequência de infraestrutura que a
> Parte II construiu neste meio-tempo (Personas com allowlist de tools/skills, `task` paralelo,
> `AgentExecutionsPanel`, T17), então a lista ficou mais curta do que o esboço original.

```
Fase 1 ☑ Skills básicas (já feito)
  └─ ☑ Skills de exemplo embarcadas (Fase B2/T47, 2026-08-16)
  └─ ☐ Documentar formato SKILL.md na ajuda (skill-creator já cobre parcialmente)
  └─ ☐ Importar skill de URL → virou Fase 6, Degrau 1

Fase 2 ☑ Prompts prontos como agentes leves (mecanismo já existe, é a Persona)
  └─ ☑ system_prompt_override → Persona.content (T24)
  └─ ☑ Filtro de ferramentas → Persona.tools (T48)
  └─ ☑ Filtro de skills → Persona.skills (B3)
  └─ ☐ Documentar (não semear) as 5 personas de exemplo na ajuda

Fase 3 ☑ Pipeline Dev → QA → Analista (implementado — 2026-08-16)
  └─ ☑ Etapa DEV → reaproveita subagent::run como está
  └─ ☑ Etapa QA → reaproveita verifier::run como está
  └─ ☑ Etapa ANALISTA → novo agent/analyst.rs (cópia estrutural do verifier)
  └─ ☑ agent/pipeline.rs → laço determinístico dev→qa→analista, max_rounds
  └─ ☑ Tool run_pipeline + guarda de profundidade (sem chamar pipeline dentro de pipeline)
  └─ ☑ AgentExecution.parent_id usado pela 1ª vez + evento agent:pipeline_status
  └─ ☑ Gatilho: item novo no menu `/` (Fase E2)

Fase 4 ☑ Skills de produtividade (implementada — 2026-08-16, maioria era conteúdo, não código)
  └─ ☑ file_organizer (skill, seedada) — undo via create_python_tool (T17), não mecanismo próprio
  └─ ☑ email_triage (skill, seedada) — depende de MCP de e-mail do usuário, Cerne não implementa cliente
  └─ ☑ english_tutor — documentado como Persona (não seedado), nova seção "## Personas" na ajuda

Fase 5 ☑ Multi-agente gerenciado (implementada — 2026-08-16)
  └─ ☑ Catálogo de skills no prompt (A6), task paralelo (A4), rastreamento (C2) — nada novo
  └─ ☑ Único item real: skill "project-manager" ensinando o padrão de delegação
  └─ ☐ Caminho futuro: delegar via Fase G (sessões visíveis) em vez de task, se G for implementada

Fase 6 ☑ Skill store / comunidade (Degrau 1 implementado — 2026-08-16)
  └─ ☑ Degrau 1: botão "Importar skill de URL" no AgentsSkillsPanel, zero infra nova
  └─ ☐ Degrau 2 (se houver demanda): index.json curado, ainda sem contas/hospedagem própria
  └─ ☐ Degrau 3 (não recomendado por ora): registry completo com contas/review — fora de escopo
```
