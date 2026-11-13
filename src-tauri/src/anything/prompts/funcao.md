Tarefa: estudar UMA função e gravar a ficha dela num arquivo.
Função: {nome}, no endereço {endereco}.
Objetivo da pessoa: {objetivo}

Funções que esta chama, já estudadas (um resumo de cada):
{resumos}

O que fazer, nesta ordem:
1. Peça o pseudocódigo desta função ao motor com a ferramenta `native-function-pseudocode`, passando o endereço {endereco}. Uma vez só. Se a saída vier cortada e o começo não bastar, peça a parte seguinte; não peça de novo o que já leu.
2. Leia o pseudocódigo e entenda o que a função faz.
3. Escreva a ficha com a ferramenta `write_file` no caminho completo indicado no fim desta mensagem.

ATENÇÃO: a ficha é um texto SEU, curto. NÃO copie a saída da ferramenta para o arquivo (nada de pseudocódigo, assembly, "sha256", "callers"). Quem lê a ficha depois só vai ver o resumo, então ele precisa dizer o que a função faz.

A ficha é um JSON com EXATAMENTE estes campos:
- "endereco": o endereço, em texto, como {endereco}
- "nome_original": o nome que a ferramenta mostrou (ex.: sub_401000)
- "nome_sugerido": um nome curto em snake_case que diga o que a função faz
- "resumo": UMA frase de até 160 caracteres dizendo o que a função faz
- "o_que_faz": até 5 frases com os detalhes
- "constantes": lista de {"valor": "...", "significado": "..."} (pode ser lista vazia [])
- "base": "observado" (está escrito no código), "derivado" (você concluiu do que viu) ou "inferido" (palpite)
- "confianca": "alta", "media" ou "baixa"
- "evidencias": lista de textos curtos do que você usou (pode ser [])
- "pendencias": lista do que você não conseguiu saber (pode ser [])
Não escreva "chama" nem "chamada_por": o código preenche.

Exemplo de ficha (de OUTRA função; troque todos os valores pelos desta):
{
  "endereco": "0x401000",
  "nome_original": "sub_401000",
  "nome_sugerido": "calcula_dano_final",
  "resumo": "Calcula o dano final somando o dano base e descontando a armadura (fator 0,75).",
  "o_que_faz": "Recebe o dano base e a armadura. Multiplica a armadura por 0,75 e subtrai do dano base. Se o resultado for negativo, devolve zero.",
  "constantes": [{"valor": "0.75", "significado": "fator de redução da armadura"}],
  "base": "observado",
  "confianca": "alta",
  "evidencias": ["pseudocódigo: dano - armadura * 0.75"],
  "pendencias": []
}

Resumo ruim: "Função que faz cálculos." (não diz o que calcula). Resumo bom: o do exemplo.
Só termine depois de gravar o arquivo.
