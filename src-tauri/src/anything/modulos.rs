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
