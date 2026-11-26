//! rastreio-capi: rastreio de campanhas e Conversions API da Meta, em Rust.
//!
//! A biblioteca faz quatro coisas, separadas em módulos:
//! - [`captura`]: lê UTM e clids (fbclid, gclid, ttclid, msclkid) de uma URL, com a URL tratada como não confiável.
//! - [`atribuicao`] e [`cookie`]: guarda o primeiro e o último toque num cookie assinado (HMAC-SHA256), com prazo.
//! - [`dados`] e [`evento`]: normaliza e faz o hash SHA-256 dos dados do cliente, e monta o evento da Conversions API.
//! - [`envio`] e [`consentimento`]: manda por HTTPS, com reenvio em 429, 5xx e falha de rede, e nada sai sem consentimento.
//!
//! O módulo [`modelo`] junta tudo num servidor HTTP pequeno, com a tela em [`painel`].
//!
//! # Uso
//!
//! Da URL de entrada até o evento montado, sem nenhuma chamada de rede. O envio à Meta fica em
//! [`envio::Envio::enviar`], que só manda com consentimento, Pixel e token.
//!
//! ```
//! use rastreio_capi::atribuicao::Atribuicao;
//! use rastreio_capi::captura::{self, Plataforma};
//! use rastreio_capi::cookie::Cookie;
//! use rastreio_capi::dados::Dados;
//! use rastreio_capi::evento::{self, Entrada, Nome};
//! use rastreio_capi::identificadores::{self, INDICE_PADRAO};
//!
//! // 1. a visita chega com UTM e clid; a atribuição guarda o primeiro e o último toque
//! let agora_ms = 1_700_000_000_000;
//! let mut atribuicao = Atribuicao::default();
//! atribuicao.registrar(
//!     captura::de_url("https://loja.com.br/oferta?utm_source=facebook&fbclid=AbC123"),
//!     agora_ms,
//! );
//!
//! // 2. o cookie é assinado: alterado ou vencido, não é lido
//! let cookie = Cookie::new(b"troque-por-um-segredo-com-32-bytes-ou-mais", 90).expect("chave");
//! let valor = cookie.gravar(&atribuicao);
//! let lida = cookie.ler(&valor, agora_ms + 1_000).expect("cookie válido");
//!
//! // 3. na venda, o fbc vem do cookie _fbc ou do fbclid do último toque
//! let fbclid = lida
//!     .ultimo
//!     .as_ref()
//!     .and_then(|toque| toque.captura.clid(Plataforma::Meta));
//! let fbc = identificadores::fbc_escolhido(None, fbclid, agora_ms, INDICE_PADRAO);
//!
//! // 4. o evento sai em hash para o que é dado pessoal, e em claro só para o que a Meta pede em claro
//! let entrada = Entrada {
//!     nome: Nome::Purchase,
//!     event_id: "pedido-1001",
//!     momento_s: agora_ms / 1_000,
//!     agora_s: agora_ms / 1_000 + 5,
//!     pagina: "https://loja.com.br/obrigado?pedido=1001",
//!     dados: Dados {
//!         email: Some("Ana@Exemplo.com"),
//!         telefone: Some("(11) 91234-5678"),
//!         ..Dados::default()
//!     },
//!     ip: None,
//!     user_agent: None,
//!     fbc: fbc.as_deref(),
//!     fbp: None,
//!     valor_centavos: Some(19_990),
//!     moeda: Some("BRL"),
//!     ddi_padrao: "55",
//!     enviar_ip_e_user_agent: true,
//! };
//! let evento = evento::montar(&entrada).expect("evento");
//! assert_eq!(evento.event_name, "Purchase");
//! assert_eq!(evento.event_source_url, "https://loja.com.br/obrigado");
//! assert_eq!(evento.user_data.em.as_ref().map(Vec::len), Some(1));
//! assert_eq!(evento.user_data.fbc.as_deref(), Some("fb.1.1700000000000.AbC123"));
//! ```

pub mod atribuicao;
pub mod captura;
pub mod consentimento;
pub mod cookie;
pub mod dados;
pub mod envio;
pub mod evento;
pub mod identificadores;
pub mod modelo;
pub mod painel;
#[cfg(test)]
mod suporte;
