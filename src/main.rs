//! Roda o modelo: `cargo run --release` e abra <http://127.0.0.1:8080>.
//! As variáveis estão em README.md, na seção "Ajustes". Nenhum segredo é impresso.

use rastreio_capi::modelo::{Ajustes, executar};

fn main() {
    if let Err(erro) = executar(&Ajustes::do_ambiente()) {
        eprintln!("{erro}");
        std::process::exit(1);
    }
}
