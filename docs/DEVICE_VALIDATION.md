# Validação por ambiente

## Termux local

- Data: 2026-11-14.
- Ambiente: Termux arm64 no Galaxy M54 5G, Android 16/API 36.
- `cargo fmt --all`: passou.
- `cargo clippy --workspace --all-targets -- -D warnings`: passou.
- `cargo test --workspace`: passou, 12 testes unitários.
- `cargo run -p pocketterm-cli -- -c 'mkdir sample'` e `find`: passou em diretório de smoke isolado sob `.tools/tmp`.
- `./gradlew assembleDebug`: `BUILD SUCCESSFUL`, código 0.
- APK anterior: `android/app/build/outputs/apk/debug/app-debug.apk`, SHA-256 `3c4ead294a8f6c7a4153f2d931efef02f160d59a43ec4e772991a4a77fea6e52`.

### Validação adicional desta etapa (Termux; não prova no aparelho)

- `native/tests/run_exec_shim_termux.sh`: passou para `execve`, `execv`, `execvp`, `posix_spawn`, `system()` e script shebang. O harness também conferiu variáveis herdadas, stdout/stderr separados e código de saída 23.
- `pkg mirrors`: listou `termux-primary` e `termux-cf` como endpoints diagnósticos.
- `pkg test`: ambos os endpoints retornaram HTTP 404 para a assinatura Ed25519 exigida e foram classificados como inválidos. `pkg update` terminou com código 1, sem confiar em índice algum.
- Os endpoints oficiais Termux não são repositório PocketTerm e não permitem concluir instalação de pacotes Python.
- `./gradlew assembleDebug`: `BUILD SUCCESSFUL` localmente em Termux; o APK foi compilado, não instalado nesta etapa.
- SHA-256 do APK desta etapa, após incluir permissão `INTERNET`: `9488e4889808071c8a549d7a498e7a2dca284309391a65acefdfe33df3133c8e`.

## Aparelho Galaxy M54 5G

- Data: 2026-11-14.
- Modelo: `SM-M546B`.
- Android: 16 (API 36).
- ABI: `arm64-v8a`.
- APK `com.pocketterm.android`, versão 0.1.0, targetSdk 37: instalado; Activity abriu e carregou a biblioteca JNI, sem crash observado no lançamento.
- Terminal chamou ELF PIE arm64 em `/data/user/0/com.pocketterm.android/files/usr/bin/gate-child` pelo `/system/bin/linker64`: stdout `gate: child ELF OK`, código 0. Esse teste foi executado pelo comando nativo no app instalado, não por `run-as`.
- Tentativa inicial com caminho relativo falhou com `./usr/bin/gate-child: command not found` porque o diretório atual era `filesDir/hi`; repetição com caminho absoluto dentro do `filesDir` passou.
- Comandos pai com `system()`, interceptação do shim `LD_PRELOAD`, `execve`/`execv`/`execvp`, `posix_spawn` e script shebang via filho: não testados nesta etapa. O harness executado em Termux não conta como prova no processo Android. Nenhuma interação ADB foi realizada nesta etapa; aguardando confirmação do usuário antes da próxima sequência.
- Captura do terminal mostrou `python: command not found`; Python não está instalado no prefixo do app. Nenhum Python HTTP foi executado.
- Logcat/AVC: não coletado para a prova do ELF; não inferir ausência de AVC.
- Mirrors, assinaturas de repositório, instalação de pacotes, SAF, saída stderr e cancelamento de filhos no aparelho: não testados.

## Resultado histórico separado

Uma versão anterior executou ELF e script via `adb shell run-as` no mesmo aparelho, fora do processo normal do app; isso não é a prova atual do shim. Detalhes e gate parcial em [GATE_RESULT.md](GATE_RESULT.md).

## Etapa loja PocketTerm (14/11/2026; desenvolvimento local)

- Implementação da factory e do cliente: arquivos locais em `factory/`, `.github/workflows/package-factory.yml` e `crates/pocketterm-core/src/packages.rs`.
- Os scripts shell foram verificados sintaticamente e o script de configuração foi exercitado com checkout mock do `termux-packages`; isso não compila os pacotes e não prova o workflow.
- `cargo fmt --all`: passou; `cargo clippy --workspace --all-targets -- -D warnings`: passou; `cargo test --workspace`: passou, 19 testes unitários no crate core e os demais crates sem falhas.
- `bash -n factory/*.sh`, teste mock de `configure-termux.sh` e `git diff --check`: passaram. Nenhum `actionlint`/validador YAML está instalado no ambiente.
- Smoke `cargo run -p pocketterm-cli -- -c 'pkg list'` e `pkg verify` passou em diretório temporário; `pkg install python` recusou como esperado sem índice atualizado (`no package index; run pkg update first`).
- Não houve execução do workflow Actions, publicação de Pages, download de pacote ou assinatura da Release com a chave privada (indisponível por projeto).
- Aparelho: nenhum ADB utilizado nesta etapa. `pkg update`, `pkg install python`, `python3 --version` e `python3 -m http.server` continuam não testados no aparelho.
- Dependências externas para primeira publicação: configurar `PT_SIGNING_KEY` e `PT_PUBLISH_TOKEN` como secrets Actions do repositório PocketTerm; habilitar Pages no repositório `PocketTerm-pkg` usando branch `gh-pages` e raiz. Depois executar `PocketTerm package factory` manualmente e revisar o artefato antes de validar no app.
