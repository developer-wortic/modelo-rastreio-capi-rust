//! Roda o binário de verdade (o `main` só é coberto assim). Nada sai da máquina e nenhum evento vai à Meta:
//! o modelo sem Pixel e token fica em modo demonstração.

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const BINARIO: &str = env!("CARGO_BIN_EXE_rastreio-capi-modelo");

/// Uma porta livre agora. Ela é solta logo depois, para o binário usar.
fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("porta livre")
        .local_addr()
        .expect("endereço")
        .port()
}

#[test]
fn porta_que_nao_abre_para_com_a_mensagem_e_codigo_1() {
    let saida = Command::new(BINARIO)
        .env("PORTA", "abc")
        .env_remove("RASTREIO_PIXEL_ID")
        .output()
        .expect("roda o binário");
    assert_eq!(saida.status.code(), Some(1));
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(erro.contains("não consegui abrir"), "stderr: {erro}");
}

#[test]
fn segredo_curto_para_com_a_mensagem() {
    let saida = Command::new(BINARIO)
        .env("PORTA", porta_livre().to_string())
        .env("RASTREIO_SEGREDO", "curto")
        .output()
        .expect("roda o binário");
    assert_eq!(saida.status.code(), Some(1));
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(erro.contains("32 caracteres"), "stderr: {erro}");
}

#[test]
fn sobe_atende_os_pedidos_e_sai_limpo_no_limite() {
    let porta = porta_livre();
    let mut filho = Command::new(BINARIO)
        .env("PORTA", porta.to_string())
        .env("RASTREIO_MAX_PEDIDOS", "3")
        .env("RASTREIO_SEGREDO", "0123456789abcdef0123456789abcdef")
        .env_remove("RASTREIO_PIXEL_ID")
        .env_remove("RASTREIO_ACCESS_TOKEN")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sobe o binário");
    let linhas = BufReader::new(filho.stdout.take().expect("saída"));
    let (envio, recebe) = std::sync::mpsc::channel();
    thread::spawn(move || {
        for linha in linhas.lines().map_while(Result::ok) {
            if envio.send(linha).is_err() {
                break;
            }
        }
    });

    // a primeira resposta conta como pedido 1; antes de subir, a conexão recusada não conta
    let base = format!("http://127.0.0.1:{porta}");
    let inicio = Instant::now();
    loop {
        if ureq::get(&format!("{base}/")).call().is_ok() {
            break;
        }
        assert!(
            inicio.elapsed() < Duration::from_secs(10),
            "o binário não subiu"
        );
        thread::sleep(Duration::from_millis(50));
    }
    let pixel = ureq::get(&format!(
        "{base}/r?u=https%3A%2F%2Floja.com%2F%3Futm_source%3Dfb"
    ))
    .call();
    assert!(pixel.is_ok(), "pixel respondeu");
    let agente = ureq::AgentBuilder::new().redirects(0).build();
    let _ = agente
        .get(&format!("{base}/consentimento?aceitar=nao"))
        .call();

    let limite = Instant::now();
    let codigo = loop {
        if let Some(status) = filho.try_wait().expect("espera") {
            break status;
        }
        if limite.elapsed() > Duration::from_secs(10) {
            let _ = filho.kill();
            panic!("o binário não saiu depois do limite de pedidos");
        }
        thread::sleep(Duration::from_millis(50));
    };
    assert!(codigo.success(), "saiu com {codigo}");
    let primeira = recebe
        .recv_timeout(Duration::from_secs(5))
        .expect("a primeira linha da saída");
    assert!(
        primeira.starts_with("Rastreio CAPI no ar em http://127.0.0.1:"),
        "saída: {primeira}"
    );
}
