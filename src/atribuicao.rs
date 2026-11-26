//! Atribuição de primeiro e último toque.
//!
//! Um toque é uma visita com UTM ou clid. O primeiro toque fica para sempre (até o cookie vencer);
//! o último é trocado a cada toque novo. Visita sem nenhum rastreio (acesso direto) não troca nada.

use crate::captura::Captura;

/// Um toque: o horário em que a visita aconteceu e o que a URL trouxe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toque {
    /// Horário do toque, em milissegundos desde a época Unix.
    pub quando_ms: u64,
    /// O que a URL trouxe: UTM e clids.
    pub captura: Captura,
}

/// Primeiro e último toque de um visitante.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Atribuicao {
    /// O primeiro toque com rastreio. Não muda depois de gravado.
    pub primeiro: Option<Toque>,
    /// O último toque com rastreio. Troca a cada toque novo.
    pub ultimo: Option<Toque>,
}

impl Atribuicao {
    /// Registra uma captura. Devolve `true` se virou toque, `false` se a URL não trouxe rastreio.
    pub fn registrar(&mut self, captura: Captura, agora_ms: u64) -> bool {
        if !captura.tem_toque() {
            return false;
        }
        let toque = Toque {
            quando_ms: agora_ms,
            captura,
        };
        if self.primeiro.is_none() {
            self.primeiro = Some(toque.clone());
        }
        self.ultimo = Some(toque);
        true
    }

    /// Se o último toque ainda está dentro do prazo (em milissegundos). Sem último toque, não está.
    #[must_use]
    pub fn vigente(&self, agora_ms: u64, prazo_ms: u64) -> bool {
        self.ultimo
            .as_ref()
            .is_some_and(|t| agora_ms <= t.quando_ms.saturating_add(prazo_ms))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::captura::de_url;

    #[test]
    fn registrar_sem_rastreio_nao_vira_toque() {
        let mut atrib = Atribuicao::default();
        assert!(!atrib.registrar(de_url("https://a.com/pagina"), 1));
        assert!(atrib.primeiro.is_none() && atrib.ultimo.is_none());
    }

    #[test]
    fn primeiro_fica_e_o_ultimo_troca() {
        let mut atrib = Atribuicao::default();
        assert!(atrib.registrar(de_url("https://a.com/?utm_source=a"), 10));
        assert!(atrib.registrar(de_url("https://a.com/?utm_source=b"), 20));
        assert_eq!(atrib.primeiro.as_ref().map(|t| t.quando_ms), Some(10));
        assert_eq!(atrib.ultimo.as_ref().map(|t| t.quando_ms), Some(20));
        assert_eq!(
            atrib.primeiro.and_then(|t| t.captura.utm.source),
            Some("a".to_string())
        );
    }

    #[test]
    fn vigente_respeita_o_prazo_e_sem_ultimo_nao_esta() {
        let mut atrib = Atribuicao::default();
        assert!(!atrib.vigente(0, 1000));
        atrib.registrar(de_url("https://a.com/?utm_source=a"), 1000);
        assert!(atrib.vigente(1000 + 500, 500));
        assert!(!atrib.vigente(1000 + 501, 500));
    }

    #[test]
    fn prazo_enorme_nao_estoura() {
        let mut atrib = Atribuicao::default();
        atrib.registrar(de_url("https://a.com/?utm_source=a"), u64::MAX - 1);
        assert!(atrib.vigente(u64::MAX, u64::MAX));
    }
}
