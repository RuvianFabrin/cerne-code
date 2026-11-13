//! Modo Anything, fase 10: agrupa as funções em módulos pelo código, sem o modelo. Módulo = componente
//! ligado do grafo de chamadas entre as funções já estudadas; grupo grande demais é dividido.

use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Tamanho máximo de um módulo antes de ser dividido.
pub const MODULO_MAXIMO: usize = 15;

/// Componentes ligados do grafo (ignorando a direção das arestas), em ordem estável.
pub fn componentes(nodos: &[String], arestas: &HashMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let mut vizinhos: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in nodos {
        vizinhos.entry(n.clone()).or_default();
    }
    for (de, para) in arestas {
        for p in para {
            if vizinhos.contains_key(de) && vizinhos.contains_key(p) {
                vizinhos.get_mut(de).unwrap().insert(p.clone());
                vizinhos.get_mut(p).unwrap().insert(de.clone());
            }
        }
    }
    let mut visto = BTreeSet::new();
    let mut grupos = Vec::new();
    for n in vizinhos.keys() {
        if visto.contains(n) {
            continue;
        }
        let mut grupo = Vec::new();
        let mut pilha = vec![n.clone()];
        visto.insert(n.clone());
        while let Some(x) = pilha.pop() {
            grupo.push(x.clone());
            for v in &vizinhos[&x] {
                if visto.insert(v.clone()) {
                    pilha.push(v.clone());
                }
            }
        }
        grupo.sort();
        grupos.push(grupo);
    }
    grupos
}

/// Componentes com menos funções que isto não viram módulo sozinhos: entram no pool de "avulsas".
pub const COMPONENTE_PEQUENO: usize = 3;

/// Agrupa as funções em módulos: cada componente do grafo com [`COMPONENTE_PEQUENO`] funções ou mais
/// é um módulo (dividido se passar do máximo); as componentes menores (inclusive função isolada) são
/// juntadas por endereço, porque o compilador costuma deixar perto as funções do mesmo arquivo-fonte, e
/// assim uma função solta não vira um módulo de uma linha só.
pub fn agrupar(nodos: &[String], arestas: &HashMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let (grandes, pequenas): (Vec<_>, Vec<_>) = componentes(nodos, arestas).into_iter().partition(|c| c.len() >= COMPONENTE_PEQUENO);
    let mut modulos: Vec<Vec<String>> = grandes.into_iter().flat_map(dividir).collect();
    let mut avulsas: Vec<String> = pequenas.into_iter().flatten().collect();
    avulsas.sort();
    modulos.extend(dividir(avulsas));
    modulos
}

/// Divide um grupo grande em pedaços de até [`MODULO_MAXIMO`], por endereço (estável).
pub fn dividir(grupo: Vec<String>) -> Vec<Vec<String>> {
    grupo.chunks(MODULO_MAXIMO).map(|c| c.to_vec()).collect()
}

/// Nome de arquivo estável para um módulo: `modulo_01`, `modulo_02`...
pub fn nome_arquivo(indice: usize) -> String {
    format!("modulo_{:02}", indice + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn unconnected_functions_are_separate_modules() {
        let g: HashMap<String, Vec<String>> = HashMap::new();
        assert_eq!(componentes(&s(&["0x1", "0x2"]), &g).len(), 2);
    }

    #[test]
    fn connected_functions_share_a_module_whatever_the_direction() {
        let mut g = HashMap::new();
        g.insert("0x1".to_string(), s(&["0x2"]));
        let c = componentes(&s(&["0x1", "0x2", "0x3"]), &g);
        assert_eq!(c.len(), 2);
        assert!(c.iter().any(|m| m == &s(&["0x1", "0x2"])));
    }

    #[test]
    fn a_big_module_is_split_into_pieces() {
        let grande: Vec<String> = (0..40).map(|i| format!("0x{i:x}")).collect();
        let partes = dividir(grande);
        assert_eq!(partes.len(), 3);
        assert!(partes.iter().all(|p| p.len() <= MODULO_MAXIMO));
    }

    #[test]
    fn module_file_names_are_stable() {
        assert_eq!(nome_arquivo(0), "modulo_01");
        assert_eq!(nome_arquivo(11), "modulo_12");
    }
}

/// Seções que um arquivo de módulo ou de relatório precisa ter. Devolve as que faltam.
pub fn secoes_faltando(texto: &str, esperadas: &[&str]) -> Vec<String> {
    esperadas.iter().filter(|s| !texto.lines().any(|l| l.trim() == **s)).map(|s| format!("falta a seção '{s}'")).collect()
}

/// Seções de um módulo.
pub const SECOES_MODULO: &[&str] = &["## Nome", "## O que faz", "## Funções principais", "## Dúvidas em aberto"];
/// Seções do relatório.
pub const SECOES_RELATORIO: &[&str] = &["## Resposta ao objetivo", "## Como funciona", "## Observado", "## Inferido", "## Sem resposta"];

#[cfg(test)]
mod secoes_tests {
    use super::*;

    #[test]
    fn missing_sections_are_listed() {
        let texto = "## Nome\nx\n## O que faz\ny\n";
        let f = secoes_faltando(texto, SECOES_MODULO);
        assert_eq!(f.len(), 2);
        assert!(f[0].contains("Funções principais"));
    }

    #[test]
    fn a_complete_report_has_no_missing_sections() {
        let texto = SECOES_RELATORIO.join("\ncorpo\n");
        assert!(secoes_faltando(&texto, SECOES_RELATORIO).is_empty());
    }
}

#[cfg(test)]
mod agrupar_tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn isolated_functions_are_pooled_instead_of_one_module_each() {
        let g: HashMap<String, Vec<String>> = HashMap::new();
        let m = agrupar(&s(&["0x30", "0x10", "0x20"]), &g);
        assert_eq!(m, vec![s(&["0x10", "0x20", "0x30"])]);
    }

    #[test]
    fn a_real_cluster_stays_its_own_module_and_the_loose_ones_share_another() {
        let mut g = HashMap::new();
        g.insert("0x1".to_string(), s(&["0x2", "0x3"]));
        let m = agrupar(&s(&["0x1", "0x2", "0x3", "0x8", "0x9"]), &g);
        assert_eq!(m.len(), 2);
        assert!(m.contains(&s(&["0x1", "0x2", "0x3"])));
        assert!(m.contains(&s(&["0x8", "0x9"])));
    }

    #[test]
    fn nothing_is_lost_and_nothing_repeats() {
        let nodos: Vec<String> = (0..50).map(|i| format!("0x{:x}", i * 16)).collect();
        let mut g = HashMap::new();
        for i in 0..20 {
            g.insert(nodos[i].clone(), vec![nodos[i + 1].clone()]);
        }
        let m = agrupar(&nodos, &g);
        let mut todos: Vec<String> = m.iter().flatten().cloned().collect();
        todos.sort();
        let mut esperado = nodos.clone();
        esperado.sort();
        assert_eq!(todos, esperado);
        assert!(m.iter().all(|x| x.len() <= MODULO_MAXIMO));
    }
}
