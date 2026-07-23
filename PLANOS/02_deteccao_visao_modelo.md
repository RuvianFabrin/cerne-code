# Detecção automática de visão do modelo

> **Status:** ✅ Concluído
> **Prioridade:** Alta (pré-requisito para computer_use)
> **Tipo:** Feature
> **Arquivos prováveis:** `src-tauri/src/providers/mod.rs`, `src-tauri/src/lib.rs`, `src/components/NewSessionDialog.vue`, `src/components/ProviderPicker.vue`, `src/stores/provider.ts`

## Problema

Hoje o campo `supports_vision` do provider custom é um checkbox manual que o usuário marca "no escuro" — sem saber se o modelo realmente suporta imagens. Se estiver errado, o computer_use fica habilitado para um modelo cego (ou desabilitado para um modelo com visão).

## Fase 1 — Teste automático de visão

### Tarefa 1.1 — Criar endpoint de teste de visão no backend
- Novo comando Tauri `test_vision(provider_id, model) -> Result<bool>`
- Envia uma imagem pequena (1x1 pixel PNG base64, ~100 bytes) como mensagem de usuário
- Envia junto o texto "Responda apenas: SIM"
- Se o modelo responder sem erro → `supports_vision = true`
- Se retornar erro de "image not supported" / "invalid content type" / 400 → `supports_vision = false`
- Timeout de 15 segundos (modelo lento não deve travar a UI)
- Salvar o resultado no `custom_providers.json` como `supports_vision: true/false`

### Tarefa 1.2 — Botão "Checar visão" na UI de nova sessão
- Em `NewSessionDialog.vue` / `ProviderPicker.vue`, ao lado do seletor de modelo:
  - Se `supports_vision` já está salvo → mostra badge "👁️ Com visão" ou "🚫 Sem visão"
  - Se não está salvo → mostra botão "Checar visão" + checkbox manual "Eu sei que tem visão" (default: desmarcado)
- Ao clicar "Checar visão":
  - Desabilita o botão, mostra spinner
  - Chama `test_vision(provider_id, model)`
  - Mostra resultado inline: "✅ Modelo suporta imagens" ou "❌ Modelo não suporta imagens"
  - Salva o resultado automaticamente
- O checkbox manual serve para o usuário que já sabe a resposta e não quer esperar o teste

### Tarefa 1.3 — Cache de resultados por modelo
- Salvar em `custom_providers.json` um mapa `vision_cache: { "modelo-x": true, "modelo-y": false }`
- Quando o usuário troca de modelo no picker, checar o cache antes de mostrar o botão
- Botão "Re-checar" disponível para forçar novo teste (modelo pode ter sido atualizado)

## Fase 2 — Validação no agent loop

### Tarefa 2.1 — Usar cache no agent loop
- No `mod.rs`, ao montar as tools de computer_use, checar o `vision_cache` do provider config
- Se o modelo não está no cache, fazer o teste automaticamente antes da primeira sessão
- Se o teste falhar, logar warning e não incluir as tools

## Critério de conclusão
- Usuário consegue checar visão de qualquer modelo com 1 clique
- Resultado é salvo e reutilizado
- Computer_use só aparece para modelos com visão confirmada
