fn main() {
    tauri_build::build();

    // ⚠️ STACK DA THREAD PRINCIPAL (Windows) — sem isso o app morre no
    // startup com STATUS_STACK_OVERFLOW (0xC00000FD), achado ao vivo em
    // 2026-09-15.
    //
    // Causa: o `tiktoken` (contagem de tokens do medidor de contexto, ver
    // `context.rs`) mantém um cache thread-local em `piece_cache.rs`:
    //
    //     const SLOTS: usize = 4096;
    //     struct Slot { owner: u64, len: u8, n_tokens: u8,
    //                   key: [u8; 96], tokens: [u32; 48] }   // 304 bytes
    //     thread_local! {
    //         static CACHE: RefCell<Box<[Slot; SLOTS]>> =
    //             RefCell::new(Box::new([EMPTY; SLOTS]));
    //     }
    //
    // O `Box::new([EMPTY; SLOTS])` monta o array **na stack** antes de mover
    // pro heap: 304 × 4096 ≈ **1,19 MB de uma vez**. E é `thread_local`, então
    // acontece na primeira contagem de CADA thread nova — não dá pra
    // "pré-aquecer" numa thread separada.
    //
    // Medido (thread com stack explícito chamando `count`): 256 KB, 512 KB e
    // 1 MB estouram; 2 MB passa. A thread principal do Windows tem **1 MB** por
    // padrão (o resto tem 2 MB: threads do tokio e `std::thread`), e
    // `get_session_context_usage` é comando Tauri **síncrono** — roda na
    // principal, que é exatamente onde estoura.
    //
    // 16 MB dá folga confortável e não custa memória de verdade: é reserva de
    // espaço de endereço, com commit sob demanda.
    //
    // Sem `-bin=`: vale pra TODOS os binários da crate, inclusive o de teste
    // (mesmo padrão do fix de manifesto logo abaixo).
    #[cfg(windows)]
    println!("cargo:rustc-link-arg=/STACK:16777216");

    // Fix pro `cargo test` falhar com STATUS_ENTRYPOINT_NOT_FOUND
    // (0xc0000139) tentando localizar `TaskDialogIndirect` - essa funcao so
    // existe na v6 (com tema) do comctl32.dll, carregada apenas quando o
    // binario tem um manifesto pedindo isso. O `tauri_build::build()` acima
    // ja embute esse manifesto, mas via `cargo:rustc-link-arg-bin=cerne=...`
    // (achado lendo o output do build script), que so vale pro binario
    // principal `cerne.exe` - o binario de teste sintetico que `cargo test`
    // gera (`cerne_lib-<hash>.exe`) nunca recebia esse manifesto, entao
    // carregava a v5 antiga do comctl32.dll (sem `TaskDialogIndirect`) e
    // falhava ja na inicializacao, antes de qualquer teste rodar. A flag
    // abaixo, sem `-bin=`, se aplica a TODOS os binarios da crate (inclusive
    // os de teste), entao cobre o caso que o Tauri deixa de fora.
    #[cfg(windows)]
    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}
