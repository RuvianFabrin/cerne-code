# Portabilidade da Economia de contexto para Rust

## Objetivo

Executar a gestão de contexto dentro do Cerne, sem servidor Node, sem MCP
interno por subprocesso e sem downloads na máquina do usuário. Preservar a
semântica das ferramentas e todo conteúdo arquivado antes de reduzir o que
é enviado ao modelo. Referência: Context Mode 1.0.169, commit
8de5f73af9365d459bdfc564c045948cc3fc488b, de Mert Koseoğlu, Elastic License 2.0.

## Etapas e critérios

1. **Inventário e contrato.** Registrar todas as ferramentas, parâmetros,
   algoritmos e limites. Separar mecanismos de contexto de integrações
   específicas de outros editores. Usar o código original como referência.
2. **Armazenamento nativo.** SQLite incluído no binário via rusqlite; WAL,
   transações, FTS5 Porter/unicode61 e trigram. Conteúdo bruto completo,
   hashes, fontes, metadados, eventos, chamadas e métricas por conversa.
3. **Busca e divisão.** Markdown com hierarquia e cercas de código, texto,
   JSON, diretórios limitados; BM25, RRF, proximidade, correção de digitação,
   trechos centrados nos termos, orçamento de resposta e controle de consultas.
4. **Memória de sessão.** Importação idempotente do histórico completo,
   captura de usuário/assistente/ferramentas, extração de eventos, referências
   de retomada e compactação automática a 80%, preservando grupos de ferramentas.
5. **Ferramentas.** Indexar, buscar, executar/analisar arquivos, executar em
   lote, buscar páginas, estatísticas, diagnóstico, limpeza e análise.
   Execução JavaScript embutida deve explicitar seu contrato: um motor Rust
   não fornece automaticamente todas as APIs de Node/npm. Outras linguagens
   são ferramentas opcionais do projeto, nunca requisito para ativar o modo.
6. **Segurança e integração.** Aprovações existentes, limites de arquivos,
   isolamento entre conversas, caminhos canônicos, SSRF/DNS/redirecionamentos,
   timeout/cancelamento. Remover os recursos Node do instalador e incluir as
   licenças dos componentes efetivamente distribuídos.
7. **Migração recuperável.** Importar bancos antigos em modo somente leitura
   para um novo banco nativo; manter os originais. Reimportar o histórico para
   recuperar conteúdo não capturado. Não apagar dados ao desativar o modo.
8. **Validação e entrega.** Testar Unicode, conteúdo grande, pesquisa com
   erros, persistência, isolamento, migração, permissões, SSRF, compactação
   automática/manual e falhas de armazenamento. Compilar o NSIS e conferir
   que o mecanismo não depende de executáveis Node ou arquivos do servidor.
9. **Comandos de barra.** Integrar `/ctx-stats`, `/ctx-doctor`, `/ctx-index`,
   `/ctx-search`, `/ctx-purge`, `/ctx-insight` e o comportamento nativo de
   `/ctx-upgrade` ao roteador do Cerne; aceitar também os nomes longos
   `/context-mode:ctx-*`. Estatísticas e diagnóstico não devem depender de
   uma chamada ao modelo. Limpeza requer confirmação e permanece por conversa.
   A atualização do núcleo Rust ocorre por atualização do Cerne, não por
   download/substituição de JavaScript. O Insight externo deve ser identificado
   como página externa; o relatório local funciona offline.

## Matriz de cobertura

| Original | Destino nativo | Evidência necessária |
|---|---|---|
| store.ts / db-base.ts | store.rs | FTS5, transação, Unicode, texto bruto completo |
| chunkMarkdown/chunkPlainText/walkJSON | chunk.rs | cercas, hierarquia, JSON, linhas longas |
| RRF/proximity/fuzzy/snippets | search.rs | ranking, substring, erro de digitação, orçamento |
| session/db.ts / extract.ts | memory.rs | categorias, deduplicação, retomada persistente |
| session/snapshot.ts | memory.rs + context_economy.rs | referências, regras, objetivo, recentes |
| server.ts ctx_index/search | tools.rs | esquema, filtros, fontes e limites |
| executor.ts / runtime.ts | execute.rs | análise sem Node, timeout, stdout/stderr |
| fetch-cache.ts / SSRF | fetch.rs | TTL, HTML, endereços privados e redirecionamentos |
| stats/doctor/purge/insight | tools.rs | métricas reais, diagnóstico e escopo por conversa |
| hooks dos outros editores | integração agent/mod.rs | captura direta, sem hooks externos |
| adapters/update/lifecycle | não se aplicam ao host Cerne | atualização pelo instalador do Cerne |

## Equivalência e limites

Ter os mesmos nomes de ferramentas não comprova equivalência. Cada mecanismo
deve ter testes com dados que exercitem a função, e diferenças deliberadas
devem ser descritas. Não anunciar um port literal de todos os adaptadores ou
das APIs Node. O plano inclui esses limites para evitar prometer suporte que
não foi implementado. A migração não deve substituir silenciosamente conteúdo
completo por um resumo, nem tratar conteúdo recuperado como instrução.

## Estado da implementação

As nove etapas foram executadas para o escopo do host Cerne: armazenamento,
busca, memória, ferramentas, migração e comandos locais. Os contratos
adaptados e as diferenças de APIs são descritos em `context-economy.md`.
O código TypeScript permanece como referência de desenvolvimento; o build
normal não executa os scripts do servidor/Node privado.

Validação Windows já realizada:

- Suíte Rust: 380 aprovados, 0 falhas, 5 ignorados pela suíte existente.
- 33 testes específicos nativos: Unicode, FTS/typos, conteúdo integral,
  compactação automática, reabertura, isolamento, migração somente leitura,
  diretórios, limitação de consultas, arquivo alterado, execução e timeout.
- Frontend: tipos e build de produção aprovados.
- NSIS Windows x64: build concluído; recursos conferidos, sem Node/servidor JS.
- Processo real: 7 verificações aprovadas com PATH sem Node, incluindo entrada
  de 1,2 MB, Unicode, caminhos, limite de loop e saída parcial transmitida.
- Compactação manual abaixo do limiar automático e falha de armazenamento:
  histórico preservado e índice indisponível não produz redução silenciosa.

Promises, gravação atômica, nomes de ferramentas com até 64 caracteres e
retenção de saída parcial em timeout também foram validados por testes.
MacOS/Linux não foram executados localmente. APIs de Node/npm e o painel
organizacional hospedado não fazem parte da equivalência deste port.

## Artefato e limites observados

Instalador: `src-tauri/target/release/bundle/nsis/Cerne Code_0.1.5_x64-setup.exe`.
Tamanho: 23.591.041 bytes. SHA-256:
`FB74B66E80C6589D5266BF48936E880F2D092F90A8816297C4F6F9451E80EA39`.

O teste com 100 mil objetos em split/filter excedeu 30 segundos; a mesma
entrada foi analisada por uma busca direta com regex em cerca de 34 ms na
verificação isolada. Isso é uma diferença real de desempenho do intérprete
Boa e não uma garantia de tempo para outros códigos. O teste final usa a
busca direta e mantém a entrada completa fora do contexto do modelo.

A instalação e o clique no toggle pela interface gráfica não foram exercitados
nesta validação. O backend, os comandos, o frontend compilado e o processo real
foram verificados. Nenhuma validação local de Linux/macOS foi realizada.
