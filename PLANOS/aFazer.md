# A Fazer — Cerne Code

> Coisas que você pediu mas que AINDA NÃO foram implementadas (nem código escrito, ou escrito mas
> comprovadamente não funciona ainda). Assim que uma vira código de verdade, sai daqui e entra em
> `PLANOS/Testar.md`, esperando você confirmar na janela real; depois de confirmada, vai pra
> `PLANOS/Feito.md`. Detalhe técnico completo (decisões, reduções de escopo, desenhos já prontos
> pra implementar) continua em `PLANOS/14_backlog_pendente.md` e
> `PLANOS/13_roteiro_agentes_skills_fases.md`.

---

> Numeração fixa — ao pedir pra trabalhar em algo desta lista, uso o número do item. Quando um item
> sai daqui, os demais são renumerados.

## Bug aberto (fix anterior não resolveu)

**1.** **Modal "ver ferramentas" de MCP ainda falha** (2026-08-17): mesmo depois de reaproveitar a
conexão já viva do pool quando o servidor está habilitado (em vez de abrir uma segunda conexão
paralela), o usuário reportou que continua dando erro. A correção feita (`McpClients::list_tools`,
`list_mcp_server_tools` em `lib.rs`) parte do princípio de que o servidor JÁ está conectado no
pool — se `state.mcp_clients.list_tools()` devolver `None` (ex: servidor conectou mas
`list_all_tools()` falhou depois, ou nunca chegou a conectar de verdade apesar de aparecer
habilitado), cai no fallback antigo (`test_connection`, conexão nova) que é exatamente o caminho
que já falhava antes. Precisa investigar com o erro exato que aparece agora (pode ser diferente do
anterior) — se ainda for "connection closed: initialize response", o problema pode ser mais
estrutural no próprio `test_connection`/`build_command` com esse servidor específico
(`test-everything`), não só a duplicação de conexão.

## Ideias registradas, nada de código ainda

Detalhe completo de cada uma em `PLANOS/14_backlog_pendente.md`, seção "Ideias futuras".

**2.** **Estudo de viabilidade mobile** — ESTUDO antes de qualquer código: o que funciona em
mobile (chat, Persona) vs o que é estruturalmente inviável (ferramentas que rodam processo/MCP
via stdio/`computer_use`), se o projeto sequer builda pra Android/iOS hoje.

**3.** **Modelo de visão separado (fallback)** — quando o modelo principal da sessão não vê imagem,
rotear a imagem + pergunta pra um modelo de visão dedicado (local ou API, escolhido pelo usuário)
e devolver a descrição em texto pro modelo principal continuar.

**4.** **Modo "conversa" em tempo real (voz↔voz)** — único item restante da lista original de "Voz
completa" (os outros — ler texto, microfone, escolha de provider/modelo de TTS/STT, detecção de
idioma, e o redesenho do "+" — já foram implementados e confirmados). **Precisa de pesquisa antes
de desenhar**: existe API nativa de voz-a-voz de baixa latência, ou precisa encadear STT→LLM→TTS
na mão?

**5.** **Sub-agentes isolados por git worktree** (só este item Hermes ficou de fora — os outros 4
foram implementados): pedido "faz todas as 5", mas decidi adiar este de propósito — mexe direto
no repositório git REAL do usuário (criar/limpar worktree e branch por sessão orquestrada), e
errar a limpeza deixaria o repositório dele sujo sem ele poder revisar enquanto dormia. Desenho
técnico completo já registrado em `14_backlog_pendente.md`, pronto pra implementar numa sessão em
que dê pra testar ao vivo.

**6.** **Escolher pasta de dados do Cerne Code (`app_data_dir`)** — pedido do usuário (2026-08-18):
hoje o `app_data_dir` é fixo (pasta padrão do SO), e às vezes o usuário tem um disco maior onde
não precisa se preocupar com espaço. Em Configurações, permitir escolher outra pasta pra onde
tudo (sessões, skills, personas, config, chaves) passa a ficar. Ao mudar, perguntar se pode mover
os arquivos já existentes pra lá — se sim, mover e só então trocar o caminho ativo (nunca trocar o
caminho ativo antes de confirmar que a cópia/move terminou com sucesso, senão o app aponta pra
uma pasta vazia). Pontos a decidir na implementação: onde fica guardado QUAL é o caminho atual
(precisa ser um lugar fixo e conhecido, tipo um arquivo pequeno ao lado do executável ou uma
chave de registro do Windows, já que não dá pra guardar "onde estão as configs" DENTRO das
configs); o que fazer se a pasta de destino já tiver arquivos (conflito); e se o app precisa
reiniciar depois da troca pra recarregar tudo do novo caminho.

**7.** **Unificar rodapé da sidebar num só "Configurações" com navegação lateral** — pedido do
usuário (2026-08-18, com prints do menu de Configurações do Claude como referência de layout):
hoje o rodapé da sidebar tem 4 itens separados (Agentes & Skills, Ajuda, Configurações, Sobre),
cada um abrindo seu próprio modal. Substituir por UM item só ("Configurações", ou outro nome se
preferir) que abre um modal maior com navegação em duas colunas — lista à esquerda ("Configurações"
/ "Agentes, Skills & Persona" / "Ajuda" / "Sobre"), conteúdo da opção selecionada à direita. Abre
sempre com "Configurações" pré-selecionada (primeira da lista). Os 4 componentes de conteúdo já
existem prontos (`Settings.vue`, `AgentsSkillsPanel.vue`, `HelpModal.vue`, sobre) — o trabalho é
essencialmente um componente de shell novo com a navegação lateral + adaptar os 4 existentes pra
renderizar como painel de conteúdo em vez de `Dialog` próprio (ou embrulhar cada um mantendo o
próprio HTML interno, sem o wrapper `Dialog`/header duplicado).

**8.** **Auto-atualização a partir do GitHub (verificar versão, mostrar changelog, baixar e
reinstalar)** — pedido do usuário (2026-08-18): o Cerne Code checar se a versão instalada é mais
antiga que a mais recente publicada no repositório git, avisar o usuário com a lista de novidades
(changelog/release notes), e ao clicar em "Atualizar" baixar o `.exe` novo, fechar o app e
reinstalar/substituir sozinho. Nada implementado ainda — registrado só como ideia. Pontos a
decidir na implementação: onde comparar a versão (GitHub Releases via API, ou uma tag/arquivo
`version.json` simples no repo?), formato do changelog (texto livre da release, ou algo
estruturado que o app já sabe renderizar?), e o mecanismo de "fechar e reinstalar sozinho" no
Windows (um processo não consegue substituir o próprio `.exe` em execução — precisa de um
instalador/updater auxiliar separado, ou baixar pra um caminho novo e trocar num próximo
lançamento, padrão comum de apps desktop tipo VS Code/Discord). Tauri tem um plugin oficial
(`tauri-plugin-updater`) feito exatamente pra isso — vale avaliar primeiro em vez de implementar
esse mecanismo na mão.

**9.** **Filtro de domínio (allow/block list) pra `web_search`** — pedido do usuário (2026-08-18):
"veja como eles implementam 'Search free'" apontando pro `unslothai/unsloth` no GitHub. Investigado
(via agente de pesquisa): não é algo no core de fine-tuning, é a tool `web_search` do Unsloth
Studio (`studio/backend/core/inference/tools.py`) — eles delegam a busca em si pra lib de terceiros
`ddgs` (sem scraping próprio, diferente do Cerne), mas têm uma ideia genuinamente nova que vale
considerar: **filtro de domínio configurável** — quando existe uma allowlist/blocklist de sites,
reescrevem a query anexando `(site:a.com OR site:b.com OR ...)` (limitado a 8 domínios por chamada,
com rotação determinística via CRC32 se a lista for maior) e ainda filtram os resultados devolvidos
contra a política antes de mostrar pro modelo. Útil pra casos tipo "só pesquise na documentação
oficial" numa Persona/Agente. **Não é uma lacuna de segurança** — o `validate_public_url`
(`websearch.rs`) do Cerne já bloqueia SSRF (localhost, IP privado/link-local, o que cobre o
endereço de metadados de nuvem `169.254.169.254` que o Unsloth também bloqueia) — isso já existe.
O multi-engine paralelo do Cerne (DuckDuckGo→Brave+Mojeek) também já é mais resiliente que a
abordagem deles (só DuckDuckGo via `ddgs`, sem fallback próprio). **Baixa prioridade, sem
implementação por ora** — perguntado ao usuário se faz diferença prática: só tem um caso de uso
real em mente, buscar na doc de design do TJPR (site próprio deles), fora isso não. Fica registrado
pra quando surgir um caso concreto que justifique, não é pra fazer nesta leva.

**10.** **Limpeza de segredos antes de mandar código pro LLM** — pedido do usuário (2026-08-18),
depois de perguntar como o mercado trata isso. Hoje o Cerne não faz NENHUMA varredura — se um
arquivo lido/editado tiver senha, chave de API ou token, vai pro modelo (e pro provider externo,
se for API) igual qualquer outro texto. Prática comum no mercado (GitHub Copilot, Cursor e afins):
varrer por padrões de segredo CONHECIDOS antes de montar o contexto (regex tipo o conjunto de
regras do `gitleaks`/`trufflehog` — chave da AWS, token do GitHub, bloco `BEGIN PRIVATE KEY`, etc.)
e substituir o valor por um placeholder tipo `[REDACTED]`. Limitações honestas dessa abordagem:
não pega segredo em formato não-padrão/customizado, e pode gerar falso-positivo (mascarar algo que
não era segredo de verdade). Nada implementado ainda — precisa de desenho próprio: onde aplicar
(toda leitura de arquivo? só antes de mandar pro provider?), qual conjunto de regras usar (vendorizar
uma lista tipo a do `gitleaks`, ou escrever um conjunto próprio menor e mais preciso?), e se
mostra pro usuário quando redigiu algo (avisar "1 segredo escondido nesta leitura" é mais honesto
que fazer silenciosamente).
