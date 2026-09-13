"""Reparo pontual do chat_log.json de uma sessao travada.

Reproduz no arquivo real o mesmo conserto que src-tauri/src/history.rs faz em
memoria: garante que toda mensagem `assistant` com `tool_calls` seja seguida
imediatamente por uma mensagem `tool` para cada `tool_call_id` (e descarta
mensagens `tool` orfas). Sem isso o provider responde 400 em toda mensagem
nova e a conversa fica impossivel de continuar.

Uso: python reparar-chat-log.py <caminho do chat_log.json>
"""

import json
import shutil
import sys
from datetime import datetime
from pathlib import Path

MARKER = (
    "\u26a0\ufe0f Resultado perdido: o Cerne foi interrompido (cancelado, reiniciado ou deu "
    "erro) antes desta ferramenta terminar de responder. Ela pode nao ter rodado por completo "
    "\u2014 nao assuma nenhum resultado; se precisar de verdade, chame '{name}' de novo."
)


def synthetic(name: str, call_id: str) -> dict:
    return {
        "role": "tool",
        "content": MARKER.format(name=name),
        "tool_call_id": call_id,
        "name": name,
        "images": [],
    }


def repair(messages: list) -> tuple[list, int]:
    out: list = []
    pending: list = []  # [(id, nome)] ainda sem resposta
    fixed = 0

    def flush():
        nonlocal fixed
        for call_id, name in pending:
            out.append(synthetic(name, call_id))
            fixed += 1
        pending.clear()

    for msg in messages:
        if msg.get("role") == "tool":
            call_id = msg.get("tool_call_id")
            hit = next((c for c in pending if c[0] == call_id), None)
            if hit is not None:
                pending.remove(hit)
                out.append(msg)
            else:
                fixed += 1  # resposta sem pedido: o provider rejeitaria
            continue

        flush()

        if msg.get("role") == "assistant":
            for call in msg.get("tool_calls") or []:
                fn = (call.get("function") or {}).get("name", "?")
                pending.append((call["id"], fn))
        out.append(msg)

    flush()
    return out, fixed


def main() -> int:
    path = Path(sys.argv[1])
    messages = json.loads(path.read_text(encoding="utf-8"))

    repaired, fixed = repair(messages)

    if fixed == 0:
        print(f"nada a fazer: {path} ja esta integro ({len(messages)} mensagens)")
        return 0

    backup = path.with_name(f"{path.name}.bak-{datetime.now():%Y%m%d-%H%M%S}")
    shutil.copy2(path, backup)
    path.write_text(json.dumps(repaired, ensure_ascii=False, indent=2), encoding="utf-8")

    print(f"corrigido: {fixed} ajuste(s) em {len(messages)} mensagens")
    print(f"backup: {backup}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
