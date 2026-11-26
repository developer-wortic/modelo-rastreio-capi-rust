# Como contribuir

Obrigado por querer ajudar. Este é um modelo pequeno de rastreio com a Conversions API da Meta, feito para ser fácil de ler e de adaptar.

## Antes de começar

- Para mudança grande, abra uma issue antes do código.
- Não ponha dado pessoal em issue ou pull request: nada de e-mail, telefone, IP, nome de cliente, token de acesso, chave de API ou `RASTREIO_SEGREDO`. Um evento recusado pela Meta vai pelo modelo "Evento recusado", sem os dados do cliente.

## Rodar as checagens

```sh
cargo fmt --check
cargo clippy -- -D warnings -W clippy::pedantic
cargo test
cargo llvm-cov --summary-only
```

O mesmo roda no GitHub Actions em cada push e em cada pull request. A cobertura de linhas precisa ficar em 100%.

## Regras do projeto

- Dependência nova precisa de motivo claro.
- Mudança de código vem com teste. Cobertura de linhas abaixo de 100% não entra.
- Vetor de hash vem da documentação publicada pela Meta, não de um cálculo feito aqui.
- Nenhum teste fala com a Meta. Os testes de envio usam o servidor falso de `src/suporte.rs`.
- JSON só na borda: o corpo da Graph API da Meta e as respostas HTTP de `/evento`. Dado entre módulos do projeto fica em estrutura Rust ou em texto próprio, como o cookie de atribuição.
- A tela continua em HTML puro, sem script.
- Nada de travessão nem de texto em inglês na interface. A documentação fica em português, com um resumo em inglês no README.
- O projeto usa a licença AAL (uso livre com créditos à Wortic). Ao contribuir, você concorda que sua contribuição entra sob a mesma licença.

## Pull request

- Diga o que muda e por quê.
- Inclua a saída de `cargo test` e de `cargo llvm-cov --summary-only`.
- Se mexer no evento, mostre o corpo gerado (com dados de exemplo da documentação da Meta) e diga quais campos mudaram.
