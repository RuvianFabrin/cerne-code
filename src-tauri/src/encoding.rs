//! Deteccao e preservacao de codificacao de arquivo de texto.
//!
//! O Cerne edita arquivos de projetos existentes, que nem sempre sao UTF-8 —
//! arquivos gerados no Windows sao comumente UTF-16 (com BOM) ou uma
//! codificacao legada de 1 byte (Windows-1252, as vezes rotulada "ISO"). Ler
//! como UTF-8 puro (`std::fs::read_to_string`) e seguro na LEITURA — bytes
//! invalidos simplesmente falham, nada e escrito — mas um caso especifico e
//! perigoso: texto ASCII puro salvo em UTF-16LE/BE (`'H'` `'\0'` `'e'` `'\0'`
//! ...) e, por coincidencia, uma sequencia de bytes valida como UTF-8 (byte
//! `0x00` e o proprio caractere NUL, um code point valido) — a leitura
//! "funciona" mas devolve uma string cheia de NUL entre cada caractere, e
//! reescrever isso como UTF-8 de verdade corrompe um arquivo que programas
//! Windows liam perfeitamente bem.
//!
//! Usa `encoding_rs` (Mozilla, implementacao de referencia do WHATWG Encoding
//! Standard, o motor por baixo do Firefox/Servo) pra decodificar/reencodar, e
//! `chardetng` (tambem da Mozilla, o mesmo detector estatistico do "Detectar
//! automaticamente" do Firefox) pra adivinhar a codificacao legada quando nao
//! ha BOM nem UTF-8 valido — em vez de uma heuristica caseira que so cobre
//! Windows-1252/Latin-1, isso da cobertura real pra qualquer codificacao que
//! o navegador tambem reconheceria (Shift-JIS, GBK, EUC-KR, KOI8-R, as
//! variantes ISO-8859 de verdade, etc.), nao so a mais comum no Windows.
//! Mesmo padrao das outras portas desta sessao (`ast-grep-core`,
//! `grep-regex`/`grep-searcher`, `dashmap`): crate real e mantida, nada de
//! reimplementar deteccao de charset a mao.

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8};

/// Codificacao (e presenca de BOM) detectada na leitura de um arquivo — usada
/// pra reencodar o conteudo editado de volta no mesmo formato ao escrever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileEncoding {
    encoding: &'static Encoding,
    bom: bool,
}

impl FileEncoding {
    /// UTF-8 sem BOM — o default razoavel pra arquivo novo, que nao tem um
    /// "original" de onde herdar codificacao.
    pub const UTF8_NO_BOM: FileEncoding = FileEncoding {
        encoding: UTF_8,
        bom: false,
    };
}

/// Detecta a codificacao dos bytes crus de um arquivo e decodifica pra uma
/// `String` (sempre UTF-8 internamente, como toda `String` do Rust). Ordem
/// de deteccao, do mais confiavel pro mais especulativo:
/// 1. BOM (byte-order mark) — UTF-8/UTF-16LE/UTF-16BE, deterministico.
/// 2. Sem BOM, UTF-8 estrito valido — a maioria dos arquivos de texto hoje
///    (inclusive todo arquivo puramente ASCII, que e um subconjunto valido
///    de UTF-8 e de qualquer codificacao legada de 1 byte ao mesmo tempo).
/// 3. Sem BOM e invalido como UTF-8 — deteccao estatistica via `chardetng`
///    (mesmo motor do Firefox), que cobre qualquer codificacao legada real
///    (Windows-1252/ISO-8859-*/Shift-JIS/GBK/EUC-KR/KOI8-R/etc.), nao so
///    Windows-1252. Quando o sinal e fraco (arquivo curto, pouco texto
///    nao-ASCII), o proprio `chardetng` cai pra Windows-1252 sozinho — mesmo
///    comportamento de fallback que um navegador teria pra pagina sem
///    `charset` declarado.
pub fn decode(bytes: &[u8]) -> (String, FileEncoding) {
    if let Some((encoding, bom_len)) = Encoding::for_bom(bytes) {
        let (text, _) = encoding.decode_without_bom_handling(&bytes[bom_len..]);
        return (
            text.into_owned(),
            FileEncoding {
                encoding,
                bom: true,
            },
        );
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return (
            text.to_string(),
            FileEncoding {
                encoding: UTF_8,
                bom: false,
            },
        );
    }
    // Allow: sem risco de execucao de script aqui (nao e um navegador), entao
    // vale detectar ISO-2022-JP de verdade em vez de descartar a possibilidade.
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Allow);
    detector.feed(bytes, true);
    // Ja sabemos que nao e UTF-8 valido (checado acima) - nega explicitamente
    // pro guess() pra ele nunca devolver UTF-8 de volta por engano.
    let guessed = detector.guess(None, Utf8Detection::Deny);
    let (text, _) = guessed.decode_without_bom_handling(bytes);
    (
        text.into_owned(),
        FileEncoding {
            encoding: guessed,
            bom: false,
        },
    )
}

/// Reencoda texto editado de volta pros bytes reais a escrever em disco, na
/// mesma codificacao (e BOM) detectada na leitura original.
///
/// UTF-16LE/BE sao tratados a mao (`str::encode_utf16` + empacotamento de
/// bytes) porque o `encoding_rs` — de proposito, por seguir o WHATWG Encoding
/// Standard — nao codifica PARA UTF-16: a spec e sobre decodificar conteudo
/// da web, e formularios web nunca submetem UTF-16, entao o encoder desses
/// dois rotulos so devolve UTF-8. Os demais (Windows-1252 etc.) codificam
/// normalmente pelo `encoding_rs`.
pub fn encode(text: &str, enc: FileEncoding) -> Vec<u8> {
    let mut bytes = if enc.encoding == UTF_16LE {
        text.encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect()
    } else if enc.encoding == UTF_16BE {
        text.encode_utf16()
            .flat_map(|unit| unit.to_be_bytes())
            .collect()
    } else {
        let (bytes, _, _) = enc.encoding.encode(text);
        bytes.into_owned()
    };
    if enc.bom {
        let bom_bytes: &[u8] = if enc.encoding == UTF_16LE {
            &[0xFF, 0xFE]
        } else if enc.encoding == UTF_16BE {
            &[0xFE, 0xFF]
        } else {
            &[0xEF, 0xBB, 0xBF]
        };
        let mut with_bom = bom_bytes.to_vec();
        with_bom.append(&mut bytes);
        with_bom
    } else {
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::WINDOWS_1252;

    #[test]
    fn decodes_plain_ascii_as_utf8_no_bom() {
        let (text, enc) = decode(b"hello world");
        assert_eq!(text, "hello world");
        assert_eq!(enc, FileEncoding::UTF8_NO_BOM);
    }

    #[test]
    fn decodes_utf8_bom_and_strips_it() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("ola".as_bytes());
        let (text, enc) = decode(&bytes);
        assert_eq!(text, "ola");
        assert_eq!(
            enc,
            FileEncoding {
                encoding: UTF_8,
                bom: true
            }
        );
    }

    #[test]
    fn decodes_utf16le_bom_correctly_instead_of_garbling_as_utf8() {
        // "Hi" em UTF-16LE com BOM: FF FE 48 00 69 00
        let bytes = [0xFF, 0xFE, 0x48, 0x00, 0x69, 0x00];
        let (text, enc) = decode(&bytes);
        assert_eq!(text, "Hi");
        assert_eq!(
            enc,
            FileEncoding {
                encoding: UTF_16LE,
                bom: true
            }
        );
    }

    #[test]
    fn decodes_utf16be_bom() {
        // "Hi" em UTF-16BE com BOM: FE FF 00 48 00 69
        let bytes = [0xFE, 0xFF, 0x00, 0x48, 0x00, 0x69];
        let (text, enc) = decode(&bytes);
        assert_eq!(text, "Hi");
        assert_eq!(
            enc,
            FileEncoding {
                encoding: UTF_16BE,
                bom: true
            }
        );
    }

    #[test]
    fn detects_shift_jis_via_chardetng_not_just_windows_1252() {
        // Antes da troca pra chardetng, todo arquivo sem BOM e invalido como
        // UTF-8 caia direto em Windows-1252, mesmo sendo japones de verdade —
        // isso confirma a deteccao estatistica real cobre mais que so
        // Windows-1252/ISO-8859-1. Texto repetido pra dar sinal estatistico
        // suficiente pro chardetng (amostra curta demais e ambigua).
        let japanese = "こんにちは、世界。".repeat(20);
        let (shift_jis_bytes, _, had_errors) = encoding_rs::SHIFT_JIS.encode(&japanese);
        assert!(
            !had_errors,
            "esse texto deveria codificar limpo em Shift-JIS"
        );

        let (decoded, enc) = decode(&shift_jis_bytes);
        assert_eq!(
            enc.encoding.name(),
            "Shift_JIS",
            "deveria detectar Shift-JIS, nao cair em Windows-1252"
        );
        assert_eq!(decoded, japanese);
    }

    #[test]
    fn falls_back_to_windows_1252_for_invalid_utf8_no_bom() {
        // 0xE9 sozinho e um lead byte de continuacao UTF-8 invalido, mas e
        // 'e' com acento agudo em Windows-1252/Latin-1.
        let bytes = [b'c', b'a', b'f', 0xE9];
        let (text, enc) = decode(&bytes);
        assert_eq!(text, "café");
        assert_eq!(
            enc,
            FileEncoding {
                encoding: WINDOWS_1252,
                bom: false
            }
        );
    }

    #[test]
    fn round_trips_utf16le_through_edit() {
        let original_bytes = {
            let mut b = vec![0xFF, 0xFE];
            b.extend("café".encode_utf16().flat_map(|u| u.to_le_bytes()));
            b
        };
        let (text, enc) = decode(&original_bytes);
        assert_eq!(text, "café");
        let edited = text.replace("café", "chá");
        let rewritten = encode(&edited, enc);
        let (roundtrip_text, roundtrip_enc) = decode(&rewritten);
        assert_eq!(roundtrip_text, "chá");
        assert_eq!(roundtrip_enc, enc);
    }

    #[test]
    fn round_trips_windows_1252_through_edit() {
        let (text, enc) = decode(&[b'c', b'a', b'f', 0xE9]);
        let edited = text.replace("café", "cafe com leite");
        let rewritten = encode(&edited, enc);
        assert_eq!(rewritten, b"cafe com leite");
    }

    #[test]
    fn new_file_defaults_to_utf8_no_bom() {
        let bytes = encode("novo arquivo", FileEncoding::UTF8_NO_BOM);
        assert_eq!(bytes, b"novo arquivo");
    }
}
