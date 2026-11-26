//! Normalização e hash SHA-256 dos dados do cliente, do jeito que a Meta pede.
//!
//! Regra da documentação da Meta (parâmetros de informações do cliente): o dado é normalizado e depois
//! vira SHA-256 em hexadecimal minúsculo. O dado cru nunca sai da biblioteca.
//! Os vetores de teste (`john_smith@gmail.com`, `16505551212`, `mary`, `valéry`, `19970216`, `us`) são
//! os exemplos publicados pela própria Meta.

use sha2::{Digest, Sha256};

/// Os dados do cliente como chegam (sem normalizar). Cada campo é opcional.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dados<'a> {
    /// E-mail.
    pub email: Option<&'a str>,
    /// Telefone, com ou sem DDI.
    pub telefone: Option<&'a str>,
    /// Primeiro nome.
    pub nome: Option<&'a str>,
    /// Sobrenome.
    pub sobrenome: Option<&'a str>,
    /// Cidade.
    pub cidade: Option<&'a str>,
    /// Estado, de preferência a sigla (`SP`).
    pub estado: Option<&'a str>,
    /// CEP.
    pub cep: Option<&'a str>,
    /// País, de preferência o código ISO de duas letras (`BR`).
    pub pais: Option<&'a str>,
    /// Data de nascimento, em `AAAA-MM-DD`, `DD/MM/AAAA` ou `AAAAMMDD`.
    pub nascimento: Option<&'a str>,
    /// Identificador do cliente no seu sistema (ID de cadastro, por exemplo).
    pub externo: Option<&'a str>,
}

/// Os campos já normalizados, ainda sem hash. Cada um é `None` quando o dado não veio ou não serve.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Normalizados {
    /// `em`: e-mail minúsculo, sem espaço.
    pub em: Option<String>,
    /// `ph`: telefone só com dígitos e DDI.
    pub ph: Option<String>,
    /// `fn`: primeiro nome, minúsculo, sem pontuação.
    pub fn_: Option<String>,
    /// `ln`: sobrenome, minúsculo, sem pontuação.
    pub ln: Option<String>,
    /// `db`: data de nascimento, `AAAAMMDD`.
    pub db: Option<String>,
    /// `ct`: cidade, minúscula, sem espaço nem pontuação.
    pub ct: Option<String>,
    /// `st`: estado, sigla minúscula.
    pub st: Option<String>,
    /// `zp`: CEP, só letras e números, minúsculo.
    pub zp: Option<String>,
    /// `country`: país, código ISO de duas letras minúsculo.
    pub country: Option<String>,
    /// `external_id`: identificador do cliente, sem espaço nas pontas.
    pub external_id: Option<String>,
}

impl Dados<'_> {
    /// Normaliza cada campo. `ddi_padrao` é o DDI que entra quando o telefone não traz um (`55` para o Brasil).
    #[must_use]
    pub fn normalizados(&self, ddi_padrao: &str) -> Normalizados {
        Normalizados {
            em: self.email.and_then(email),
            ph: self.telefone.and_then(|t| telefone(t, ddi_padrao)),
            fn_: self.nome.and_then(nome),
            ln: self.sobrenome.and_then(nome),
            db: self.nascimento.and_then(nascimento),
            ct: self.cidade.and_then(cidade),
            st: self.estado.and_then(estado),
            zp: self.cep.and_then(cep),
            country: self.pais.and_then(pais),
            external_id: self.externo.and_then(externo),
        }
    }
}

/// SHA-256 do texto, em hexadecimal minúsculo de 64 caracteres.
#[must_use]
pub fn sha256_hex(texto: &str) -> String {
    use std::fmt::Write;
    let mut saida = String::with_capacity(64);
    for byte in Sha256::digest(texto.as_bytes()) {
        let _ = write!(saida, "{byte:02x}");
    }
    saida
}

/// E-mail: sem espaço nenhum, tudo minúsculo.
#[must_use]
pub fn email(valor: &str) -> Option<String> {
    let limpo: String = valor
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    (!limpo.is_empty()).then_some(limpo)
}

/// Telefone só com dígitos e DDI. Zeros à esquerda saem. Sem DDI explícito (nem `+` nem `00`), um número
/// de 10 ou 11 dígitos ganha o `ddi_padrao`.
#[must_use]
pub fn telefone(valor: &str, ddi_padrao: &str) -> Option<String> {
    let internacional = valor.trim_start().starts_with('+');
    let digitos: String = valor.chars().filter(char::is_ascii_digit).collect();
    let (tem_ddi, numero) = if let Some(resto) = digitos.strip_prefix("00") {
        (true, resto.to_string())
    } else {
        (internacional, digitos.clone())
    };
    let numero = numero.trim_start_matches('0');
    if numero.is_empty() {
        return None;
    }
    if tem_ddi || !(10..=11).contains(&numero.len()) {
        Some(numero.to_string())
    } else {
        Some(format!("{ddi_padrao}{numero}"))
    }
}

/// Nome (primeiro nome ou sobrenome): minúsculo, sem pontuação, letras com acento ficam, espaço simples.
#[must_use]
pub fn nome(valor: &str) -> Option<String> {
    let sem_pontuacao: String = valor
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let limpo = sem_pontuacao
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!limpo.is_empty()).then_some(limpo)
}

/// Cidade: minúscula, sem espaço nem pontuação (`São Paulo` vira `sãopaulo`).
#[must_use]
pub fn cidade(valor: &str) -> Option<String> {
    let limpa: String = valor
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    (!limpa.is_empty()).then_some(limpa)
}

/// Estado: a sigla de duas letras, minúscula. Outro formato fica de fora.
#[must_use]
pub fn estado(valor: &str) -> Option<String> {
    let sigla = valor.trim().to_lowercase();
    (sigla.chars().count() == 2 && sigla.chars().all(|c| c.is_ascii_lowercase())).then_some(sigla)
}

/// CEP: só letras e números, minúsculo (`01310-100` vira `01310100`).
#[must_use]
pub fn cep(valor: &str) -> Option<String> {
    let limpo: String = valor
        .to_lowercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    (!limpo.is_empty()).then_some(limpo)
}

/// País: código ISO de duas letras, minúsculo. Alguns nomes comuns viram o código.
#[must_use]
pub fn pais(valor: &str) -> Option<String> {
    let minusculo = valor.trim().to_lowercase();
    if minusculo.len() == 2 && minusculo.chars().all(|c| c.is_ascii_lowercase()) {
        return Some(minusculo);
    }
    let codigo = match minusculo.as_str() {
        "brasil" | "brazil" => "br",
        "estados unidos" | "united states" | "eua" => "us",
        "portugal" => "pt",
        "argentina" => "ar",
        _ => return None,
    };
    Some(codigo.to_string())
}

/// Data de nascimento em `AAAAMMDD`. Aceita `AAAA-MM-DD`, `DD/MM/AAAA` e `AAAAMMDD`. Ano de 1900 a 2100.
#[must_use]
pub fn nascimento(valor: &str) -> Option<String> {
    let v = valor.trim();
    let partes: Vec<&str> = if v.contains('/') {
        v.split('/').rev().collect()
    } else if v.contains('-') {
        v.split('-').collect()
    } else if v.len() == 8 && v.is_ascii() {
        vec![&v[0..4], &v[4..6], &v[6..8]]
    } else {
        return None;
    };
    let [ano, mes, dia] = partes.as_slice() else {
        return None;
    };
    let ano: u32 = ano.parse().ok()?;
    let mes: u32 = mes.parse().ok()?;
    let dia: u32 = dia.parse().ok()?;
    if !(1900..=2100).contains(&ano) || !(1..=12).contains(&mes) || !(1..=31).contains(&dia) {
        return None;
    }
    Some(format!("{ano:04}{mes:02}{dia:02}"))
}

/// Identificador do cliente: o texto sem espaço nas pontas. Vazio vira `None`.
#[must_use]
pub fn externo(valor: &str) -> Option<String> {
    let aparado = valor.trim();
    (!aparado.is_empty()).then(|| aparado.to_string())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn hash_bate_com_os_vetores_publicados_pela_meta() {
        assert_eq!(
            sha256_hex(&email("John_Smith@gmail.com").expect("e-mail")),
            "62a14e44f765419d10fea99367361a727c12365e2520f32218d505ed9aa0f62f"
        );
        assert_eq!(
            sha256_hex("16505551212"),
            "e323ec626319ca94ee8bff2e4c87cf613be6ea19919ed1364124e16807ab3176"
        );
        assert_eq!(
            sha256_hex("mary"),
            "6915771be1c5aa0c886870b6951b03d7eafc121fea0e80a5ea83beb7c449f4ec"
        );
        assert_eq!(
            sha256_hex("정"),
            "8fa8cd9c440be61d0151429310034083132b35975c4bea67fdd74158eb51db14"
        );
        assert_eq!(
            sha256_hex("valéry"),
            "08e1996b5dd49e62a4b4c010d44e4345592a863bb9f8e3976219bac29417149c"
        );
        assert_eq!(
            sha256_hex("19970216"),
            "01acdbf6ec7b4f478a225f1a246e5d6767eeab1a7ffa17f025265b5b94f40f0c"
        );
        assert_eq!(
            sha256_hex("us"),
            "79adb2a2fce5c6ba215fe5f27f532d4e7edbac4b6a5e09e1ef3a08084a904621"
        );
    }

    #[test]
    fn sha256_bate_com_o_padrao_conhecido_e_tem_64_caracteres() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(sha256_hex("").len(), 64);
    }

    #[test]
    fn email_fica_minusculo_e_sem_espaco() {
        assert_eq!(
            email("  Ana Maria@Exemplo.COM ").as_deref(),
            Some("anamaria@exemplo.com")
        );
        assert!(email("   ").is_none());
    }

    #[test]
    fn telefone_so_com_digitos_e_ddi() {
        assert_eq!(
            telefone("(650)555-1212", "1").as_deref(),
            Some("16505551212")
        );
        assert_eq!(
            telefone("+1 650 555 1212", "1").as_deref(),
            Some("16505551212")
        );
        assert_eq!(
            telefone("011 91234-5678", "55").as_deref(),
            Some("5511912345678")
        );
        assert_eq!(
            telefone("0055 11 91234-5678", "55").as_deref(),
            Some("5511912345678")
        );
        assert_eq!(
            telefone("+55 (11) 91234-5678", "55").as_deref(),
            Some("5511912345678")
        );
        assert_eq!(telefone("123", "55").as_deref(), Some("123"));
        assert!(telefone("000", "55").is_none());
        assert!(telefone("sem numero", "55").is_none());
    }

    #[test]
    fn nome_e_sobrenome_minusculos_sem_pontuacao() {
        assert_eq!(nome("Valéry").as_deref(), Some("valéry"));
        assert_eq!(
            nome("Ana-Maria  da  Silva!").as_deref(),
            Some("anamaria da silva")
        );
        assert_eq!(nome("정").as_deref(), Some("정"));
        assert!(nome("!!!").is_none());
    }

    #[test]
    fn cidade_sem_espaco_nem_pontuacao() {
        assert_eq!(cidade("São Paulo").as_deref(), Some("sãopaulo"));
        assert_eq!(cidade("Rio-de-Janeiro").as_deref(), Some("riodejaneiro"));
        assert!(cidade("   ").is_none());
    }

    #[test]
    fn estado_so_a_sigla_de_duas_letras() {
        assert_eq!(estado("SP").as_deref(), Some("sp"));
        assert_eq!(estado(" sp ").as_deref(), Some("sp"));
        assert!(estado("São Paulo").is_none());
        assert!(estado("S1").is_none());
    }

    #[test]
    fn cep_so_letras_e_numeros_minusculo() {
        assert_eq!(cep("01310-100").as_deref(), Some("01310100"));
        assert_eq!(cep("SW1A 1AA").as_deref(), Some("sw1a1aa"));
        assert!(cep("-").is_none());
    }

    #[test]
    fn pais_iso_ou_nome_comum() {
        assert_eq!(pais("BR").as_deref(), Some("br"));
        assert_eq!(pais("United States").as_deref(), Some("us"));
        assert_eq!(pais("Brasil").as_deref(), Some("br"));
        assert_eq!(pais("Brazil").as_deref(), Some("br"));
        assert_eq!(pais("estados unidos").as_deref(), Some("us"));
        assert_eq!(pais("EUA").as_deref(), Some("us"));
        assert_eq!(pais("Portugal").as_deref(), Some("pt"));
        assert_eq!(pais("Argentina").as_deref(), Some("ar"));
        assert!(pais("Marte").is_none());
        assert!(pais("b1").is_none());
    }

    #[test]
    fn nascimento_vira_aaaammdd_em_tres_formatos() {
        assert_eq!(nascimento("1997-02-16").as_deref(), Some("19970216"));
        assert_eq!(nascimento("16/02/1997").as_deref(), Some("19970216"));
        assert_eq!(nascimento("19970216").as_deref(), Some("19970216"));
    }

    #[test]
    fn nascimento_recusa_data_que_nao_existe_ou_nao_cabe() {
        for ruim in [
            "1997/02",
            "16/13/1997",
            "1899-01-01",
            "2101-01-01",
            "1997-02-00",
            "1997-02-32",
            "abcdefgh",
            "12345",
            "2001-02-03-04",
            "ééééé",
        ] {
            assert!(nascimento(ruim).is_none(), "{ruim}");
        }
    }

    #[test]
    fn externo_aparado_e_vazio_vira_none() {
        assert_eq!(externo("  id-9 ").as_deref(), Some("id-9"));
        assert!(externo("   ").is_none());
    }

    #[test]
    fn normalizados_preenche_cada_campo() {
        let dados = Dados {
            email: Some("A@B.COM"),
            telefone: Some("(11) 91234-5678"),
            nome: Some("Ana"),
            sobrenome: Some("Lima"),
            cidade: Some("São Paulo"),
            estado: Some("SP"),
            cep: Some("01310-100"),
            pais: Some("BR"),
            nascimento: Some("16/02/1997"),
            externo: Some("cliente-7"),
        };
        let n = dados.normalizados("55");
        assert_eq!(n.em.as_deref(), Some("a@b.com"));
        assert_eq!(n.ph.as_deref(), Some("5511912345678"));
        assert_eq!(n.fn_.as_deref(), Some("ana"));
        assert_eq!(n.ln.as_deref(), Some("lima"));
        assert_eq!(n.ct.as_deref(), Some("sãopaulo"));
        assert_eq!(n.st.as_deref(), Some("sp"));
        assert_eq!(n.zp.as_deref(), Some("01310100"));
        assert_eq!(n.country.as_deref(), Some("br"));
        assert_eq!(n.db.as_deref(), Some("19970216"));
        assert_eq!(n.external_id.as_deref(), Some("cliente-7"));
    }

    #[test]
    fn normalizados_sem_dado_fica_tudo_none() {
        let n = Dados::default().normalizados("55");
        assert_eq!(n, Normalizados::default());
    }
}
