# PocketTerm

## Objetivo e contexto
PocketTerm é um terminal Android independente, sem root e sem APIs privadas. Não é uma IDE e não depende do PocketIDE. O núcleo de comandos e arquivos é Rust; a integração e a interface nativa são Kotlin/JNI. A execução de ELF privado usa `/system/bin/linker64` em Android arm64. A interface deve continuar sendo um terminal simples.

## Regras de plataforma
- `targetSdk` deve ser o mais recente estável; em 13/11/2026, Android 17/API 37 é a versão estável mais recente conforme a documentação oficial; confirmar o requisito de distribuição antes de publicar. Não reduzir esse valor.
- Interceptação de `exec` somente nos processos filhos do terminal via `LD_PRELOAD`, nunca globalmente.
- Binários nativos próprios em `jniLibs/<abi>/lib*.so`, com `extractNativeLibs="true"`, `useLegacyPackaging=true` e alinhamento ELF de 16 KB.
- Nenhuma permissão ampla de armazenamento: tudo em `filesDir`; importar/exportar somente via SAF. Não usar `MANAGE_EXTERNAL_STORAGE`.

## Segurança
- Não manter servidor de aplicação local sem necessidade. Se houver um, somente em `127.0.0.1`, porta aleatória, token por sessão e validação de `Host`/`Origin`.
- Rede somente por HTTPS com rustls, sem OpenSSL/native-tls.
- Nada baixado executa sozinho. Pacotes e índices exigem assinatura autenticada e SHA-256 válido.
- Caminhos de arquivos sempre confinados ao `filesDir`; não seguir symlinks para fora.
- Sem `unwrap()` fora de testes.

## Visual e mensagens do terminal
- Visual clássico preto e branco, sem ANSI, cores, emoji, caixas ou arte ASCII; sem zsh ou temas.
- Prompt fixo `pocket@android:~$ `.
- Mensagens em inglês; erros no formato `comando: mensagem`.
- A interface Android deve ser terminal nativo; não adicionar WebView, frontend web, editor, LSP ou painéis de IDE.

## Build e testes
Na raiz do workspace, executar `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace`. Para gerar o APK localmente no Termux, usar `./gradlew assembleDebug`; o wrapper configura JDK 17, SDK, Gradle e AAPT2 extraídos em `.tools/`. `cargo build` sozinho compila apenas o workspace Rust.

## Commits
- Um commit por etapa numerada, em português, no formato `tipo: descrição` (`fix`, `feat`, `test`, `docs`, `refactor`).
- Antes de cada commit executar fmt, clippy e test; não commitar com testes falhando.
- Não reescrever histórico nem fazer force push. Não criar commit sem pedido explícito do usuário.

## Honestidade de validação
Nada é declarado como funcionando no aparelho sem teste em aparelho real. O que só rodou em Termux, emulador ou CI deve ficar registrado como “não provado no aparelho”. Build do APK não conclui o gate do linker.

## Estado e pendências
- Auditoria: identificados e removidos WebView/JavaScript bridge, editor web, árvore de arquivos, painel de output e serviço de servidor HTTP. Axum foi removido e a biblioteca JNI foi renomeada para `pocketterm-jni`. O `applicationId` agora é `com.pocketterm.android`.
- Etapa de arquitetura: Activity Android nativa com saída rolável, entrada de comando, histórico e botão de parada; núcleo Rust com comandos `pwd`, `ls`, `cd`, `mkdir`, `touch`, `cat`, `cp`, `mv`, `rm`, `rmdir`, `find` e execução de programas. Testes unitários cobrem comandos de arquivos, tokenização, traversal, cabeçalho ELF e interrupção de grupo de processos.
- Gate do linker: “parcial; não concluído”. A prova anterior no Galaxy M54 5G/Android 16/API 36/arm64 executou ELF PIE ARM64 pelo `/system/bin/linker64` no processo normal do app e imprimiu `gate: child ELF OK`, código 0. O harness atualizado do shim passou no Termux para `system()`, `execve`, `execv`, `execvp`, `posix_spawn`, shebang, ambiente e captura de saída/código; isso não prova funcionamento no app Android. A validação ADB está pendente de confirmação expressa porque o aparelho é compartilhado.
- Arquivos: comandos `pwd`, `ls`, `cd`, `mkdir`, `touch`, `cat`, `cp`, `mv`, `rm`, `rmdir` e `find` implementados. Testes incluem caminhos inválidos, alvos inexistentes, diretórios, traversal e symlink para fora.
- Mirrors: implementados testes HTTPS de conectividade, assinatura Ed25519 e SHA-256, seleção somente após validação e fallback. No teste Termux de 14/11/2026, os dois mirrors oficiais consultados responderam sem assinatura Ed25519 (`HTTP 404` no endpoint de assinatura); foram corretamente marcados inválidos. Nenhum índice foi confiado/atualizado.
- Pacotes e Python: instalação ainda não implementada para um repositório PocketTerm real. Os mirrors oficiais Termux não fornecem o formato/chave Ed25519 exigidos e os binários Termux dependem do applicationId/prefixo `com.termux`. `python3` e `python3 -m http.server` não foram validados; é necessário publicar ou fornecer um repositório de pacotes reconstruídos para PocketTerm e a chave pública confiável.
- Ambiente local: Termux arm64 no Galaxy M54 5G; JDK 17, Gradle 9.9.1, SDK/API 37 e AAPT2 em `.tools/`. Na etapa atual, fmt, Clippy e 12 testes Rust passaram; o harness do shim passou no Termux e `./gradlew assembleDebug` terminou com `BUILD SUCCESSFUL`. APK compilado localmente, sem reinstalação nesta etapa. Nenhum desses resultados locais substitui validação no aparelho.
- O usuário pediu commits pequenos por etapa; antes de cada commit executar fmt, clippy e test. Não reescrever histórico. Avisar e aguardar confirmação antes de qualquer nova interação ADB.
