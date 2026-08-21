# Testar — Cerne Code

> Coisas já IMPLEMENTADAS, mas ainda não confirmadas por você na janela real. Histórico completo
> de como cada uma foi construída continua em `PLANOS/14_backlog_pendente.md` — aqui é só a lista
> do que falta clicar e conferir.

---

> Numeração fixa — ao pedir pra testar algo desta lista, uso o número do item. Quando um item é
> confirmado, ele sai daqui e vai pra `PLANOS/Feito.md`; os demais são renumerados.

**1.** **Backup de sessão como `.zip`** — em Configurações → seção "Backup de sessões": exportar
sessão atual, exportar todas, importar de um `.zip`. Sessão importada sempre vira uma sessão NOVA
(id novo) — reimportar o mesmo zip cria uma segunda cópia em vez de dar erro.

**2.** **Backup/sincronização via `git`** — mesma seção de Configurações, logo abaixo do `.zip`.
**Atualizado (2026-08-18)** depois de um travamento real do modal de Configurações ao clicar sem
querer em "Sincronizar" sem nome/email/token configurados — dois fixes: (a) `run_git` ganhou timeout
de 20s + `GIT_TERMINAL_PROMPT=0` (git falha rápido com erro em vez de travar esperando credencial
que não tem como chegar); (b) nome/email locais (podem ser fake) e um token (guardado criptografado
no cofre do sistema) agora são configuráveis na própria seção, em vez de depender só do que já
estivesse configurado no git/sistema. Teste: "Iniciar backup via git" → definir remoto https →
preencher nome/email fake → colar um token de acesso (ex: Personal Access Token do GitHub) →
"Sincronizar" (commit + pull + push). Também vale confirmar que clicar em "Sincronizar" SEM nome/
email configurado dá um erro claro na hora (não trava mais).
