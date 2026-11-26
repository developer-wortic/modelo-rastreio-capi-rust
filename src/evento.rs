//! O evento da Conversions API da Meta: montagem no formato da documentação.
//!
//! Campos do evento: `event_name`, `event_time` (segundos), `event_id` (desduplica com o Pixel),
//! `action_source` (`website`), `event_source_url` (a página, sem consulta), `user_data` e `custom_data`.
//! Em `user_data`, os dados do cliente vão em hash SHA-256 (`em`, `ph`, `fn`, `ln`, `db`, `ct`, `st`, `zp`,
//! `country`, `external_id`). IP, user agent, `fbc` e `fbp` vão em claro, como a Meta pede para eles.

use std::fmt;
use std::net::IpAddr;

use serde::Serialize;

use crate::captura;
use crate::dados::{Dados, Normalizados, sha256_hex};

/// Janela que a Meta aceita para `event_time`: 7 dias, em segundos.
pub const JANELA_S: u64 = 7 * 86_400;
const MAX_EVENT_ID: usize = 256;
const MAX_USER_AGENT: usize = 512;

/// Os eventos que este modelo sabe montar. São nomes padrão da Meta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nome {
    /// Compra concluída.
    Purchase,
    /// Lead captado.
    Lead,
    /// Início de checkout.
    InitiateCheckout,
    /// Página vista.
    PageView,
}

impl Nome {
    /// Todos os nomes, na ordem do painel.
    pub const TODOS: [Nome; 4] = [
        Nome::Purchase,
        Nome::Lead,
        Nome::InitiateCheckout,
        Nome::PageView,
    ];

    /// O nome exatamente como a Meta espera (`Purchase`, `Lead`, `InitiateCheckout`, `PageView`).
    #[must_use]
    pub fn como_texto(self) -> &'static str {
        match self {
            Nome::Purchase => "Purchase",
            Nome::Lead => "Lead",
            Nome::InitiateCheckout => "InitiateCheckout",
            Nome::PageView => "PageView",
        }
    }

    /// O nome a partir do texto, se for um dos que o modelo monta.
    #[must_use]
    pub fn de_texto(texto: &str) -> Option<Nome> {
        Nome::TODOS.into_iter().find(|n| n.como_texto() == texto)
    }
}

/// O `user_data` do evento. Os campos hasheados são listas, como na documentação da Meta.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct UserData {
    /// E-mail em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub em: Option<Vec<String>>,
    /// Telefone em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ph: Option<Vec<String>>,
    /// Primeiro nome em hash.
    #[serde(rename = "fn", skip_serializing_if = "Option::is_none")]
    pub fn_: Option<Vec<String>>,
    /// Sobrenome em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ln: Option<Vec<String>>,
    /// Data de nascimento em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db: Option<Vec<String>>,
    /// Cidade em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ct: Option<Vec<String>>,
    /// Estado em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub st: Option<Vec<String>>,
    /// CEP em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zp: Option<Vec<String>>,
    /// País em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<Vec<String>>,
    /// Identificador do cliente em hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<Vec<String>>,
    /// IP do visitante, em claro.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ip_address: Option<String>,
    /// User agent do visitante, em claro.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_user_agent: Option<String>,
    /// Identificador de clique da Meta (`fbc`), em claro.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fbc: Option<String>,
    /// Identificador de navegador da Meta (`fbp`), em claro.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fbp: Option<String>,
}

impl UserData {
    /// Monta o `user_data` a partir dos dados normalizados e dos identificadores em claro.
    #[must_use]
    pub fn de(
        n: &Normalizados,
        ip: Option<String>,
        user_agent: Option<String>,
        fbc: Option<&str>,
        fbp: Option<&str>,
    ) -> Self {
        Self {
            em: lista_hash(n.em.as_deref()),
            ph: lista_hash(n.ph.as_deref()),
            fn_: lista_hash(n.fn_.as_deref()),
            ln: lista_hash(n.ln.as_deref()),
            db: lista_hash(n.db.as_deref()),
            ct: lista_hash(n.ct.as_deref()),
            st: lista_hash(n.st.as_deref()),
            zp: lista_hash(n.zp.as_deref()),
            country: lista_hash(n.country.as_deref()),
            external_id: lista_hash(n.external_id.as_deref()),
            client_ip_address: ip,
            client_user_agent: user_agent,
            fbc: fbc.map(str::to_string),
            fbp: fbp.map(str::to_string),
        }
    }

    /// Se nenhum identificador entrou. A Meta exige pelo menos um.
    #[must_use]
    pub fn vazio(&self) -> bool {
        self == &UserData::default()
    }
}

/// Os valores monetários do evento. `value` é em reais (ou na moeda dada), `currency` é o código ISO.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CustomData {
    /// Valor da conversão.
    pub value: f64,
    /// Código ISO de três letras, maiúsculo (`BRL`).
    pub currency: String,
}

/// Um evento pronto para envio, no formato da Conversions API.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Evento {
    /// Nome do evento (`Purchase`, `Lead`, `InitiateCheckout` ou `PageView`).
    pub event_name: &'static str,
    /// Horário em que o evento aconteceu, em segundos desde a época Unix.
    pub event_time: u64,
    /// Identificador único do evento. Igual ao `eventID` do Pixel, para desduplicar.
    pub event_id: String,
    /// Onde a conversão aconteceu. Este modelo só monta `website`.
    pub action_source: &'static str,
    /// A página onde o evento aconteceu, sem a consulta.
    pub event_source_url: String,
    /// Os dados do cliente, em hash, e os identificadores.
    pub user_data: UserData,
    /// Valor e moeda, quando o evento tem valor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<CustomData>,
}

/// O que é preciso para montar um evento. Os dados do cliente entram crus e saem em hash.
#[derive(Debug, Clone, Copy)]
pub struct Entrada<'a> {
    /// Qual evento é.
    pub nome: Nome,
    /// Identificador único do evento (número do pedido, por exemplo).
    pub event_id: &'a str,
    /// Quando o evento aconteceu, em segundos.
    pub momento_s: u64,
    /// Agora, em segundos. Serve para checar a janela de 7 dias.
    pub agora_s: u64,
    /// A página onde o evento aconteceu. Entra sem a consulta.
    pub pagina: &'a str,
    /// Os dados do cliente, ainda sem normalizar.
    pub dados: Dados<'a>,
    /// IP do visitante, se houver. Só entra se `enviar_ip_e_user_agent` for verdadeiro.
    pub ip: Option<&'a str>,
    /// User agent do visitante, se houver. Só entra se `enviar_ip_e_user_agent` for verdadeiro.
    pub user_agent: Option<&'a str>,
    /// O `fbc` do clique, já montado ou lido do cookie.
    pub fbc: Option<&'a str>,
    /// O `fbp` do navegador, se houver.
    pub fbp: Option<&'a str>,
    /// Valor em centavos. Sem valor, o evento não leva `custom_data`.
    pub valor_centavos: Option<u32>,
    /// Código ISO da moeda (`BRL`). Obrigatório quando há valor.
    pub moeda: Option<&'a str>,
    /// O DDI que entra quando o telefone não traz um (`55`).
    pub ddi_padrao: &'a str,
    /// Se IP e user agent podem ir à Meta. Desligue quando a base legal não cobrir isso.
    pub enviar_ip_e_user_agent: bool,
}

/// O motivo de o evento não poder ser montado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroEvento {
    /// O `event_id` está vazio, longo demais ou com caractere de controle.
    EventIdInvalido,
    /// O horário é mais antigo que 7 dias. A Meta recusa o lote inteiro nesse caso.
    Antigo,
    /// A página não é uma URL `http` ou `https` válida.
    PaginaInvalida,
    /// Tem valor, mas a moeda não é um código de três letras.
    MoedaInvalida,
    /// Não sobrou nenhum identificador do cliente. A Meta exige pelo menos um.
    SemIdentificador,
}

impl fmt::Display for ErroEvento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let texto = match self {
            ErroEvento::EventIdInvalido => {
                "event_id vazio, longo demais ou com caractere de controle"
            }
            ErroEvento::Antigo => "o horário do evento passa de 7 dias; a Meta recusa o lote",
            ErroEvento::PaginaInvalida => "a página precisa ser uma URL http ou https",
            ErroEvento::MoedaInvalida => "a moeda precisa ter três letras, como BRL",
            ErroEvento::SemIdentificador => "nenhum dado do cliente ou identificador para enviar",
        };
        f.write_str(texto)
    }
}

/// Monta o evento. Não faz rede e não guarda nada.
///
/// # Errors
///
/// Devolve um [`ErroEvento`] quando o evento não cumpre o que a Meta exige.
pub fn montar(entrada: &Entrada) -> Result<Evento, ErroEvento> {
    if !event_id_valido(entrada.event_id) {
        return Err(ErroEvento::EventIdInvalido);
    }
    if entrada.agora_s.saturating_sub(entrada.momento_s) > JANELA_S {
        return Err(ErroEvento::Antigo);
    }
    let event_source_url =
        captura::url_sem_consulta(entrada.pagina).ok_or(ErroEvento::PaginaInvalida)?;
    let custom_data = match entrada.valor_centavos {
        None => None,
        Some(centavos) => {
            let currency = entrada
                .moeda
                .and_then(moeda_valida)
                .ok_or(ErroEvento::MoedaInvalida)?;
            Some(CustomData {
                value: f64::from(centavos) / 100.0,
                currency,
            })
        }
    };
    let (ip, user_agent) = if entrada.enviar_ip_e_user_agent {
        (
            entrada.ip.and_then(ip_valido),
            entrada.user_agent.and_then(user_agent_limpo),
        )
    } else {
        (None, None)
    };
    let normalizados = entrada.dados.normalizados(entrada.ddi_padrao);
    let user_data = UserData::de(&normalizados, ip, user_agent, entrada.fbc, entrada.fbp);
    if user_data.vazio() {
        return Err(ErroEvento::SemIdentificador);
    }
    Ok(Evento {
        event_name: entrada.nome.como_texto(),
        event_time: entrada.momento_s,
        event_id: entrada.event_id.to_string(),
        action_source: "website",
        event_source_url,
        user_data,
        custom_data,
    })
}

/// O corpo da requisição de envio: `data` com os eventos e, em teste, o `test_event_code`.
///
/// # Panics
///
/// Não entra em pânico: a estrutura sempre serializa.
#[must_use]
pub fn corpo(eventos: &[Evento], test_event_code: Option<&str>) -> String {
    #[derive(Serialize)]
    struct Lote<'a> {
        data: &'a [Evento],
        #[serde(skip_serializing_if = "Option::is_none")]
        test_event_code: Option<&'a str>,
    }
    serde_json::to_string(&Lote {
        // borda-json: corpo da Graph API da Meta, que só fala JSON
        data: eventos,
        test_event_code,
    })
    .expect("o lote de eventos sempre serializa")
}

/// Converte um valor digitado (`123,45`, `123.45` ou `123`) em centavos. Mais de duas casas decimais não entra.
#[must_use]
pub fn centavos(valor: &str) -> Option<u32> {
    let texto = valor.trim().replace(',', ".");
    let (inteiro, fracao) = texto.split_once('.').unwrap_or((texto.as_str(), ""));
    let inteiro_ok = !inteiro.is_empty() && inteiro.chars().all(|c| c.is_ascii_digit());
    let fracao_ok = fracao.len() <= 2 && fracao.chars().all(|c| c.is_ascii_digit());
    if !inteiro_ok || !fracao_ok {
        return None;
    }
    let reais: u32 = inteiro.parse().ok()?;
    let cent: u32 = if fracao.is_empty() {
        0
    } else {
        format!("{fracao:0<2}").parse().ok()?
    };
    reais.checked_mul(100)?.checked_add(cent)
}

fn lista_hash(valor: Option<&str>) -> Option<Vec<String>> {
    valor.map(|v| vec![sha256_hex(v)])
}

fn event_id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_EVENT_ID && !id.chars().any(char::is_control)
}

fn moeda_valida(moeda: &str) -> Option<String> {
    let maiuscula = moeda.trim().to_ascii_uppercase();
    (maiuscula.len() == 3 && maiuscula.chars().all(|c| c.is_ascii_alphabetic()))
        .then_some(maiuscula)
}

fn ip_valido(ip: &str) -> Option<String> {
    ip.trim().parse::<IpAddr>().ok().map(|i| i.to_string())
}

fn user_agent_limpo(texto: &str) -> Option<String> {
    let limpo: String = texto
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_USER_AGENT)
        .collect();
    let aparado = limpo.trim();
    (!aparado.is_empty()).then(|| aparado.to_string())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::dados::sha256_hex;

    const MOMENTO: u64 = 1_633_552_688;

    /// O exemplo da documentação da Meta: Purchase, com e-mail, telefone, nome, IP e user agent.
    fn entrada_completa<'a>() -> Entrada<'a> {
        Entrada {
            nome: Nome::Purchase,
            event_id: "event.id.123",
            momento_s: MOMENTO,
            agora_s: MOMENTO + 10,
            pagina: "https://jaspers-market.com/product/123?utm_source=x#topo",
            dados: Dados {
                email: Some("John_Smith@gmail.com"),
                telefone: Some("(650)555-1212"),
                nome: Some("Mary"),
                pais: Some("United States"),
                ..Dados::default()
            },
            ip: Some("192.19.9.9"),
            user_agent: Some("test ua"),
            fbc: Some("fb.1.1554763741205.AbCdEfGhIjKlMnOpQrStUvWxYz1234567890"),
            fbp: Some("fb.1.1558571054389.1098115397"),
            valor_centavos: Some(10_020),
            moeda: Some("usd"),
            ddi_padrao: "1",
            enviar_ip_e_user_agent: true,
        }
    }

    #[test]
    fn nomes_do_evento_sao_os_da_meta() {
        assert_eq!(Nome::Purchase.como_texto(), "Purchase");
        assert_eq!(Nome::Lead.como_texto(), "Lead");
        assert_eq!(Nome::InitiateCheckout.como_texto(), "InitiateCheckout");
        assert_eq!(Nome::PageView.como_texto(), "PageView");
        assert_eq!(Nome::de_texto("Lead"), Some(Nome::Lead));
        assert_eq!(Nome::de_texto("lead"), None);
        assert_eq!(Nome::TODOS.len(), 4);
    }

    #[test]
    fn evento_completo_bate_campo_a_campo_com_a_documentacao() {
        let evento = montar(&entrada_completa()).expect("monta");
        assert_eq!(evento.event_name, "Purchase");
        assert_eq!(evento.event_time, MOMENTO);
        assert_eq!(evento.event_id, "event.id.123");
        assert_eq!(evento.action_source, "website");
        assert_eq!(
            evento.event_source_url,
            "https://jaspers-market.com/product/123"
        );
        let u = &evento.user_data;
        assert_eq!(u.em, Some(vec![sha256_hex("john_smith@gmail.com")]));
        assert_eq!(u.ph, Some(vec![sha256_hex("16505551212")]));
        assert_eq!(u.fn_, Some(vec![sha256_hex("mary")]));
        assert_eq!(u.country, Some(vec![sha256_hex("us")]));
        assert_eq!(u.ln, None);
        assert_eq!(u.client_ip_address.as_deref(), Some("192.19.9.9"));
        assert_eq!(u.client_user_agent.as_deref(), Some("test ua"));
        assert_eq!(
            u.fbc.as_deref(),
            Some("fb.1.1554763741205.AbCdEfGhIjKlMnOpQrStUvWxYz1234567890")
        );
        assert_eq!(u.fbp.as_deref(), Some("fb.1.1558571054389.1098115397"));
        assert_eq!(
            evento.custom_data,
            Some(CustomData {
                value: 100.2,
                currency: "USD".to_string()
            })
        );
    }

    #[test]
    fn corpo_leva_o_formato_da_documentacao_e_o_test_event_code() {
        let evento = montar(&entrada_completa()).expect("monta");
        let corpo = corpo(std::slice::from_ref(&evento), Some("TEST123"));
        assert!(corpo.starts_with("{\"data\":[{"));
        assert!(corpo.contains("\"event_name\":\"Purchase\""));
        assert!(corpo.contains("\"event_time\":1633552688"));
        assert!(corpo.contains("\"event_id\":\"event.id.123\""));
        assert!(corpo.contains("\"action_source\":\"website\""));
        assert!(corpo.contains(&format!(
            "\"em\":[\"{}\"]",
            sha256_hex("john_smith@gmail.com")
        )));
        assert!(corpo.contains("\"fn\":["), "fn é o nome do campo na Meta");
        assert!(corpo.contains("\"client_ip_address\":\"192.19.9.9\""));
        assert!(corpo.contains("\"custom_data\":{\"value\":100.2,\"currency\":\"USD\"}"));
        assert!(corpo.contains("\"test_event_code\":\"TEST123\""));
        assert!(!corpo.contains("John_Smith"), "o dado cru nunca sai");
        assert!(
            !corpo.contains("jaspers-market.com/product/123?utm"),
            "a consulta não sai"
        );
    }

    #[test]
    fn corpo_sem_test_event_code_nao_manda_o_campo() {
        let evento = montar(&entrada_completa()).expect("monta");
        assert!(!corpo(&[evento], None).contains("test_event_code"));
    }

    #[test]
    fn lead_sem_valor_nao_leva_custom_data() {
        let mut entrada = entrada_completa();
        entrada.nome = Nome::Lead;
        entrada.valor_centavos = None;
        let evento = montar(&entrada).expect("monta");
        assert_eq!(evento.custom_data, None);
        assert!(!corpo(&[evento], None).contains("custom_data"));
    }

    #[test]
    fn page_view_so_com_fbp_ja_serve() {
        let entrada = Entrada {
            nome: Nome::PageView,
            event_id: "pv-1",
            momento_s: MOMENTO,
            agora_s: MOMENTO,
            pagina: "https://a.com/",
            dados: Dados::default(),
            ip: None,
            user_agent: None,
            fbc: None,
            fbp: Some("fb.1.1558571054389.1098115397"),
            valor_centavos: None,
            moeda: None,
            ddi_padrao: "55",
            enviar_ip_e_user_agent: true,
        };
        let evento = montar(&entrada).expect("monta");
        assert_eq!(
            evento.user_data.fbp.as_deref(),
            Some("fb.1.1558571054389.1098115397")
        );
    }

    #[test]
    fn sem_nenhum_identificador_nao_monta() {
        let mut entrada = entrada_completa();
        entrada.dados = Dados::default();
        entrada.ip = None;
        entrada.user_agent = None;
        entrada.fbc = None;
        entrada.fbp = None;
        assert_eq!(montar(&entrada), Err(ErroEvento::SemIdentificador));
        entrada.ip = Some("192.19.9.9");
        entrada.enviar_ip_e_user_agent = false;
        assert_eq!(montar(&entrada), Err(ErroEvento::SemIdentificador));
    }

    #[test]
    fn event_id_vazio_ou_com_controle_ou_longo_nao_monta() {
        let mut entrada = entrada_completa();
        entrada.event_id = "";
        assert_eq!(montar(&entrada), Err(ErroEvento::EventIdInvalido));
        entrada.event_id = "a\nb";
        assert_eq!(montar(&entrada), Err(ErroEvento::EventIdInvalido));
        let longo = "x".repeat(MAX_EVENT_ID + 1);
        entrada.event_id = &longo;
        assert_eq!(montar(&entrada), Err(ErroEvento::EventIdInvalido));
    }

    #[test]
    fn evento_com_mais_de_sete_dias_nao_monta_e_com_sete_dias_monta() {
        let mut entrada = entrada_completa();
        entrada.agora_s = entrada.momento_s + JANELA_S + 1;
        assert_eq!(montar(&entrada), Err(ErroEvento::Antigo));
        entrada.agora_s = entrada.momento_s + JANELA_S;
        assert!(montar(&entrada).is_ok());
    }

    #[test]
    fn pagina_que_nao_e_http_nao_monta() {
        let mut entrada = entrada_completa();
        entrada.pagina = "javascript:alert(1)";
        assert_eq!(montar(&entrada), Err(ErroEvento::PaginaInvalida));
    }

    #[test]
    fn moeda_precisa_ter_tres_letras_quando_ha_valor() {
        let mut entrada = entrada_completa();
        entrada.moeda = None;
        assert_eq!(montar(&entrada), Err(ErroEvento::MoedaInvalida));
        entrada.moeda = Some("BR1");
        assert_eq!(montar(&entrada), Err(ErroEvento::MoedaInvalida));
        entrada.moeda = Some("BRLX");
        assert_eq!(montar(&entrada), Err(ErroEvento::MoedaInvalida));
        entrada.moeda = Some(" brl ");
        assert_eq!(
            montar(&entrada)
                .expect("monta")
                .custom_data
                .expect("valor")
                .currency,
            "BRL"
        );
    }

    #[test]
    fn ip_invalido_fica_de_fora_e_user_agent_e_limpo_e_cortado() {
        let mut entrada = entrada_completa();
        entrada.ip = Some("não é ip");
        let longo = format!("Mozilla\u{7}/5.0 {}", "x".repeat(MAX_USER_AGENT));
        entrada.user_agent = Some(&longo);
        let evento = montar(&entrada).expect("monta");
        assert_eq!(evento.user_data.client_ip_address, None);
        let ua = evento.user_data.client_user_agent.expect("ua");
        assert!(ua.starts_with("Mozilla/5.0"));
        assert!(!ua.contains('\u{7}'));
        assert_eq!(ua.chars().count(), MAX_USER_AGENT);
        entrada.user_agent = Some("   ");
        assert_eq!(
            montar(&entrada).expect("monta").user_data.client_user_agent,
            None
        );
    }

    #[test]
    fn cada_erro_tem_a_sua_mensagem() {
        assert!(ErroEvento::EventIdInvalido.to_string().contains("event_id"));
        assert!(ErroEvento::Antigo.to_string().contains("7 dias"));
        assert!(ErroEvento::PaginaInvalida.to_string().contains("http"));
        assert!(ErroEvento::MoedaInvalida.to_string().contains("moeda"));
        assert!(ErroEvento::SemIdentificador.to_string().contains("nenhum"));
    }

    #[test]
    fn centavos_le_os_jeitos_comuns_de_digitar_valor() {
        assert_eq!(centavos("99,90"), Some(9990));
        assert_eq!(centavos("99.9"), Some(9990));
        assert_eq!(centavos("7"), Some(700));
        assert_eq!(centavos("0,05"), Some(5));
        assert_eq!(centavos("5,"), Some(500));
        assert_eq!(centavos(" 12 "), Some(1200));
        assert_eq!(centavos("1,234"), None);
        assert_eq!(centavos("abc"), None);
        assert_eq!(centavos(""), None);
        assert_eq!(centavos(",5"), None);
        assert_eq!(centavos("1.2.3"), None);
        assert_eq!(centavos("4294967295"), None);
    }

    #[test]
    fn user_data_vazio_e_detectado() {
        assert!(UserData::default().vazio());
        let u = UserData {
            fbp: Some("x".to_string()),
            ..UserData::default()
        };
        assert!(!u.vazio());
    }
}
