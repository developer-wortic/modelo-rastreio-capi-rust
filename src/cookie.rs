//! Cookie primário de atribuição: guarda o primeiro e o último toque, assinado com HMAC-SHA256 e com prazo.
//!
//! O conteúdo é texto próprio, sem formato de troca: um registro por toque (`p` ou `u`), campos separados
//! por um caractere de controle. Os valores que entram nunca têm caractere de controle, então a separação
//! não se confunde. O texto vai em base64url, seguido de ponto e da assinatura em base64url.
//! Cookie adulterado, assinado com outra chave, malformado ou vencido é recusado como se não existisse.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::atribuicao::{Atribuicao, Toque};
use crate::captura::{Captura, Clid, Plataforma, Utm};

/// Nome do cookie no navegador.
pub const NOME: &str = "rc_atrib";
/// Tamanho mínimo da chave de assinatura, em bytes (256 bits).
pub const CHAVE_MINIMA: usize = 32;
const DIA_MS: u64 = 86_400_000;
const CAMPO: &str = "\u{1f}";
const REGISTRO: &str = "\n";
/// Campos de um registro: rótulo, horário, cinco UTM e uma posição por plataforma.
const CAMPOS: usize = 2 + 5 + 4;

type HmacSha256 = Hmac<Sha256>;

/// O motivo de não montar o cookie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroCookie {
    /// A chave tem menos de [`CHAVE_MINIMA`] bytes.
    ChaveCurta,
}

/// O cookie de atribuição: a chave de assinatura e o prazo.
pub struct Cookie {
    chave: Vec<u8>,
    prazo_ms: u64,
}

impl Cookie {
    /// Um cookie com a chave dada e o prazo em dias. A chave precisa ter pelo menos 32 bytes.
    ///
    /// # Errors
    ///
    /// Devolve [`ErroCookie::ChaveCurta`] quando a chave tem menos de 32 bytes.
    pub fn new(chave: &[u8], prazo_dias: u64) -> Result<Self, ErroCookie> {
        if chave.len() < CHAVE_MINIMA {
            return Err(ErroCookie::ChaveCurta);
        }
        Ok(Self {
            chave: chave.to_vec(),
            prazo_ms: prazo_dias.saturating_mul(DIA_MS),
        })
    }

    /// O valor a gravar no cookie, assinado.
    #[must_use]
    pub fn gravar(&self, atribuicao: &Atribuicao) -> String {
        let dados = codificar(atribuicao);
        let assinatura = self.assinar(dados.as_bytes());
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(dados.as_bytes()),
            URL_SAFE_NO_PAD.encode(assinatura)
        )
    }

    /// Lê o valor do cookie. Devolve `None` se a assinatura não bate, se o conteúdo não é uma
    /// atribuição ou se o último toque já passou do prazo em `agora_ms`.
    #[must_use]
    pub fn ler(&self, valor: &str, agora_ms: u64) -> Option<Atribuicao> {
        let (corpo, assinatura) = valor.split_once('.')?;
        let dados = URL_SAFE_NO_PAD.decode(corpo).ok()?;
        let assinatura = URL_SAFE_NO_PAD.decode(assinatura).ok()?;
        self.mac(&dados).verify_slice(&assinatura).ok()?;
        let texto = String::from_utf8(dados).ok()?;
        let atribuicao = decodificar(&texto)?;
        atribuicao
            .vigente(agora_ms, self.prazo_ms)
            .then_some(atribuicao)
    }

    /// A linha `Set-Cookie` completa: caminho, prazo, `HttpOnly`, `SameSite=Lax` e `Secure` quando o
    /// site roda em HTTPS.
    #[must_use]
    pub fn cabecalho(&self, valor: &str, seguro: bool) -> String {
        let segundos = self.prazo_ms / 1000;
        let secure = if seguro { "; Secure" } else { "" };
        format!("{NOME}={valor}; Path=/; Max-Age={segundos}; HttpOnly; SameSite=Lax{secure}")
    }

    fn mac(&self, dados: &[u8]) -> HmacSha256 {
        let mut mac =
            HmacSha256::new_from_slice(&self.chave).expect("HMAC aceita chave de qualquer tamanho");
        mac.update(dados);
        mac
    }

    fn assinar(&self, dados: &[u8]) -> Vec<u8> {
        self.mac(dados).finalize().into_bytes().to_vec()
    }
}

/// Um registro por toque, na ordem primeiro e depois último.
fn codificar(atribuicao: &Atribuicao) -> String {
    [("p", &atribuicao.primeiro), ("u", &atribuicao.ultimo)]
        .into_iter()
        .filter_map(|(rotulo, toque)| toque.as_ref().map(|t| linha(rotulo, t)))
        .collect::<Vec<_>>()
        .join(REGISTRO)
}

fn linha(rotulo: &str, toque: &Toque) -> String {
    let utm = &toque.captura.utm;
    let mut campos = vec![rotulo.to_string(), toque.quando_ms.to_string()];
    campos.extend(
        [
            &utm.source,
            &utm.medium,
            &utm.campaign,
            &utm.content,
            &utm.term,
        ]
        .map(|c| c.clone().unwrap_or_default()),
    );
    campos.extend(Plataforma::TODAS.map(|p| toque.captura.clid(p).unwrap_or_default().to_string()));
    campos.join(CAMPO)
}

fn decodificar(texto: &str) -> Option<Atribuicao> {
    let mut atribuicao = Atribuicao::default();
    for registro in texto.split(REGISTRO).filter(|r| !r.is_empty()) {
        let campos: Vec<&str> = registro.split(CAMPO).collect();
        if campos.len() != CAMPOS {
            return None;
        }
        let toque = Toque {
            quando_ms: campos[1].parse().ok()?,
            captura: Captura {
                utm: Utm {
                    source: opcional(campos[2]),
                    medium: opcional(campos[3]),
                    campaign: opcional(campos[4]),
                    content: opcional(campos[5]),
                    term: opcional(campos[6]),
                },
                clids: clids(&campos[7..CAMPOS]),
            },
        };
        match campos[0] {
            "p" => atribuicao.primeiro = Some(toque),
            "u" => atribuicao.ultimo = Some(toque),
            _ => return None,
        }
    }
    Some(atribuicao)
}

fn opcional(valor: &str) -> Option<String> {
    (!valor.is_empty()).then(|| valor.to_string())
}

fn clids(valores: &[&str]) -> Vec<Clid> {
    Plataforma::TODAS
        .iter()
        .zip(valores)
        .filter_map(|(plataforma, valor)| {
            opcional(valor).map(|valor| Clid {
                plataforma: *plataforma,
                valor,
            })
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::captura::de_url;

    const DIA: u64 = 86_400_000;

    fn chave() -> Vec<u8> {
        b"0123456789abcdef0123456789abcdef".to_vec()
    }

    fn cookie(prazo_dias: u64) -> Cookie {
        Cookie::new(&chave(), prazo_dias).expect("chave de 32 bytes")
    }

    fn com_dois_toques() -> Atribuicao {
        let mut atrib = Atribuicao::default();
        atrib.registrar(de_url("https://a.com/?utm_source=google&gclid=G123"), 1000);
        atrib.registrar(
            de_url("https://a.com/?utm_source=facebook&utm_medium=cpc&fbclid=F456"),
            2000,
        );
        atrib
    }

    #[test]
    fn chave_curta_e_recusada() {
        assert_eq!(
            Cookie::new(&[1u8; 5], 90).err(),
            Some(ErroCookie::ChaveCurta)
        );
        assert!(Cookie::new(&[1u8; CHAVE_MINIMA], 90).is_ok());
    }

    #[test]
    fn ida_e_volta_preserva_o_primeiro_e_o_ultimo_toque() {
        let c = cookie(90);
        let atrib = com_dois_toques();
        let valor = c.gravar(&atrib);
        assert_eq!(c.ler(&valor, 3000), Some(atrib));
    }

    #[test]
    fn cookie_vencido_nao_e_lido_e_o_que_esta_no_prazo_e_lido() {
        let c = cookie(1);
        let valor = c.gravar(&com_dois_toques());
        assert!(c.ler(&valor, 2000 + DIA).is_some());
        assert!(c.ler(&valor, 2000 + DIA + 1).is_none());
    }

    #[test]
    fn cookie_adulterado_e_recusado() {
        let c = cookie(90);
        let valor = c.gravar(&com_dois_toques());
        let primeiro = if valor.starts_with('A') { 'B' } else { 'A' };
        let trocado = format!("{primeiro}{}", &valor[1..]);
        assert!(c.ler(&trocado, 3000).is_none());
        let (corpo, assinatura) = valor.split_once('.').expect("ponto");
        let assinatura_falsa = "A".repeat(assinatura.len());
        assert!(
            c.ler(&format!("{corpo}.{assinatura_falsa}"), 3000)
                .is_none()
        );
        assert!(c.ler(&valor, 3000).is_some(), "o original continua válido");
    }

    #[test]
    fn cookie_assinado_com_outra_chave_e_recusado() {
        let outra = Cookie::new(b"outra-chave-com-mais-de-32-bytes!!", 90).expect("chave");
        let valor = outra.gravar(&com_dois_toques());
        assert!(cookie(90).ler(&valor, 3000).is_none());
    }

    #[test]
    fn formato_quebrado_e_recusado() {
        let c = cookie(90);
        assert!(c.ler("sem-ponto", 0).is_none());
        assert!(c.ler("!!.!!", 0).is_none());
        assert!(c.ler("", 0).is_none());
    }

    fn assinado(c: &Cookie, dados: &[u8]) -> String {
        let assinatura = c.assinar(dados);
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(dados),
            URL_SAFE_NO_PAD.encode(assinatura)
        )
    }

    #[test]
    fn assinado_com_a_chave_mas_com_conteudo_que_nao_e_atribuicao_e_recusado() {
        let c = cookie(90);
        assert!(c.ler(&assinado(&c, b"lixo sem registro"), 0).is_none());
        assert!(c.ler(&assinado(&c, &[0xff, 0xfe]), 0).is_none());
        let rotulo_ruim = ["z", "1", "", "", "", "", "", "", "", "", ""].join(CAMPO);
        assert!(c.ler(&assinado(&c, rotulo_ruim.as_bytes()), 0).is_none());
        let horario_ruim = ["p", "x", "", "", "", "", "", "", "", "", ""].join(CAMPO);
        assert!(c.ler(&assinado(&c, horario_ruim.as_bytes()), 0).is_none());
    }

    #[test]
    fn cookie_sem_nenhum_toque_nao_e_atribuicao_vigente() {
        let c = cookie(90);
        let valor = c.gravar(&Atribuicao::default());
        assert!(c.ler(&valor, 0).is_none());
    }

    #[test]
    fn cabecalho_leva_prazo_httponly_samesite_e_secure_quando_https() {
        let c = cookie(90);
        let sem = c.cabecalho("VALOR", false);
        assert!(sem.starts_with("rc_atrib=VALOR; Path=/; Max-Age=7776000; HttpOnly; SameSite=Lax"));
        assert!(!sem.contains("Secure"));
        assert!(c.cabecalho("VALOR", true).ends_with("; Secure"));
    }

    #[test]
    fn valor_com_os_limites_maximos_cabe_no_cookie() {
        let texto_max = "u".repeat(100);
        let clid_max = "c".repeat(150);
        let utm = Utm {
            source: Some(texto_max.clone()),
            medium: Some(texto_max.clone()),
            campaign: Some(texto_max.clone()),
            content: Some(texto_max.clone()),
            term: Some(texto_max),
        };
        let clids = Plataforma::TODAS
            .into_iter()
            .map(|plataforma| Clid {
                plataforma,
                valor: clid_max.clone(),
            })
            .collect();
        let toque = Toque {
            quando_ms: 1,
            captura: Captura { utm, clids },
        };
        let atrib = Atribuicao {
            primeiro: Some(toque.clone()),
            ultimo: Some(toque),
        };
        let c = cookie(90);
        let valor = c.gravar(&atrib);
        assert!(valor.len() < 4000, "cookie grande demais: {}", valor.len());
        assert_eq!(c.ler(&valor, 2), Some(atrib));
    }

    #[test]
    fn registro_com_so_utm_e_so_clid_sobrevive_a_ida_e_volta() {
        let c = cookie(90);
        let mut atrib = Atribuicao::default();
        atrib.registrar(de_url("https://a.com/?fbclid=SO1"), 5);
        atrib.registrar(de_url("https://a.com/?utm_term=t"), 6);
        assert_eq!(c.ler(&c.gravar(&atrib), 7), Some(atrib));
    }
}
