//! Captura de UTM e de clids (fbclid, gclid, ttclid, msclkid) de uma URL.
//!
//! Tudo que vem da URL é tratado como não confiável. Parâmetro repetido vale o primeiro, caractere de
//! controle sai, valor longo é cortado e clid com caractere fora de letras, números, `-` e `_` é descartado.
//! A URL completa nunca vai para a Meta: só a página, sem a consulta (ver [`url_sem_consulta`]).

/// Tamanho máximo de um valor de UTM, em caracteres. Com isso o cookie fica abaixo de 4 KB.
pub const MAX_UTM: usize = 100;
/// Tamanho máximo de um clid, em caracteres.
pub const MAX_CLID: usize = 150;
/// Quantos parâmetros da consulta são lidos. O resto é ignorado.
pub const MAX_PARAMETROS: usize = 50;
/// Tamanho máximo de uma URL de página aceita, em caracteres.
const MAX_URL: usize = 2048;

/// De qual plataforma veio o clique. Cada uma usa um parâmetro próprio na URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plataforma {
    /// Meta (Facebook e Instagram), parâmetro `fbclid`.
    Meta,
    /// Google Ads, parâmetro `gclid`.
    Google,
    /// `TikTok` Ads, parâmetro `ttclid`.
    TikTok,
    /// Microsoft Ads, parâmetro `msclkid`.
    Microsoft,
}

impl Plataforma {
    /// Todas as plataformas, na ordem em que aparecem no painel.
    pub const TODAS: [Plataforma; 4] = [
        Plataforma::Meta,
        Plataforma::Google,
        Plataforma::TikTok,
        Plataforma::Microsoft,
    ];

    /// O nome do parâmetro da URL (`fbclid`, `gclid`, `ttclid` ou `msclkid`).
    #[must_use]
    pub fn parametro(self) -> &'static str {
        match self {
            Plataforma::Meta => "fbclid",
            Plataforma::Google => "gclid",
            Plataforma::TikTok => "ttclid",
            Plataforma::Microsoft => "msclkid",
        }
    }
}

/// Os cinco campos de UTM. Cada um é `None` quando a URL não trouxe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Utm {
    /// `utm_source`: a origem, por exemplo `facebook`.
    pub source: Option<String>,
    /// `utm_medium`: o meio, por exemplo `cpc`.
    pub medium: Option<String>,
    /// `utm_campaign`: a campanha.
    pub campaign: Option<String>,
    /// `utm_content`: o anúncio ou a variação criativa.
    pub content: Option<String>,
    /// `utm_term`: o termo ou o público.
    pub term: Option<String>,
}

impl Utm {
    /// Se pelo menos um dos cinco campos veio.
    #[must_use]
    pub fn algum(&self) -> bool {
        self.source.is_some()
            || self.medium.is_some()
            || self.campaign.is_some()
            || self.content.is_some()
            || self.term.is_some()
    }
}

/// Um clid capturado: a plataforma e o valor, já validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clid {
    /// De qual plataforma é o clique.
    pub plataforma: Plataforma,
    /// O valor do clid, só com letras, números, `-` e `_`.
    pub valor: String,
}

/// O que uma URL trouxe de rastreio: UTM e clids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Captura {
    /// Os cinco campos de UTM.
    pub utm: Utm,
    /// Um clid por plataforma, no máximo.
    pub clids: Vec<Clid>,
}

impl Captura {
    /// Se a URL trouxe algum rastreio (UTM ou clid). Sem isso, não vira toque de atribuição.
    #[must_use]
    pub fn tem_toque(&self) -> bool {
        self.utm.algum() || !self.clids.is_empty()
    }

    /// O valor do clid de uma plataforma, se a URL trouxe.
    #[must_use]
    pub fn clid(&self, plataforma: Plataforma) -> Option<&str> {
        self.clids
            .iter()
            .find(|c| c.plataforma == plataforma)
            .map(|c| c.valor.as_str())
    }
}

/// Lê a consulta de uma URL e devolve o que há de UTM e de clids.
///
/// Aceita qualquer texto: URL relativa, URL quebrada ou lixo só devolvem uma captura vazia.
#[must_use]
pub fn de_url(url: &str) -> Captura {
    let mut captura = Captura::default();
    for (chave, valor) in pares(consulta(url)) {
        let chave = chave.to_ascii_lowercase();
        match chave.as_str() {
            "utm_source" => primeiro(&mut captura.utm.source, valor_utm(&valor)),
            "utm_medium" => primeiro(&mut captura.utm.medium, valor_utm(&valor)),
            "utm_campaign" => primeiro(&mut captura.utm.campaign, valor_utm(&valor)),
            "utm_content" => primeiro(&mut captura.utm.content, valor_utm(&valor)),
            "utm_term" => primeiro(&mut captura.utm.term, valor_utm(&valor)),
            outra => {
                if let Some(plataforma) = plataforma_do_parametro(outra)
                    && captura.clid(plataforma).is_none()
                    && let Some(valor) = valor_clid(&valor)
                {
                    captura.clids.push(Clid { plataforma, valor });
                }
            }
        }
    }
    captura
}

/// A URL da página sem a consulta e sem o fragmento, só `http` ou `https`.
///
/// Devolve `None` para `javascript:`, URL com usuário e senha (`user@host`), espaço, controle
/// ou URL longa demais. É esse texto que vai no campo `event_source_url` do evento.
#[must_use]
pub fn url_sem_consulta(url: &str) -> Option<String> {
    let sem_consulta = url.split(['?', '#']).next()?;
    let (esquema, resto) = sem_consulta.split_once("://")?;
    let esquema = esquema.to_ascii_lowercase();
    if esquema != "http" && esquema != "https" {
        return None;
    }
    let corte = resto.find('/').unwrap_or(resto.len());
    let (host, caminho) = resto.split_at(corte);
    let host_ok = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c));
    let caminho_ok = caminho
        .chars()
        .all(|c| !c.is_control() && !c.is_whitespace());
    if !host_ok || !caminho_ok || sem_consulta.len() > MAX_URL {
        return None;
    }
    Some(format!("{esquema}://{host}{caminho}"))
}

/// Os pares `chave=valor` de um texto de consulta (`a=1&b=2`), já decodificados.
///
/// Serve à URL e ao corpo de formulário. No máximo [`MAX_PARAMETROS`] pares.
#[must_use]
pub fn pares(texto: &str) -> Vec<(String, String)> {
    texto
        .split('&')
        .filter(|par| !par.is_empty())
        .take(MAX_PARAMETROS)
        .map(|par| {
            let (chave, valor) = par.split_once('=').unwrap_or((par, ""));
            (desfazer(chave), desfazer(valor))
        })
        .collect()
}

/// Decodifica `%XX` e troca `+` por espaço. Sequência inválida fica como veio, e UTF-8 quebrado
/// vira o caractere de substituição (nunca dá pânico).
#[must_use]
pub fn desfazer(texto: &str) -> String {
    let com_espacos = texto.replace('+', " ");
    let bytes = com_espacos.as_bytes();
    let mut saida: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(alto), Some(baixo)) = (hexa(bytes[i + 1]), hexa(bytes[i + 2]))
        {
            saida.push(alto * 16 + baixo);
            i += 3;
        } else {
            saida.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&saida).into_owned()
}

/// A consulta da URL: o que vem depois de `?` e antes de `#`.
fn consulta(url: &str) -> &str {
    let sem_fragmento = url.split_once('#').map_or(url, |(antes, _)| antes);
    sem_fragmento
        .split_once('?')
        .map_or("", |(_, depois)| depois)
}

/// Guarda o valor só na primeira vez: parâmetro repetido não troca o que já entrou.
fn primeiro(slot: &mut Option<String>, valor: Option<String>) {
    if slot.is_none() {
        *slot = valor;
    }
}

fn plataforma_do_parametro(chave: &str) -> Option<Plataforma> {
    Plataforma::TODAS
        .into_iter()
        .find(|p| p.parametro() == chave)
}

fn hexa(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Tira o controle, corta no tamanho e aparoa. Vazio vira `None`.
fn limpar(texto: &str, maximo: usize) -> Option<String> {
    let limpo: String = texto
        .chars()
        .filter(|c| !c.is_control())
        .take(maximo)
        .collect();
    let aparado = limpo.trim();
    (!aparado.is_empty()).then(|| aparado.to_string())
}

fn valor_utm(bruto: &str) -> Option<String> {
    limpar(bruto, MAX_UTM).map(|v| v.to_lowercase())
}

fn valor_clid(bruto: &str) -> Option<String> {
    let valor = bruto.trim();
    let valido = !valor.is_empty()
        && valor.len() <= MAX_CLID
        && valor
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valido.then(|| valor.to_string())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn consulta_de(q: &str) -> Captura {
        de_url(&format!("https://loja.com/p?{q}"))
    }

    #[test]
    fn utm_basicas_viram_minusculas_e_aparadas() {
        let c = consulta_de(
            "utm_source=Facebook&utm_medium=cpc&utm_campaign=Verao+2026&utm_content=video%201&utm_term=%20x%20",
        );
        assert_eq!(c.utm.source.as_deref(), Some("facebook"));
        assert_eq!(c.utm.medium.as_deref(), Some("cpc"));
        assert_eq!(c.utm.campaign.as_deref(), Some("verao 2026"));
        assert_eq!(c.utm.content.as_deref(), Some("video 1"));
        assert_eq!(c.utm.term.as_deref(), Some("x"));
        assert!(c.utm.algum() && c.tem_toque());
    }

    #[test]
    fn cada_clid_vira_a_sua_plataforma() {
        let c = consulta_de("fbclid=AbC-_9&gclid=G1&ttclid=T2&msclkid=M3");
        assert_eq!(c.clid(Plataforma::Meta), Some("AbC-_9"));
        assert_eq!(c.clid(Plataforma::Google), Some("G1"));
        assert_eq!(c.clid(Plataforma::TikTok), Some("T2"));
        assert_eq!(c.clid(Plataforma::Microsoft), Some("M3"));
        assert_eq!(c.clids.len(), 4);
        assert!(c.tem_toque());
    }

    #[test]
    fn primeiro_parametro_vale_quando_repete() {
        let c = consulta_de("utm_source=a&utm_source=b&fbclid=X1&fbclid=X2");
        assert_eq!(c.utm.source.as_deref(), Some("a"));
        assert_eq!(c.clid(Plataforma::Meta), Some("X1"));
        assert_eq!(c.clids.len(), 1);
    }

    #[test]
    fn chave_maiuscula_ou_codificada_tambem_vale() {
        let c = consulta_de("UTM_SOURCE=x&%75tm_medium=y&FBCLID=Z");
        assert_eq!(c.utm.source.as_deref(), Some("x"));
        assert_eq!(c.utm.medium.as_deref(), Some("y"));
        assert_eq!(c.clid(Plataforma::Meta), Some("Z"));
    }

    #[test]
    fn clid_com_caractere_estranho_longo_ou_vazio_e_descartado() {
        let longo = "a".repeat(MAX_CLID + 1);
        let c = consulta_de(&format!(
            "fbclid=abc%3Cscript%3E&gclid={longo}&ttclid=&msclkid=ok%20sim"
        ));
        assert!(c.clids.is_empty(), "{:?}", c.clids);
        assert!(!c.tem_toque());
    }

    #[test]
    fn controle_e_quebra_de_linha_nao_entram_no_valor() {
        let c = consulta_de("utm_source=%0d%0aSET%20COOKIE%3A%20x&utm_medium=%00%01");
        assert_eq!(c.utm.source.as_deref(), Some("set cookie: x"));
        assert!(c.utm.medium.is_none());
    }

    #[test]
    fn valor_longo_e_cortado() {
        let c = consulta_de(&format!("utm_campaign={}", "c".repeat(300)));
        assert_eq!(c.utm.campaign.as_ref().map(String::len), Some(MAX_UTM));
    }

    #[test]
    fn so_os_primeiros_parametros_sao_lidos() {
        let muitos = (0..MAX_PARAMETROS)
            .map(|i| format!("x{i}=1&"))
            .collect::<Vec<_>>()
            .concat();
        let c = consulta_de(&format!("{muitos}utm_source=tarde"));
        assert!(c.utm.source.is_none());
    }

    #[test]
    fn percent_invalido_fica_como_veio_e_utf8_quebrado_vira_substituto() {
        let c = consulta_de("utm_source=%zz%2&utm_medium=%FF%FE&utm_campaign=a+b%2Bc");
        assert_eq!(c.utm.source.as_deref(), Some("%zz%2"));
        assert_eq!(c.utm.medium.as_deref(), Some("\u{fffd}\u{fffd}"));
        assert_eq!(c.utm.campaign.as_deref(), Some("a b+c"));
    }

    #[test]
    fn valor_vazio_e_chave_sem_igual_nao_contam() {
        let c = consulta_de("utm_source=&utm_medium&&=x");
        assert!(!c.utm.algum());
        assert!(!c.tem_toque());
    }

    #[test]
    fn fragmento_nao_entra_na_consulta() {
        let c = de_url("https://a.com/?utm_source=x#utm_medium=y");
        assert_eq!(c.utm.source.as_deref(), Some("x"));
        assert!(c.utm.medium.is_none());
        assert!(!de_url("https://a.com/#utm_source=x").tem_toque());
    }

    #[test]
    fn sem_consulta_nao_ha_rastreio() {
        assert!(!de_url("https://a.com/pagina").tem_toque());
        assert!(!de_url("texto qualquer").tem_toque());
        assert!(!de_url("").tem_toque());
    }

    #[test]
    fn url_sem_consulta_so_aceita_http_e_https_e_tira_a_consulta() {
        assert_eq!(
            url_sem_consulta("https://loja.com/oferta?utm_source=x#topo").as_deref(),
            Some("https://loja.com/oferta")
        );
        assert_eq!(
            url_sem_consulta("HTTP://Loja.COM").as_deref(),
            Some("http://Loja.COM")
        );
        assert_eq!(
            url_sem_consulta("https://loja.com:8443/a").as_deref(),
            Some("https://loja.com:8443/a")
        );
        assert_eq!(
            url_sem_consulta("https://loja.com?x=1").as_deref(),
            Some("https://loja.com")
        );
    }

    #[test]
    fn url_sem_consulta_recusa_o_que_nao_e_pagina() {
        let ruins = [
            "javascript:alert(1)",
            "javascript:alert(1)//://x",
            "ftp://a.com/",
            "https://",
            "https:///caminho",
            "https://usuario:senha@a.com/",
            "https://a.com/com espaco",
            "https://a.com/\nlinha",
            "https://a<b>.com/",
            "//a.com/relativa",
            "/relativa",
            "",
        ];
        for ruim in ruins {
            assert!(url_sem_consulta(ruim).is_none(), "devia recusar: {ruim}");
        }
        let longa = format!("https://a.com/{}", "x".repeat(MAX_URL));
        assert!(url_sem_consulta(&longa).is_none());
    }

    #[test]
    fn pares_decodifica_e_respeita_o_limite() {
        assert_eq!(
            pares("a=1&b=2+3&c"),
            vec![
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2 3".to_string()),
                ("c".to_string(), String::new()),
            ]
        );
        assert_eq!(pares(""), Vec::new());
        assert_eq!(pares("&&"), Vec::new());
    }

    #[test]
    fn desfazer_trata_sequencias_e_sobras() {
        assert_eq!(desfazer("%41%42%2b"), "AB+");
        assert_eq!(desfazer("100%"), "100%");
        assert_eq!(desfazer("%4"), "%4");
        assert_eq!(desfazer("%G1"), "%G1");
        assert_eq!(desfazer("a%20b"), "a b");
    }

    #[test]
    fn plataforma_tem_o_parametro_certo() {
        assert_eq!(Plataforma::Meta.parametro(), "fbclid");
        assert_eq!(Plataforma::Google.parametro(), "gclid");
        assert_eq!(Plataforma::TikTok.parametro(), "ttclid");
        assert_eq!(Plataforma::Microsoft.parametro(), "msclkid");
        assert_eq!(Plataforma::TODAS.len(), 4);
    }

    #[test]
    fn captura_vazia_nao_tem_toque_e_clid_ausente_da_none() {
        let c = Captura::default();
        assert!(!c.tem_toque());
        assert!(c.clid(Plataforma::Meta).is_none());
    }
}
