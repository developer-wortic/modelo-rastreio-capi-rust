//! Envio do evento à Conversions API da Meta, por HTTPS, com reenvio.
//!
//! - Reenvia em 429, em 5xx e em falha de rede ou tempo esgotado, até `tentativas` vezes.
//!   Quando a Meta manda `Retry-After`, a espera segue esse valor (com teto de 60 segundos).
//! - Não reenvia em 4xx (exceto 429): o erro é do evento, e tentar de novo não resolve.
//! - O token vai na URL, como a Meta pede. Ele nunca aparece no `Debug`, nem em erro, nem em resultado.
//! - Só aceita `https`. `http` só passa para `127.0.0.1` e `localhost`, para teste local.

use std::fmt;
use std::thread;
use std::time::Duration;

use crate::consentimento::{self, Motivo};
use crate::evento::{self, Entrada, ErroEvento, Evento};

/// A Graph API da Meta.
pub const BASE_PADRAO: &str = "https://graph.facebook.com";
/// A versão da Graph API usada nos exemplos da documentação da Meta.
pub const VERSAO_PADRAO: &str = "v27.0";
const ESPERA_MAXIMA: Duration = Duration::from_secs(60);
const TOKEN_OCULTO: &str = "<oculto>";

/// Como enviar: de quem é o Pixel, com qual token, para onde e com quantas tentativas.
#[derive(Clone)]
pub struct Config {
    /// O ID do Pixel (só dígitos).
    pub pixel_id: String,
    /// O token de acesso da Conversions API. Nunca sai em log.
    pub token: String,
    /// A base da API, `https://graph.facebook.com` por padrão.
    pub base: String,
    /// A versão da Graph API (`v27.0`).
    pub versao: String,
    /// O código de teste da Events Manager. Quando existe, o evento aparece em Test Events.
    pub test_event_code: Option<String>,
    /// Quantas tentativas no total, a primeira inclusa.
    pub tentativas: u32,
    /// A espera base entre tentativas. Dobra a cada tentativa.
    pub espera_base: Duration,
    /// Tempo limite de cada tentativa.
    pub tempo_limite: Duration,
}

impl Config {
    /// A configuração padrão para um Pixel e um token: 3 tentativas, espera de 1 segundo, 10 segundos de limite.
    #[must_use]
    pub fn nova(pixel_id: &str, token: &str) -> Self {
        Self {
            pixel_id: pixel_id.to_string(),
            token: token.to_string(),
            base: BASE_PADRAO.to_string(),
            versao: VERSAO_PADRAO.to_string(),
            test_event_code: None,
            tentativas: 3,
            espera_base: Duration::from_secs(1),
            tempo_limite: Duration::from_secs(10),
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("pixel_id", &self.pixel_id)
            .field("token", &TOKEN_OCULTO)
            .field("base", &self.base)
            .field("versao", &self.versao)
            .field("test_event_code", &self.test_event_code)
            .field("tentativas", &self.tentativas)
            .finish_non_exhaustive()
    }
}

/// Por que a configuração não serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroConfig {
    /// O ID do Pixel está vazio ou tem algo que não é dígito.
    PixelInvalido,
    /// O token está vazio.
    TokenVazio,
    /// A base não é `https`, e não é `127.0.0.1` nem `localhost`.
    BaseInsegura,
}

impl fmt::Display for ErroConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ErroConfig::PixelInvalido => "o ID do Pixel precisa ter só dígitos",
            ErroConfig::TokenVazio => "o token de acesso está vazio",
            ErroConfig::BaseInsegura => {
                "a base da API precisa ser https (http só em 127.0.0.1 ou localhost)"
            }
        })
    }
}

/// O que aconteceu com um envio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resultado {
    /// A Meta aceitou. `recebidos` vem da resposta, quando ela traz o número.
    Enviado {
        /// Quantas tentativas foram necessárias.
        tentativas: u32,
        /// Quantos eventos a Meta disse ter recebido.
        recebidos: Option<u64>,
    },
    /// O consentimento não foi dado. Nada foi montado nem enviado.
    Recusado(Motivo),
    /// O evento não cumpre o que a Meta exige. Nada foi enviado.
    Invalido(ErroEvento),
    /// A Meta recusou (4xx) ou as tentativas acabaram. O motivo não traz o token.
    Falhou {
        /// Quantas tentativas foram feitas.
        tentativas: u32,
        /// O motivo, em texto.
        motivo: String,
    },
}

/// Um cliente da Conversions API, com o agente HTTP pronto.
pub struct Envio {
    config: Config,
    agente: ureq::Agent,
}

enum Tentativa {
    Aceita(Option<u64>),
    Recusada(String),
    Passageira(String, Option<Duration>),
}

impl Envio {
    /// Monta o cliente. Confere o Pixel, o token e a base antes de qualquer envio.
    ///
    /// # Errors
    ///
    /// Devolve um [`ErroConfig`] quando o Pixel, o token ou a base não servem.
    pub fn nova(config: Config) -> Result<Self, ErroConfig> {
        if config.pixel_id.is_empty() || !config.pixel_id.chars().all(|c| c.is_ascii_digit()) {
            return Err(ErroConfig::PixelInvalido);
        }
        if config.token.trim().is_empty() {
            return Err(ErroConfig::TokenVazio);
        }
        if !base_segura(&config.base) {
            return Err(ErroConfig::BaseInsegura);
        }
        let agente = ureq::AgentBuilder::new()
            .timeout(config.tempo_limite)
            .build();
        Ok(Self { config, agente })
    }

    /// Consentimento primeiro, montagem depois, envio por último. Sem consentimento, não monta nada.
    #[must_use]
    pub fn enviar(&self, consentido: bool, entrada: &Entrada) -> Resultado {
        if let Err(motivo) = consentimento::checar(consentido) {
            return Resultado::Recusado(motivo);
        }
        match evento::montar(entrada) {
            Ok(evento) => self.transmitir(&[evento]),
            Err(erro) => Resultado::Invalido(erro),
        }
    }

    /// Manda um lote de eventos já montados. Reenvia conforme a regra do módulo.
    #[must_use]
    pub fn transmitir(&self, eventos: &[Evento]) -> Resultado {
        let corpo = evento::corpo(eventos, self.config.test_event_code.as_deref());
        let url = self.url();
        let mut tentativa = 1;
        loop {
            match self.uma(&url, &corpo) {
                Tentativa::Aceita(recebidos) => {
                    return Resultado::Enviado {
                        tentativas: tentativa,
                        recebidos,
                    };
                }
                Tentativa::Passageira(_, espera) if tentativa < self.config.tentativas => {
                    thread::sleep(espera.unwrap_or_else(|| self.espera(tentativa)));
                }
                Tentativa::Recusada(motivo) | Tentativa::Passageira(motivo, _) => {
                    return Resultado::Falhou {
                        tentativas: tentativa,
                        motivo,
                    };
                }
            }
            tentativa += 1;
        }
    }

    fn url(&self) -> String {
        format!(
            "{}/{}/{}/events?access_token={}",
            self.config.base.trim_end_matches('/'),
            self.config.versao,
            self.config.pixel_id,
            self.config.token
        )
    }

    fn espera(&self, tentativa: u32) -> Duration {
        self.config
            .espera_base
            .saturating_mul(2u32.saturating_pow(tentativa - 1))
    }

    fn uma(&self, url: &str, corpo: &str) -> Tentativa {
        let pedido = self
            .agente
            .post(url)
            .set("content-type", "application/json"); // borda-json: a Graph API recebe JSON
        match pedido.send_string(corpo) {
            Ok(resposta) => {
                let texto = resposta.into_string().unwrap_or_default();
                Tentativa::Aceita(recebidos(&texto))
            }
            Err(ureq::Error::Status(codigo, resposta)) => {
                let espera = resposta.header("retry-after").and_then(segundos);
                let texto = resposta.into_string().unwrap_or_default();
                let motivo = format!(
                    "HTTP {codigo}: {}",
                    mensagem_da_meta(&texto, &self.config.token)
                );
                if codigo == 429 || codigo >= 500 {
                    Tentativa::Passageira(motivo, espera)
                } else {
                    Tentativa::Recusada(motivo)
                }
            }
            Err(ureq::Error::Transport(_)) => {
                Tentativa::Passageira("falha de rede ou tempo esgotado".to_string(), None)
            }
        }
    }
}

/// Se a base é segura para mandar o token: `https`, ou `http` só para o próprio computador.
#[must_use]
pub fn base_segura(base: &str) -> bool {
    if base.starts_with("https://") {
        return true;
    }
    ["http://127.0.0.1", "http://localhost"]
        .iter()
        .any(|prefixo| {
            base.strip_prefix(prefixo).is_some_and(|resto| {
                resto.is_empty() || resto.starts_with(':') || resto.starts_with('/')
            })
        })
}

fn recebidos(texto: &str) -> Option<u64> {
    let corpo: serde_json::Value = serde_json::from_str(texto).ok()?; // borda-json: resposta da Graph API
    corpo.get("events_received")?.as_u64()
}

fn mensagem_da_meta(texto: &str, token: &str) -> String {
    let corpo: Option<serde_json::Value> = serde_json::from_str(texto).ok(); // borda-json: erro da Graph API
    let mensagem = corpo
        .as_ref()
        .and_then(|c| c.get("error"))
        .and_then(|e| e.get("message"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("sem detalhe");
    let limpa: String = mensagem
        .replace(token, TOKEN_OCULTO)
        .chars()
        .take(200)
        .collect();
    limpa
}

fn segundos(valor: &str) -> Option<Duration> {
    valor
        .trim()
        .parse::<u64>()
        .ok()
        .map(|s| Duration::from_secs(s).min(ESPERA_MAXIMA))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::consentimento::Motivo;
    use crate::dados::Dados;
    use crate::suporte::{Resp, falso, resp};

    const TOKEN: &str = "TOKEN_DE_TESTE";
    const PIXEL: &str = "123456789012345";

    fn config_para(base: &str) -> Config {
        let mut config = Config::nova(PIXEL, TOKEN);
        config.base = base.to_string();
        config.espera_base = Duration::from_millis(1);
        config
    }

    fn envio_para(base: &str) -> Envio {
        Envio::nova(config_para(base)).expect("configuração de teste")
    }

    fn evento_de_teste() -> Evento {
        let entrada = Entrada {
            nome: evento::Nome::Lead,
            event_id: "lead-1",
            momento_s: 1000,
            agora_s: 1000,
            pagina: "https://loja.com/",
            dados: Dados {
                email: Some("ana@exemplo.com"),
                ..Dados::default()
            },
            ip: None,
            user_agent: None,
            fbc: None,
            fbp: None,
            valor_centavos: None,
            moeda: None,
            ddi_padrao: "55",
            enviar_ip_e_user_agent: false,
        };
        evento::montar(&entrada).expect("evento de teste")
    }

    fn entrada_valida() -> Entrada<'static> {
        Entrada {
            nome: evento::Nome::Lead,
            event_id: "lead-2",
            momento_s: 1000,
            agora_s: 1000,
            pagina: "https://loja.com/",
            dados: Dados {
                email: Some("ana@exemplo.com"),
                ..Dados::default()
            },
            ip: None,
            user_agent: None,
            fbc: None,
            fbp: None,
            valor_centavos: None,
            moeda: None,
            ddi_padrao: "55",
            enviar_ip_e_user_agent: false,
        }
    }

    #[test]
    fn configuracao_padrao_tem_as_tentativas_e_os_tempos_da_documentacao() {
        let c = Config::nova(PIXEL, TOKEN);
        assert_eq!(c.base, BASE_PADRAO);
        assert_eq!(c.versao, VERSAO_PADRAO);
        assert_eq!(c.tentativas, 3);
        assert_eq!(c.espera_base, Duration::from_secs(1));
        assert_eq!(c.tempo_limite, Duration::from_secs(10));
        assert!(c.test_event_code.is_none());
    }

    #[test]
    fn debug_da_configuracao_nunca_mostra_o_token() {
        let texto = format!("{:?}", Config::nova(PIXEL, TOKEN));
        assert!(texto.contains("<oculto>"));
        assert!(!texto.contains(TOKEN));
        assert!(texto.contains(PIXEL));
    }

    #[test]
    fn nova_recusa_pixel_token_e_base_ruins() {
        assert_eq!(
            Envio::nova(Config::nova("", TOKEN)).err(),
            Some(ErroConfig::PixelInvalido)
        );
        assert_eq!(
            Envio::nova(Config::nova("12a", TOKEN)).err(),
            Some(ErroConfig::PixelInvalido)
        );
        assert_eq!(
            Envio::nova(Config::nova(PIXEL, "   ")).err(),
            Some(ErroConfig::TokenVazio)
        );
        let mut config = Config::nova(PIXEL, TOKEN);
        config.base = "http://exemplo.com".to_string();
        assert_eq!(Envio::nova(config).err(), Some(ErroConfig::BaseInsegura));
        assert!(Envio::nova(Config::nova(PIXEL, TOKEN)).is_ok());
    }

    #[test]
    fn textos_de_erro_de_configuracao() {
        assert!(ErroConfig::PixelInvalido.to_string().contains("dígitos"));
        assert!(ErroConfig::TokenVazio.to_string().contains("vazio"));
        assert!(ErroConfig::BaseInsegura.to_string().contains("https"));
    }

    #[test]
    fn base_segura_so_aceita_https_ou_loopback() {
        assert!(base_segura("https://graph.facebook.com"));
        assert!(base_segura("http://127.0.0.1:8080"));
        assert!(base_segura("http://localhost"));
        assert!(base_segura("http://localhost/x"));
        assert!(!base_segura("http://127.0.0.10"));
        assert!(!base_segura("http://localhostx"));
        assert!(!base_segura("http://exemplo.com"));
        assert!(!base_segura("ftp://127.0.0.1"));
    }

    #[test]
    fn envio_aceito_devolve_quantos_eventos_a_meta_recebeu() {
        let falso = falso(vec![resp(200, "{\"events_received\":2}")]);
        let envio = envio_para(&falso.base);
        let r = envio.transmitir(&[evento_de_teste()]);
        assert_eq!(
            r,
            Resultado::Enviado {
                tentativas: 1,
                recebidos: Some(2)
            }
        );
        let pedidos = falso.pedidos.lock().unwrap();
        assert_eq!(pedidos.len(), 1);
        assert!(
            pedidos[0]
                .0
                .starts_with(&format!("/v27.0/{PIXEL}/events?access_token={TOKEN}"))
        );
        assert!(pedidos[0].1.contains("\"event_name\":\"Lead\""));
    }

    #[test]
    fn code_de_teste_vai_no_corpo_quando_configurado() {
        let falso = falso(vec![resp(200, "{}")]);
        let mut config = config_para(&falso.base);
        config.test_event_code = Some("TEST9".to_string());
        let envio = Envio::nova(config).expect("config");
        let _ = envio.transmitir(&[evento_de_teste()]);
        assert!(
            falso.pedidos.lock().unwrap()[0]
                .1
                .contains("\"test_event_code\":\"TEST9\"")
        );
    }

    #[test]
    fn resposta_de_sucesso_sem_numero_devolve_recebidos_none() {
        let primeiro = falso(vec![resp(200, "ok")]);
        let r = envio_para(&primeiro.base).transmitir(&[evento_de_teste()]);
        assert_eq!(
            r,
            Resultado::Enviado {
                tentativas: 1,
                recebidos: None
            }
        );
        let outro = falso(vec![resp(200, "{\"events_received\":\"muitos\"}")]);
        let r = envio_para(&outro.base).transmitir(&[evento_de_teste()]);
        assert!(matches!(
            r,
            Resultado::Enviado {
                recebidos: None,
                ..
            }
        ));
    }

    #[test]
    fn erro_4xx_nao_e_reenviado_e_o_motivo_vem_da_meta() {
        let falso = falso(vec![resp(
            400,
            "{\"error\":{\"message\":\"Parâmetro inválido\"}}",
        )]);
        let r = envio_para(&falso.base).transmitir(&[evento_de_teste()]);
        assert_eq!(
            r,
            Resultado::Falhou {
                tentativas: 1,
                motivo: "HTTP 400: Parâmetro inválido".to_string()
            }
        );
        assert_eq!(falso.pedidos.lock().unwrap().len(), 1, "4xx não reenvia");
    }

    #[test]
    fn token_que_volta_na_mensagem_da_meta_sai_oculto() {
        let falso = falso(vec![resp(
            401,
            "{\"error\":{\"message\":\"o token TOKEN_DE_TESTE expirou\"}}",
        )]);
        let r = envio_para(&falso.base).transmitir(&[evento_de_teste()]);
        assert!(
            matches!(&r, Resultado::Falhou { motivo, .. } if motivo.contains("<oculto>") && !motivo.contains(TOKEN)),
            "{r:?}"
        );
    }

    #[test]
    fn erro_429_e_reenviado_e_depois_aceito() {
        let mut limite = resp(429, "{}");
        limite.cabecalhos = vec![("retry-after", "0")];
        let falso = falso(vec![limite, resp(200, "{\"events_received\":1}")]);
        let r = envio_para(&falso.base).transmitir(&[evento_de_teste()]);
        assert_eq!(
            r,
            Resultado::Enviado {
                tentativas: 2,
                recebidos: Some(1)
            }
        );
        assert_eq!(falso.pedidos.lock().unwrap().len(), 2);
    }

    #[test]
    fn erro_429_sem_fim_esgota_as_tentativas() {
        let falso = falso(vec![resp(429, "{}")]);
        let r = envio_para(&falso.base).transmitir(&[evento_de_teste()]);
        assert!(matches!(r, Resultado::Falhou { tentativas: 3, .. }));
        assert_eq!(falso.pedidos.lock().unwrap().len(), 3);
    }

    #[test]
    fn erro_5xx_e_reenviado_ate_passar() {
        let falso = falso(vec![resp(500, "{}"), resp(503, "{}"), resp(200, "{}")]);
        let r = envio_para(&falso.base).transmitir(&[evento_de_teste()]);
        assert_eq!(
            r,
            Resultado::Enviado {
                tentativas: 3,
                recebidos: None
            }
        );
        assert_eq!(falso.pedidos.lock().unwrap().len(), 3);
    }

    #[test]
    fn cinco_xx_sem_fim_esgota_e_mostra_o_status() {
        let falso = falso(vec![resp(500, "{\"error\":{\"message\":\"caiu\"}}")]);
        let r = envio_para(&falso.base).transmitir(&[evento_de_teste()]);
        assert_eq!(
            r,
            Resultado::Falhou {
                tentativas: 3,
                motivo: "HTTP 500: caiu".to_string()
            }
        );
    }

    #[test]
    fn tempo_esgotado_e_reenviado_e_vira_falha_de_rede() {
        let lento = Resp {
            status: 200,
            corpo: "{}",
            cabecalhos: Vec::new(),
            atraso: Duration::from_millis(400),
        };
        let falso = falso(vec![lento]);
        let mut config = config_para(&falso.base);
        config.tentativas = 2;
        config.tempo_limite = Duration::from_millis(50);
        let r = Envio::nova(config)
            .expect("config")
            .transmitir(&[evento_de_teste()]);
        assert_eq!(
            r,
            Resultado::Falhou {
                tentativas: 2,
                motivo: "falha de rede ou tempo esgotado".to_string()
            }
        );
    }

    #[test]
    fn porta_fechada_e_falha_de_rede_com_as_tentativas_todas() {
        let porta = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("porta")
            .local_addr()
            .expect("endereço")
            .port();
        let r = envio_para(&format!("http://127.0.0.1:{porta}")).transmitir(&[evento_de_teste()]);
        assert!(
            matches!(r, Resultado::Falhou { tentativas: 3, .. }),
            "{r:?}"
        );
    }

    #[test]
    fn sem_consentimento_nao_monta_nem_chama_a_meta() {
        let falso = falso(vec![resp(200, "{}")]);
        let envio = envio_para(&falso.base);
        let r = envio.enviar(false, &entrada_valida());
        assert_eq!(r, Resultado::Recusado(Motivo::SemConsentimento));
        assert!(falso.pedidos.lock().unwrap().is_empty());
    }

    #[test]
    fn com_consentimento_e_entrada_valida_envia() {
        let falso = falso(vec![resp(200, "{\"events_received\":1}")]);
        let r = envio_para(&falso.base).enviar(true, &entrada_valida());
        assert!(matches!(r, Resultado::Enviado { tentativas: 1, .. }));
    }

    #[test]
    fn com_consentimento_e_entrada_invalida_nao_envia() {
        let falso = falso(vec![resp(200, "{}")]);
        let mut entrada = entrada_valida();
        entrada.event_id = "";
        let r = envio_para(&falso.base).enviar(true, &entrada);
        assert_eq!(r, Resultado::Invalido(ErroEvento::EventIdInvalido));
        assert!(falso.pedidos.lock().unwrap().is_empty());
    }

    #[test]
    fn retry_after_so_aceita_numero_e_tem_teto() {
        assert_eq!(segundos("5"), Some(Duration::from_secs(5)));
        assert_eq!(segundos(" 9999 "), Some(ESPERA_MAXIMA));
        assert_eq!(segundos("abc"), None);
        assert_eq!(segundos(""), None);
    }

    #[test]
    fn mensagem_da_meta_tem_padrao_e_corte() {
        assert_eq!(mensagem_da_meta("nao é json", TOKEN), "sem detalhe");
        assert_eq!(mensagem_da_meta("{\"x\":1}", TOKEN), "sem detalhe");
        let longa = format!("{{\"error\":{{\"message\":\"{}\"}}}}", "m".repeat(300));
        assert_eq!(mensagem_da_meta(&longa, TOKEN).chars().count(), 200);
    }

    #[test]
    fn espera_dobra_a_cada_tentativa() {
        let envio = envio_para("https://graph.facebook.com");
        assert_eq!(envio.espera(1), Duration::from_millis(1));
        assert_eq!(envio.espera(3), Duration::from_millis(4));
    }

    #[test]
    fn recebidos_so_le_numero() {
        assert_eq!(recebidos("{\"events_received\":7}"), Some(7));
        assert_eq!(recebidos("{}"), None);
        assert_eq!(recebidos("não é json"), None);
    }
}
