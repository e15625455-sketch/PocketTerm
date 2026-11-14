# PocketTerm

## Objetivo e contexto
PocketTerm é o terminal e o backend de um IDE mobile chamado PocketIDE, para Android recente, sem root e sem APIs privadas. O backend é em Rust (`axum`, `tokio`), a casca Android é em Kotlin com WebView e o front web usa CodeMirror 6.

## Regras de plataforma
- `targetSdk` deve ser o mais recente estável; em 13/11/2026, a documentação do Android indica Android 16, API 36, como requisito atual para novos apps e atualizações. Não reduzir esse valor.
- Interceptação de `exec` somente nos processos filhos do terminal via `LD_PRELOAD`, nunca global.
- Binários nativos próprios em `jniLibs/<abi>/lib*.so`, com `extractNativeLibs="true"`, `useLegacyPackaging=true` e alinhamento de página de 16 KB.
- Nenhuma permissão de armazenamento: tudo em `filesDir`; importar/exportar somente via SAF. Não usar `MANAGE_EXTERNAL_STORAGE`.

## Segurança
- Servidor somente em `127.0.0.1`, porta aleatória e token por sessão; validar `Host` e `Origin`.
- Rede somente por HTTPS com rustls, sem OpenSSL/native-tls.
- Nada baixado executa sozinho. Todo pacote exige índice assinado (Ed25519) e SHA-256 válido.
- Sem `unwrap()` fora de testes.

## Visual e mensagens do terminal
- Visual clássico preto e branco, sem ANSI, cores, emoji, caixas ou arte ASCII; sem zsh ou temas.
- Prompt fixo `pocket@android:~$ `.
- Mensagens em inglês; erros no formato `comando: mensagem`.

## Build e testes
Na raiz do workspace, executar `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace`.

## Commits
- Um commit por item numerado, em português, no formato `tipo: descrição` (`fix`, `feat`, `test`, `docs`, `refactor`).
- Antes de cada commit rodar fmt, clippy e test; não commitar com teste falhando.
- Não reescrever histórico nem fazer force push.

## Honestidade de validação
Nada é declarado como funcionando no aparelho sem teste em aparelho real. O que só rodou em Termux, emulador ou CI deve ficar registrado como “não provado no aparelho”.

## Estado e pendências
- Etapa 0: AGENTS.md criado. Estrutura, spike do linker/shim, gerenciador de arquivos, package manager e servidor Python pendentes.
- Ambiente: dispositivo ADB disponível (Galaxy M54 5G, Android 16/API 36, arm64); Gradle/Android SDK não encontrados no PATH. A execução normal do app ainda não foi validada.
- Item 3: linker testado em Android 16/API 36/arm64 via `run-as`, com saída de sucesso. Gate inconclusivo e não aprovado: o teste não ocorreu no processo normal do app; o pacote foi removido antes de testar o shim. Itens 4–6 não iniciados. Detalhes em `docs/GATE_RESULT.md`.
