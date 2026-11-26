//! A tela de conferência: as últimas origens, os últimos eventos e os recursos do plano Wortic.
//!
//! HTML puro, sem script. Tudo que veio da URL ou do formulário passa por [`escapar`]. A tela mostra
//! clids mascarados e nunca mostra IP, e-mail, telefone ou nome.

use crate::modelo::{Origem, Registro, Retrato};

/// O link dos planos da Wortic, mostrado nos recursos pagos.
pub const PLANOS: &str = "https://wortic.com.br/planos";

/// O que a tela precisa saber além da memória.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Situacao {
    /// Se o visitante deste navegador consentiu.
    pub consentido: bool,
    /// Se há Pixel e token, e o envio à Meta está ligado.
    pub envio_ligado: bool,
    /// Se `WORTIC_API_KEY` está definida. Só a presença; o valor nunca é lido aqui.
    pub wortic_chave: bool,
}

/// Os recursos pagos da Wortic: nome e o que fazem. Aparecem na tela com o aviso e o link dos planos.
pub const GANCHOS: [(&str, &str); 3] = [
    (
        "Painel de atribuição por anúncio",
        "Receita, custo e retorno de cada anúncio, em um painel só.",
    ),
    (
        "CAPI gerenciada com reprocessamento",
        "A Wortic guarda cada evento, reenvia o que a Meta recusou e cuida da desduplicação.",
    ),
    (
        "Google e TikTok",
        "Envio de conversões para o Google Ads e o TikTok Ads, com a mesma atribuição.",
    ),
];

const AVISO_PLANO: &str = "Recurso pago da Wortic. Este modelo não o inclui.";
const AVISO_CHAVE: &str = "WORTIC_API_KEY encontrada, mas a API de rastreio da Wortic não tem rota pública documentada. Nada foi chamado.";

const ESTILO: &str = "
:root{--tinta:#1d1d1f;--suave:#6e6e73;--linha:#d2d2d7;--fina:#e8e8ed;--azul:#0071e3;--ok:#1a7f37;--aviso:#8a5a00}
*{box-sizing:border-box}
body{margin:0;background:#fff;color:var(--tinta);font:16px/1.45 -apple-system,BlinkMacSystemFont,'Helvetica Neue',Arial,sans-serif;-webkit-font-smoothing:antialiased}
main{max-width:980px;margin:0 auto;padding:40px 20px 72px}
h1{font-size:32px;line-height:1.15;letter-spacing:-.015em;margin:0 0 6px;font-weight:600}
h2{font-size:20px;margin:36px 0 10px;font-weight:600;letter-spacing:-.01em}
.sub{color:var(--suave);margin:0 0 24px}
.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:14px}
.card{border:1px solid var(--fina);border-radius:16px;padding:18px 20px;box-shadow:0 1px 2px rgba(0,0,0,.04);min-width:0}
.card h3{font-size:16px;margin:0 0 6px;font-weight:600}
.card p{margin:0;color:var(--suave);font-size:14px}
.ok{color:var(--ok);font-weight:600}
.card p.aviso{color:var(--aviso);font-size:13px;margin:12px 0 0}
a{color:var(--azul)}
table{width:100%;min-width:640px;border-collapse:collapse;font-size:14px;border:1px solid var(--fina);border-radius:12px}
th,td{text-align:left;padding:9px 10px;border-bottom:1px solid var(--fina);vertical-align:top;white-space:nowrap}
th{background:#fafafa;color:var(--suave);font-weight:500}
.vazio{color:var(--suave);font-size:14px;border:1px dashed var(--linha);border-radius:12px;padding:16px}
pre{background:#fafafa;border:1px solid var(--fina);border-radius:12px;padding:14px;font-size:13px;overflow-x:auto;white-space:pre-wrap;overflow-wrap:anywhere}
.tabela{overflow-x:auto}
footer{margin-top:40px;color:var(--suave);font-size:13px}
";

/// Escapa o texto para HTML. Tudo que vem de fora passa por aqui antes de entrar na tela.
#[must_use]
pub fn escapar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '&' => saida.push_str("&amp;"),
            '<' => saida.push_str("&lt;"),
            '>' => saida.push_str("&gt;"),
            '"' => saida.push_str("&quot;"),
            '\'' => saida.push_str("&#39;"),
            outro => saida.push(outro),
        }
    }
    saida
}

/// Um clid na tela: seis primeiros caracteres e reticências. O valor inteiro não aparece.
#[must_use]
pub fn mascarar(valor: &str) -> String {
    let inicio: String = valor.chars().take(6).collect();
    format!("{inicio}…")
}

/// A hora em UTC a partir de milissegundos desde a época Unix, no formato `HH:MM:SS UTC`.
#[must_use]
pub fn hora(quando_ms: u64) -> String {
    let segundos = (quando_ms / 1000) % 86_400;
    format!(
        "{:02}:{:02}:{:02} UTC",
        segundos / 3600,
        (segundos % 3600) / 60,
        segundos % 60
    )
}

/// A página inteira: a situação, as origens, os eventos e os recursos pagos.
#[must_use]
pub fn pagina(retrato: &Retrato, situacao: &Situacao) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\">");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    html.push_str("<title>Rastreio e Conversions API</title><style>");
    html.push_str(ESTILO);
    html.push_str("</style></head><body><main>");
    html.push_str("<h1>Rastreio de campanhas e Conversions API</h1>");
    html.push_str("<p class=\"sub\">Modelo da Wortic em Rust. Tudo fica só na memória deste programa: quando ele para, some.</p>");
    html.push_str("<section class=\"cards\">");
    html.push_str(&cartao_consentimento(situacao.consentido));
    html.push_str(&cartao_envio(situacao.envio_ligado));
    html.push_str("</section>");
    html.push_str("<h2>Últimas origens</h2>");
    html.push_str(&tabela_origens(&retrato.origens));
    html.push_str("<h2>Últimos eventos</h2>");
    html.push_str(&tabela_eventos(&retrato.registros));
    html.push_str("<h2>Recursos do plano Wortic</h2><section class=\"cards\">");
    for (nome, descricao) in GANCHOS {
        html.push_str(&cartao_gancho(nome, descricao, situacao.wortic_chave));
    }
    html.push_str("</section>");
    html.push_str("<footer>Feito com tecnologia Wortic (wortic.com.br). Clids aparecem mascarados e IP nunca aparece nesta tela.</footer>");
    html.push_str("</main></body></html>");
    html
}

fn cartao_consentimento(consentido: bool) -> String {
    let estado = if consentido {
        "<span class=\"ok\">Consentimento concedido</span>"
    } else {
        "Consentimento não concedido"
    };
    format!(
        "<div class=\"card\"><h3>Este navegador</h3><p>{estado}</p><p><a href=\"/consentimento?aceitar=sim\">Aceitar</a> · <a href=\"/consentimento?aceitar=nao\">Recusar</a></p></div>"
    )
}

fn cartao_envio(ligado: bool) -> String {
    let texto = if ligado {
        "<span class=\"ok\">Envio à Meta ligado</span>"
    } else {
        "Modo demonstração: sem RASTREIO_PIXEL_ID e RASTREIO_ACCESS_TOKEN, nada sai daqui."
    };
    format!("<div class=\"card\"><h3>Conversions API da Meta</h3><p>{texto}</p></div>")
}

fn tabela_origens(origens: &[Origem]) -> String {
    if origens.is_empty() {
        return "<div class=\"vazio\">Nenhuma origem ainda. Abra uma página com UTM ou clid e chame <code>/r?u=</code> com o endereço dela.</div>".to_string();
    }
    let linhas: String = origens.iter().map(linha_origem).collect();
    format!(
        "<div class=\"tabela\"><table><tr><th>Hora</th><th>Página</th><th>Fonte</th><th>Meio</th><th>Campanha</th><th>Clids</th></tr>{linhas}</table></div>"
    )
}

fn linha_origem(origem: &Origem) -> String {
    let utm = &origem.captura.utm;
    let clids: Vec<String> = origem
        .captura
        .clids
        .iter()
        .map(|c| format!("{} {}", c.plataforma.parametro(), mascarar(&c.valor)))
        .collect();
    format!(
        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        hora(origem.quando_ms),
        escapar(&origem.pagina),
        celula(utm.source.as_deref()),
        celula(utm.medium.as_deref()),
        celula(utm.campaign.as_deref()),
        escapar(&clids.join(", ")),
    )
}

fn tabela_eventos(registros: &[Registro]) -> String {
    if registros.is_empty() {
        return "<div class=\"vazio\">Nenhum evento ainda. Envie uma venda para <code>POST /evento</code>.</div>".to_string();
    }
    let linhas: String = registros.iter().map(linha_registro).collect();
    format!(
        "<div class=\"tabela\"><table><tr><th>Hora</th><th>Evento</th><th>Pedido</th><th>Valor</th><th>Origem</th><th>Situação</th></tr>{linhas}</table></div>"
    )
}

fn linha_registro(registro: &Registro) -> String {
    format!(
        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        hora(registro.quando_ms),
        escapar(registro.evento),
        escapar(&registro.event_id),
        celula(registro.valor.as_deref()),
        escapar(&registro.origem),
        escapar(&registro.situacao),
    )
}

fn celula(valor: Option<&str>) -> String {
    valor.map_or_else(|| "-".to_string(), escapar)
}

fn cartao_gancho(nome: &str, descricao: &str, com_chave: bool) -> String {
    let aviso = if com_chave { AVISO_CHAVE } else { AVISO_PLANO };
    format!(
        "<div class=\"card\"><h3>{}</h3><p>{}</p><p class=\"aviso\">{} <a href=\"{PLANOS}\">Ver planos</a></p></div>",
        escapar(nome),
        escapar(descricao),
        escapar(aviso),
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::captura::de_url;
    use crate::modelo::{Origem, Registro};

    fn situacao(consentido: bool, ligado: bool, chave: bool) -> Situacao {
        Situacao {
            consentido,
            envio_ligado: ligado,
            wortic_chave: chave,
        }
    }

    fn retrato_com_dados() -> Retrato {
        let origem = Origem {
            quando_ms: 3_661_000,
            pagina: "https://loja.com/<script>alert(1)</script>".to_string(),
            captura: de_url(
                "https://loja.com/?utm_source=Facebook&utm_medium=cpc&utm_campaign=verao&fbclid=SEGREDOCLIDLONGO",
            ),
        };
        let registro = Registro {
            quando_ms: 3_661_000,
            evento: "Purchase",
            event_id: "pedido\"<1>".to_string(),
            valor: Some("99,90 BRL".to_string()),
            origem: "facebook / cpc / verao".to_string(),
            situacao: "enviado em 1 tentativa(s)".to_string(),
        };
        Retrato {
            origens: vec![origem],
            registros: vec![registro],
        }
    }

    #[test]
    fn escapar_protege_contra_html_injetado() {
        assert_eq!(
            escapar("<script>\"'&</script>"),
            "&lt;script&gt;&quot;&#39;&amp;&lt;/script&gt;"
        );
        assert_eq!(escapar("texto comum"), "texto comum");
    }

    #[test]
    fn mascarar_mostra_seis_caracteres_e_reticencias() {
        assert_eq!(mascarar("abcdefghij"), "abcdef…");
        assert_eq!(mascarar("ab"), "ab…");
    }

    #[test]
    fn hora_em_utc_no_formato_certo() {
        assert_eq!(hora(0), "00:00:00 UTC");
        assert_eq!(hora(3_661_000), "01:01:01 UTC");
        assert_eq!(hora(86_400_000 + 5000), "00:00:05 UTC");
    }

    #[test]
    fn tela_vazia_diz_o_que_fazer_e_o_estado_do_envio() {
        let html = pagina(
            &Retrato {
                origens: Vec::new(),
                registros: Vec::new(),
            },
            &situacao(false, false, false),
        );
        assert!(html.contains("Nenhuma origem ainda"));
        assert!(html.contains("Nenhum evento ainda"));
        assert!(html.contains("Consentimento não concedido"));
        assert!(html.contains("Modo demonstração"));
        assert!(html.contains("Recurso pago da Wortic"));
        assert!(html.contains("https://wortic.com.br/planos"));
        assert_eq!(html.matches("Ver planos").count(), GANCHOS.len());
    }

    #[test]
    fn tela_com_dados_mostra_origem_evento_e_nao_o_clid_inteiro() {
        let html = pagina(&retrato_com_dados(), &situacao(true, true, false));
        assert!(html.contains("Consentimento concedido"));
        assert!(html.contains("Envio à Meta ligado"));
        assert!(html.contains("facebook"));
        assert!(html.contains("fbclid SEGRED…"));
        assert!(
            !html.contains("SEGREDOCLIDLONGO"),
            "o clid inteiro não aparece"
        );
        assert!(!html.contains("<script>alert"), "a página vem escapada");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("pedido&quot;&lt;1&gt;"));
        assert!(html.contains("99,90 BRL"));
        assert!(html.contains("01:01:01 UTC"));
    }

    #[test]
    fn origem_sem_utm_mostra_traco_nas_celulas_vazias() {
        let retrato = Retrato {
            origens: vec![Origem {
                quando_ms: 0,
                pagina: "https://a.com/".to_string(),
                captura: de_url("https://a.com/?gclid=G1"),
            }],
            registros: vec![Registro {
                quando_ms: 0,
                evento: "Lead",
                event_id: "x".to_string(),
                valor: None,
                origem: "clid, sem UTM".to_string(),
                situacao: "recusado: sem_consentimento".to_string(),
            }],
        };
        let html = pagina(&retrato, &situacao(false, false, false));
        assert!(html.contains("<td>-</td>"));
        assert!(html.contains("gclid G1…"));
    }

    #[test]
    fn com_chave_wortic_o_aviso_diz_que_nada_foi_chamado() {
        let html = pagina(&retrato_com_dados(), &situacao(false, false, true));
        assert!(html.contains("WORTIC_API_KEY encontrada"));
        assert!(html.contains("Nada foi chamado"));
        assert!(!html.contains("Recurso pago da Wortic"));
    }

    #[test]
    fn cada_gancho_aparece_com_nome_e_descricao() {
        let html = pagina(&retrato_com_dados(), &situacao(false, false, false));
        for (nome, descricao) in GANCHOS {
            assert!(html.contains(nome), "{nome}");
            assert!(html.contains(descricao), "{descricao}");
        }
    }
}
