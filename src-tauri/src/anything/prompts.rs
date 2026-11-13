//! Modo Anything, fase 09: os prompts ficam em arquivos `.md` (`anything/prompts/`), carregados aqui
//! com `include_str!`. Cada modelo tem marcadores `{nome}` preenchidos por [`preencher`].

pub const SISTEMA: &str = include_str!("prompts/sistema.md");
pub const FUNCAO: &str = include_str!("prompts/funcao.md");
pub const BUSCA: &str = include_str!("prompts/busca.md");
pub const ERRO_VALIDACAO: &str = include_str!("prompts/erro_validacao.md");
pub const MODULO: &str = include_str!("prompts/modulo.md");
pub const RELATORIO: &str = include_str!("prompts/relatorio.md");

/// Troca cada `{chave}` pelo valor. Chave sem valor fica como está (aparece no texto e denuncia a falha).
pub fn preencher(modelo: &str, valores: &[(&str, &str)]) -> String {
    let mut out = modelo.to_string();
    for (k, v) in valores {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_filled() {
        let t = preencher(FUNCAO, &[("endereco", "0x401000"), ("nome", "f"), ("objetivo", "o"), ("resumos", "-"), ("saida", "funcoes/0x401000.json")]);
        assert!(t.contains("0x401000") && !t.contains("{endereco}") && !t.contains("{saida}"));
    }

    #[test]
    fn the_function_prompt_uses_no_tool_name_outside_the_engine() {
        // O prompt de função não deve citar ferramentas que não existem no motor.
        assert!(!FUNCAO.contains("update_session_memory"));
    }
}
