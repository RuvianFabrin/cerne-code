Tarefa: estudar a função {endereco} ({nome}).
Objetivo da pessoa: {objetivo}.
Funções que esta chama, já estudadas (resumo de cada uma):
{resumos}

Passos:
1. Decompile a função com a ferramenta de pseudocódigo do motor, usando o endereço {endereco}.
2. Se a saída vier cortada e o começo não bastar, peça a parte seguinte. Não peça de novo o que já leu.
3. Grave a ficha em {saida} com este formato (JSON):
   endereco, nome_original, nome_sugerido (nome curto em snake_case), resumo (até 160 caracteres),
   o_que_faz (até 5 frases), constantes [{valor, significado}], base (observado | derivado | inferido),
   confianca (alta | media | baixa), evidencias (ids que você usou), pendencias (o que não deu para saber).
   Deixe chama e chamada_por como estão; o código preenche.

Exemplo de resumo bom: "Calcula o dano final somando o base e descontando a armadura (fator 0,75)."
Exemplo ruim: "Função que faz cálculos." (não diz o que calcula).
