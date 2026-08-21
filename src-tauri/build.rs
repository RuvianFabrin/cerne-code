fn main() {
    tauri_build::build();

    // Fix pro `cargo test` falhar com STATUS_ENTRYPOINT_NOT_FOUND
    // (0xc0000139) tentando localizar `TaskDialogIndirect` — essa funcao so
    // existe na v6 (com tema) do comctl32.dll, carregada apenas quando o
    // binario tem um manifesto pedindo isso. O `tauri_build::build()` acima
    // ja embute esse manifesto, mas via `cargo:rustc-link-arg-bin=cerne=...`
    // (achado lendo o output do build script), que so vale pro binario
    // principal `cerne.exe` — o binario de teste sintetico que `cargo test`
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
