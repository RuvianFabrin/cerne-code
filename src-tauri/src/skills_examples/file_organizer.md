---
name: file-organizer
description: Organiza os arquivos soltos de uma pasta (ex. Downloads) em subpastas por categoria, com plano revisável antes de mover e undo de verdade. Use quando o usuário pedir para "organizar", "arrumar" ou "limpar" uma pasta.
---

## Objetivo

Organizar arquivos soltos numa pasta em subpastas por categoria (Documentos, Imagens, Vídeos,
Instaladores, Outros, ou o que fizer sentido pro conteúdo real da pasta) — **nunca apagar nada, só
mover**, e sempre com um jeito real de desfazer.

## Passo a passo

1. Use `list_dir` na pasta alvo pra ver o que tem lá — classifique por extensão (e, se ajudar, por
   tamanho/data) em categorias que façam sentido pro conteúdo real (não force categorias fixas se a
   pasta não tiver esse perfil de arquivo).
2. Monte um plano curto ("87 arquivos: 34 Documentos, 20 Imagens, 12 Instaladores, 21 Outros") e use
   `ask` pra confirmar com o usuário antes de mover qualquer coisa — mostre as opções de continuar,
   ajustar categorias, ou cancelar.
3. **Antes de mover, garanta um undo de verdade**: se ainda não existe uma ferramenta de organizar
   arquivos com log de undo nesta máquina (confira se já existe uma skill/ferramenta Python chamada
   algo como `organizador-de-arquivos`), use `create_python_tool` pra criar uma — um script que:
   - Recebe uma lista de `(origem, destino)` como argumento (ou lê de um JSON).
   - Grava um manifesto (`organize-log-<timestamp>.json`) com cada movimento ANTES de mover.
   - Move os arquivos.
   - Aceita um segundo modo `--reverse <manifesto>` que lê o log de volta e desfaz exatamente os
     movimentos registrados.
   Depois de criada uma vez, essa ferramenta fica disponível pra qualquer sessão futura — não
   precisa recriar toda vez, só reusar (`uv run "<caminho>" ...`, indicado pela skill companheira
   `python-tool-organizador-de-arquivos` que `create_python_tool` já gera sozinho).
4. Rode a ferramenta com o plano aprovado. Ao terminar, informe ao usuário quantos arquivos foram
   movidos e onde está o log, caso ele queira desfazer depois (`uv run "<caminho>" --reverse
   "<log>"`).

## Segurança

- **Nunca apague nada** — não existe (e não deveria existir) uma ferramenta de deletar arquivo
  disponível pra você. Essa skill é só sobre MOVER.
- **Confirmação obrigatória antes de mover** — sempre passe pelo `ask` do passo 2, mesmo que o plano
  pareça óbvio. Em modo Manual do Cerne, cada `run_command` real ainda pausa pedindo aprovação de
  qualquer forma — a pergunta do passo 2 é sobre o PLANO, não substitui essa aprovação.
- Se a pasta tiver muitos arquivos (centenas+), organize em lotes em vez de um `run_command` gigante
  de uma vez — mais fácil de revisar e de desfazer parcialmente se algo der errado.
