//! Consentimento do visitante antes de qualquer dado pessoal sair.
//!
//! Sem consentimento, nada é montado com dado do cliente, nada vai à Meta, e o motivo fica registrado.
//! A biblioteca não decide a base legal: quem usa responde pelo consentimento (ou pela base que adotar).

use std::fmt;

/// Por que um evento não foi enviado por causa do consentimento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    /// O visitante não consentiu com o uso dos dados.
    SemConsentimento,
}

impl Motivo {
    /// Código curto, para log e para a resposta da API (`sem_consentimento`).
    #[must_use]
    pub fn codigo(self) -> &'static str {
        match self {
            Motivo::SemConsentimento => "sem_consentimento",
        }
    }
}

impl fmt::Display for Motivo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Motivo::SemConsentimento => {
                "o visitante não consentiu; nenhum dado pessoal foi montado nem enviado à Meta"
            }
        })
    }
}

/// Deixa passar só com consentimento.
///
/// # Errors
///
/// Devolve [`Motivo::SemConsentimento`] quando `consentido` é falso.
pub fn checar(consentido: bool) -> Result<(), Motivo> {
    if consentido {
        Ok(())
    } else {
        Err(Motivo::SemConsentimento)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn com_consentimento_passa_e_sem_nao() {
        assert_eq!(checar(true), Ok(()));
        assert_eq!(checar(false), Err(Motivo::SemConsentimento));
    }

    #[test]
    fn motivo_tem_codigo_e_texto_que_diz_o_que_nao_saiu() {
        assert_eq!(Motivo::SemConsentimento.codigo(), "sem_consentimento");
        let texto = Motivo::SemConsentimento.to_string();
        assert!(texto.contains("não consentiu"));
        assert!(texto.contains("nenhum dado pessoal"));
    }
}
