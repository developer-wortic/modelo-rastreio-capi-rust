//! Suporte dos testes: um servidor falso que faz de conta que é a Graph API da Meta, e um cliente HTTP de teste.
//! Nada sai da máquina. Os testes são determinísticos e não mandam evento a ninguém.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tiny_http::{Header, Response, Server};

use crate::cookie;

/// Uma resposta do servidor falso: status, corpo, cabeçalhos extras e um atraso antes de responder.
#[derive(Clone)]
pub struct Resp {
    pub status: u16,
    pub corpo: &'static str,
    pub cabecalhos: Vec<(&'static str, &'static str)>,
    pub atraso: Duration,
}

/// Uma resposta simples, sem cabeçalho extra e sem atraso.
pub fn resp(status: u16, corpo: &'static str) -> Resp {
    Resp {
        status,
        corpo,
        cabecalhos: Vec::new(),
        atraso: Duration::ZERO,
    }
}

/// Um pedido recebido pelo servidor falso: o caminho com a consulta, e o corpo.
pub type Pedido = (String, String);

/// O servidor falso: a base para usar no lugar da Graph API, e o registro dos pedidos.
pub struct Falso {
    pub base: String,
    pub pedidos: Arc<Mutex<Vec<Pedido>>>,
    servidor: Arc<Server>,
}

impl Drop for Falso {
    fn drop(&mut self) {
        self.servidor.unblock();
    }
}

/// Sobe um servidor falso numa porta livre. Cada pedido recebe a próxima resposta do roteiro;
/// quando o roteiro acaba, repete a última.
pub fn falso(roteiro: Vec<Resp>) -> Falso {
    let servidor = Arc::new(Server::http("127.0.0.1:0").expect("porta livre"));
    let base = format!(
        "http://{}",
        servidor.server_addr().to_ip().expect("endereço IP")
    );
    let pedidos = Arc::new(Mutex::new(Vec::new()));
    let (no_fio, gravados) = (servidor.clone(), pedidos.clone());
    thread::spawn(move || {
        for (indice, mut pedido) in no_fio.incoming_requests().enumerate() {
            let url = pedido.url().to_string();
            let mut corpo = String::new();
            let _ = pedido.as_reader().read_to_string(&mut corpo);
            gravados.lock().unwrap().push((url, corpo));
            let escolhido = roteiro[indice.min(roteiro.len() - 1)].clone();
            thread::sleep(escolhido.atraso);
            let mut saida =
                Response::from_string(escolhido.corpo).with_status_code(escolhido.status);
            for (nome, valor) in escolhido.cabecalhos {
                saida =
                    saida.with_header(Header::from_bytes(nome, valor).expect("cabeçalho válido"));
            }
            let _ = pedido.respond(saida);
        }
    });
    Falso {
        base,
        pedidos,
        servidor,
    }
}

/// O que o teste viu da resposta.
pub struct Visto {
    pub status: u16,
    pub corpo: String,
    pub tipo: Option<String>,
    cabecalhos: Vec<(String, String)>,
}

impl Visto {
    /// O primeiro valor do cabeçalho com esse nome (sem diferenciar maiúscula).
    pub fn cabecalho(&self, nome: &str) -> Option<String> {
        self.cabecalhos
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nome))
            .map(|(_, v)| v.clone())
    }

    /// A linha `Set-Cookie` completa de um cookie com esse nome.
    pub fn cookie_por_nome(&self, nome: &str) -> Option<String> {
        let inicio = format!("{nome}=");
        self.cabecalhos
            .iter()
            .find(|(n, v)| n.eq_ignore_ascii_case("set-cookie") && v.starts_with(&inicio))
            .map(|(_, v)| v.clone())
    }

    /// O valor do cookie de atribuição (`rc_atrib`) que veio em `Set-Cookie`, sem os atributos.
    pub fn cookie_rc_atrib(&self) -> Option<String> {
        let linha = self.cookie_por_nome(cookie::NOME)?;
        let par = linha.split(';').next()?;
        par.split_once('=').map(|(_, valor)| valor.to_string())
    }
}

/// GET de verdade contra um endereço local. Os cookies viram um só cabeçalho `Cookie`.
/// Um agente que não segue redirecionamento: o 303 do consentimento precisa aparecer como veio.
fn agente() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(5))
        .build()
}

/// GET de verdade contra um endereço local. Os cookies viram um só cabeçalho `Cookie`.
pub fn pedir_get(url: &str, cookies: &[&String]) -> Visto {
    let mut pedido = agente().get(url);
    if !cookies.is_empty() {
        pedido = pedido.set("Cookie", &junta(cookies));
    }
    enviar(pedido, None)
}

/// POST de formulário contra um endereço local, com os cookies dados.
pub fn pedir_form(url: &str, corpo: &str, cookies: &[&String]) -> Visto {
    let mut pedido = agente()
        .post(url)
        .set("Content-Type", "application/x-www-form-urlencoded");
    if !cookies.is_empty() {
        pedido = pedido.set("Cookie", &junta(cookies));
    }
    enviar(pedido, Some(corpo))
}

fn junta(cookies: &[&String]) -> String {
    cookies
        .iter()
        .map(|c| c.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn enviar(pedido: ureq::Request, corpo: Option<&str>) -> Visto {
    let resultado = match corpo {
        Some(texto) => pedido.send_string(texto),
        None => pedido.call(),
    };
    let resposta = match resultado {
        Ok(r) => r,
        Err(erro) => erro.into_response().expect("resposta do servidor de teste"),
    };
    let status = resposta.status();
    let tipo = resposta.header("content-type").map(str::to_string);
    let mut cabecalhos = Vec::new();
    for nome in resposta.headers_names() {
        for valor in resposta.all(&nome) {
            cabecalhos.push((nome.clone(), valor.to_string()));
        }
    }
    let corpo = resposta.into_string().unwrap_or_default();
    Visto {
        status,
        corpo,
        tipo,
        cabecalhos,
    }
}
