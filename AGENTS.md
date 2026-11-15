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
- Pacotes e Python: cliente `pkg install/remove/list/verify` implementado para repositório assinado, com hash do `.deb`, dependências, extração confinada e rollback; factory de Python criada em `factory/`. O repositório público ainda não foi publicado, portanto instalação e execução de Python continuam não provadas. Os mirrors oficiais Termux seguem somente diagnósticos, sem instalar pacotes incompatíveis.
- Ambiente local: Termux arm64 no Galaxy M54 5G; JDK 17, Gradle 9.9.1, SDK/API 37 e AAPT2 em `.tools/`. Em validação anterior, fmt, Clippy e 12 testes Rust passaram; o harness do shim passou no Termux e `./gradlew assembleDebug` terminou com `BUILD SUCCESSFUL`. APK compilado localmente, sem reinstalação naquela etapa. Nenhum desses resultados locais substitui validação no aparelho.
- O usuário pediu commits pequenos por etapa; antes de cada commit executar fmt, clippy e test. Não reescrever histórico. Avisar e aguardar confirmação antes de qualquer nova interação ADB.

### Etapa da factory e do gerenciador de pacotes (implementada localmente; publicação pendente)
- Mirror instalável padrão: `https://e15625455-sketch.github.io/PocketTerm-pkg`, com a chave pública Ed25519 compilada em `crates/pocketterm-core/src/mirrors.rs`. Mirrors oficiais Termux continuam somente para diagnóstico e não podem ser usados para instalar pacotes no PocketTerm.
- `.github/workflows/package-factory.yml` oferece execução manual e semanal. `factory/` configura `TERMUX_APP__PACKAGE_NAME=com.pocketterm.android`, que deriva os caminhos `TERMUX_APP__DATA_DIR`, `TERMUX__ROOTFS`, `TERMUX__HOME` e `TERMUX__PREFIX`; constrói Python e toda a dependência aarch64 com `-F` (sem reutilizar pacotes do Termux), gera `Packages` e `Release`, assina `Release.ed25519` e publica `dists/`, `pool/`, avisos de licença e receitas/fontes disponíveis. O workflow tem timeout de 350 minutos e limita o site a 900 MB.
- A chave Ed25519 pinada foi atualizada a partir da PEM pública fornecida em 15/11/2026; sua chave raw hex é `8699d9826c65b6beb94b97ab996ec02e6a63e8eabecbb571e176e550a19ebd47`. O app e a factory usam o mesmo pin e a inicialização migra a configuração local antiga. A correspondência com o secret `PT_SIGNING_KEY` ainda precisa ser comprovada pelo workflow Actions.
- A assinatura da loja é feita somente pelo workflow `package-factory.yml` com o secret `PT_SIGNING_KEY`; o workflow confere que a chave corresponde à chave pública embutida. `validate-signing-key.yml` oferece validação manual mínima, sem build nem publicação: assina um Release temporário e valida a assinatura. A chave privada nunca deve ser pedida, lida ou commitada. `.gitignore` exclui `*.pem` e saídas/chaves locais da factory.
- `pkg install/remove/list/verify` está implementado em `crates/pocketterm-core/src/packages.rs`. O cliente valida índice/assinatura, confirma o SHA-256 do pacote contra o índice autenticado, resolve dependências, detecta ciclos, extrai `.deb` para o prefixo privado com limites e rejeição de traversal/links externos, registra hashes de arquivos e mantém journal de rollback da instalação. Não executa scripts do pacote automaticamente. Suporte de payload `.deb`: tar sem compressão, gzip e xz; zstd não é suportado.
- Validação Termux/local desta etapa: `cargo fmt --all`, Clippy (`-D warnings`) e `cargo test --workspace` passaram com 19 testes no core; `bash -n factory/*.sh`, teste mock de `configure-termux.sh`, `git diff --check` e smoke CLI de `pkg list`/`pkg verify` passaram. `pkg install python` foi recusado corretamente sem índice atualizado. Esses resultados não provam funcionamento no aparelho.
- Não executado: workflow Actions, geração/assinatura de Release com o secret real, publicação no Pages, download/instalação de pacote, APK atualizado, ADB ou validação Android de `python3 --version`/`python3 -m http.server`. Todos permanecem “não testado” no aparelho.
- Repositório do app confirmado pelo usuário: `https://github.com/e15625455-sketch/PocketTerm.git`. `origin` usa `git@github.com:e15625455-sketch/PocketTerm.git`.
- Na última leitura pública do remoto, `main` estava em `d79fe68` e incluía três commits após `2a59385`, além de `TESTE.yml` na raiz e alterações em `package-factory.yml`; preservar essas alterações remotas.
- A autenticação SSH passou a funcionar para `git ls-remote`; `git push --dry-run` para uma branch temporária indicou criação permitida. Um dry-run de `master:main` foi rejeitado por não ser fast-forward. O trabalho de integração deve partir do `origin/main` atualizado, sem force push.
- A branch `gh-pages` do repositório `e15625455-sketch/PocketTerm-pkg` ainda não existe. A publicação pelo workflow cria essa branch no primeiro push bem-sucedido. Depois de configurar autenticação do app e secrets Actions `PT_SIGNING_KEY` e `PT_PUBLISH_TOKEN`, enviar a branch local para `main`, executar `PocketTerm package factory`, e então habilitar Pages em `gh-pages` / raiz.
- Commits locais desta etapa: `71d44eb feat: criar factory de pacotes assinados`, `61dea1c feat: instalar pacotes autenticados no PocketTerm`, `c06f5ee fix: ajustar checkout do repositorio da loja` e `2a59385 docs: atualizar estado da factory de pacotes`. Os arquivos não relacionados já sem tracking (`codex-session-…md` e `docs/MIRROR_INVESTIGATION.md`) foram preservados.

### Validador isolado da chave de assinatura (15/11/2026)
- Criado `.github/workflows/validate-signing-key.yml`, disparado manualmente, timeout de 5 minutos. Valida que `PT_SIGNING_KEY` está presente, corresponde à chave pública embutida e consegue assinar/verificar um texto temporário; não faz build ou publicação.
- O workflow de validação ainda não foi executado no GitHub Actions; precisa ser disparado manualmente após estar no `main`. A chave secreta não foi lida localmente.
