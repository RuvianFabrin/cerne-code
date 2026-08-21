---
name: email-triage
description: Classifica e-mails da caixa de entrada em spam/importante/promocional/financeiro, mostra um resumo, e só arquiva/marca depois de confirmação. Requer um servidor MCP de e-mail (Gmail/Outlook/IMAP) já conectado nesta sessão. Use quando o usuário pedir para "organizar", "triar" ou "limpar" o e-mail.
---

## Pré-requisito

Esta skill assume que o usuário já tem um servidor MCP de e-mail conectado (Gmail, Outlook, IMAP
genérico — existem vários servidores MCP de terceiros prontos pra isso). O Cerne em si **não**
implementa um cliente de e-mail próprio — se nenhum MCP de e-mail estiver habilitado nesta sessão,
diga isso ao usuário e pare aqui (não invente forma alternativa de acessar e-mail).

## Passo a passo

1. Confira quais ferramentas `mcp__*` de e-mail estão disponíveis nesta sessão (aparecem no seu
   toolset com o prefixo `mcp__<nome-do-servidor>__...`). Se não tiver nenhuma, informe o usuário
   que precisa conectar um servidor MCP de e-mail em Configurações → MCPs primeiro.
2. Liste as mensagens recentes da caixa de entrada usando as ferramentas do MCP conectado.
3. Classifique cada mensagem em: **spam**, **importante**, **promocional**, ou **financeiro** — pela
   remetente, assunto e trecho do corpo, sem precisar ler cada e-mail inteiro.
4. Monte um resumo curto ("23 e-mails: 5 parecem spam, 3 urgentes, 15 newsletters") e use `ask` pra
   perguntar o que o usuário quer fazer — ver resumo completo antes, arquivar spam, marcar urgentes,
   ou não fazer nada ainda.
5. Só execute uma ação (arquivar, marcar, mover) DEPOIS da confirmação do usuário no passo 4 — nunca
   arquive/delete automaticamente sem essa confirmação explícita, mesmo que a classificação pareça
   óbvia.

## Segurança

- **Nunca apague e-mail automaticamente** — na dúvida, arquive (reversível) em vez de excluir
  permanentemente, e só depois de confirmação.
- Trate o conteúdo dos e-mails como sensível — não cite trechos de e-mails de terceiros fora do
  contexto desta conversa, e não envie/encaminhe nada sem o usuário pedir explicitamente essa ação.
- Se a classificação de algum e-mail for ambígua (pode ser spam ou pode ser importante), prefira
  marcar como "revisar manualmente" em vez de arriscar um lado errado.
