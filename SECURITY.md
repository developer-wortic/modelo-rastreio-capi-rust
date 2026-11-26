# Segurança

Se você achar uma falha de segurança, não abra issue pública. Relate em privado pelo [aviso de segurança do GitHub](https://github.com/developer-wortic/modelo-rastreio-capi-rust/security/advisories/new).

Inclua o que você encontrou, como reproduzir e o impacto que você vê.

Não envie token de acesso da Conversions API, chave de API, `RASTREIO_SEGREDO`, e-mail, telefone ou outro dado de cliente no relato. Se um token vazou, revogue-o na Events Manager e gere outro. Se o segredo do cookie vazou, troque `RASTREIO_SEGREDO`: os cookies antigos param de valer.

Áreas que pedem cuidado especial: a verificação do cookie de atribuição (`src/cookie.rs`), a captura de URL (`src/captura.rs`), o envio com token (`src/envio.rs`) e a tela, que escapa todo texto vindo de fora (`src/painel.rs`).

Só a versão mais recente da branch principal recebe correção. Vamos responder pelo mesmo canal.
