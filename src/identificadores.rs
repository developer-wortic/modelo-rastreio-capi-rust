//! Os identificadores de clique da Meta: `fbc` (o clique) e `fbp` (o navegador).
//!
//! Formato da documentação da Meta:
//! - `fbc`: `fb.{indice_subdominio}.{horario_em_ms}.{fbclid}`
//! - `fbp`: `fb.{indice_subdominio}.{horario_em_ms}.{numero_aleatorio}`
//!
//! O `fbc` do cookie `_fbc` do Pixel tem prioridade. Só quando ele não existe, o `fbc` é montado
//! a partir do `fbclid` que chegou na URL, com o horário em que o clique foi visto.

/// Índice de subdomínio padrão: 1 para domínio comum, como `minhaloja.com`.
pub const INDICE_PADRAO: u8 = 1;

/// Monta o `fbc` a partir do `fbclid`. `None` se o `fbclid` estiver vazio ou com caractere estranho.
#[must_use]
pub fn fbc_do_fbclid(fbclid: &str, horario_ms: u64, indice: u8) -> Option<String> {
    let valor = fbclid.trim();
    let valido = !valor.is_empty()
        && valor
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valido.then(|| format!("fb.{indice}.{horario_ms}.{valor}"))
}

/// O `fbc` do cookie `_fbc`, se estiver no formato da Meta. Caso contrário, `None`.
#[must_use]
pub fn fbc_valido(valor: &str) -> Option<&str> {
    let partes: Vec<&str> = valor.split('.').collect();
    let [prefixo, indice, horario, clid] = partes.as_slice() else {
        return None;
    };
    let clid_ok = !clid.is_empty()
        && clid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    (*prefixo == "fb" && so_digitos(indice) && so_digitos(horario) && clid_ok).then_some(valor)
}

/// O `fbp` do cookie `_fbp`, se estiver no formato da Meta. Caso contrário, `None`.
#[must_use]
pub fn fbp_valido(valor: &str) -> Option<&str> {
    let partes: Vec<&str> = valor.split('.').collect();
    let [prefixo, indice, horario, aleatorio] = partes.as_slice() else {
        return None;
    };
    let ok = *prefixo == "fb" && so_digitos(indice) && so_digitos(horario) && so_digitos(aleatorio);
    ok.then_some(valor)
}

/// Escolhe o `fbc` a usar: o do cookie `_fbc` se for válido, senão o montado do `fbclid`.
#[must_use]
pub fn fbc_escolhido(
    cookie: Option<&str>,
    fbclid: Option<&str>,
    horario_ms: u64,
    indice: u8,
) -> Option<String> {
    cookie
        .and_then(fbc_valido)
        .map(str::to_string)
        .or_else(|| fbclid.and_then(|f| fbc_do_fbclid(f, horario_ms, indice)))
}

fn so_digitos(texto: &str) -> bool {
    !texto.is_empty() && texto.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn fbc_do_fbclid_monta_no_formato_da_meta() {
        assert_eq!(
            fbc_do_fbclid("AbCd", 1_554_763_741_205, 1).as_deref(),
            Some("fb.1.1554763741205.AbCd")
        );
    }

    #[test]
    fn fbc_do_fbclid_recusa_vazio_e_caractere_estranho() {
        assert!(fbc_do_fbclid("   ", 1, 1).is_none());
        assert!(fbc_do_fbclid("a.b", 1, 1).is_none());
        assert!(fbc_do_fbclid("a b", 1, 1).is_none());
    }

    #[test]
    fn fbc_valido_aceita_o_formato_da_meta() {
        assert_eq!(
            fbc_valido("fb.1.1554763741205.AbCdEf-_9"),
            Some("fb.1.1554763741205.AbCdEf-_9")
        );
    }

    #[test]
    fn fbc_valido_recusa_formato_errado() {
        for ruim in [
            "",
            "fb.1.123",
            "xx.1.123.abc",
            "fb.a.123.abc",
            "fb.1.b.abc",
            "fb.1.123.",
            "fb.1.123.a.b",
            "fb.1.123.a<b",
        ] {
            assert!(fbc_valido(ruim).is_none(), "{ruim}");
        }
    }

    #[test]
    fn fbp_valido_aceita_e_recusa() {
        assert!(fbp_valido("fb.1.1558571054389.1098115397").is_some());
        for ruim in [
            "fb.1.1558571054389",
            "fb.1.x.1",
            "fb.1.2.",
            "xb.1.2.3",
            "fb.1.2.3.4",
        ] {
            assert!(fbp_valido(ruim).is_none(), "{ruim}");
        }
    }

    #[test]
    fn fbc_escolhido_prefere_o_cookie_valido() {
        let escolhido = fbc_escolhido(Some("fb.1.10.DoCookie"), Some("OutroClid"), 99, 1);
        assert_eq!(escolhido.as_deref(), Some("fb.1.10.DoCookie"));
    }

    #[test]
    fn fbc_escolhido_monta_do_fbclid_quando_o_cookie_falta_ou_e_ruim() {
        assert_eq!(
            fbc_escolhido(None, Some("Clid1"), 99, 2).as_deref(),
            Some("fb.2.99.Clid1")
        );
        assert_eq!(
            fbc_escolhido(Some("lixo"), Some("Clid1"), 99, 1).as_deref(),
            Some("fb.1.99.Clid1")
        );
    }

    #[test]
    fn fbc_escolhido_sem_nada_da_none() {
        assert!(fbc_escolhido(None, None, 1, 1).is_none());
        assert!(fbc_escolhido(Some("lixo"), None, 1, 1).is_none());
    }

    #[test]
    fn indice_padrao_e_1() {
        assert_eq!(INDICE_PADRAO, 1);
    }
}
