# Gate de execução pelo linker

**Estado: não concluído.** Em 14/11/2026, no processo normal do PocketTerm instalado, um ELF PIE arm64 em `filesDir/usr/bin` executou pelo linker com sucesso. O harness do shim passou em Termux, mas ainda não há prova dos processos filhos pelo shim no processo Android.

## Aparelho e build

- Samsung Galaxy M54 5G (`SM-M546B`).
- Android 16, API 36; ABI `arm64-v8a`.
- Pacote atual: `com.pocketterm.android`, versão 0.1.0, `targetSdk=37`, debuggable.
- APK compilado localmente no Termux com `./gradlew assembleDebug` e instalado no aparelho para o teste.

## Teste direto no processo do app

Um ELF PIE ARM64 foi compilado com clang Android, copiado para `/data/user/0/com.pocketterm.android/files/usr/bin/gate-child` e executado digitando o caminho absoluto no terminal nativo do app. O comando do núcleo Rust chamou `/system/bin/linker64 <binário>`.

Saída visível no terminal:

```text
gate: child ELF OK
```

Código de saída: 0. A tentativa anterior usando `./usr/bin/gate-child` não encontrou o arquivo porque o diretório atual era `filesDir/hi`; a repetição com caminho absoluto dentro de `filesDir` passou. Portanto, execução direta de ELF ARM64 pelo linker está provada neste aparelho e processo do app.

## Shim e filhos

O shim atualizado foi exercitado no Termux através de `native/tests/run_exec_shim_termux.sh`: `system()`, `execve`, `execv`, `execvp`, `posix_spawn` e shebang passaram no harness. Ele também verificou herança de ambiente, stdout/stderr e código de saída. Este resultado não é transferível ao processo Android: linker, SELinux e política do app podem mudar o comportamento. O usuário exigiu aviso e confirmação antes de qualquer interação ADB, então a validação em aparelho permanece pendente.

`Permission denied` não apareceu no teste direto. Logcat/AVC não foi coletado nesta rodada, então não concluir ausência de AVC.

## Decisão

Não marcar o gate como concluído sem testar no processo real do app: filho via `system()`, chamadas exec/spawn, shebang, ambiente, captura e interrupção. Não reduzir `targetSdk`, não usar root. O aparelho é compartilhado e nenhuma interação ADB desta etapa foi feita; aguardar confirmação antes do teste.

## Resultado do teste local em Termux

Em 14/11/2026, os dois endpoints Termux configurados foram acessíveis por HTTPS, mas não ofereceram a assinatura Ed25519 esperada (`HTTP 404`). O teste recusou ambos como inválidos e `pkg update` falhou sem aceitar índice. Isso demonstra a validação fail-closed, não um repositório funcional. Python e `python3 -m http.server` seguem não testados; dependem de repositório próprio com pacotes reconstruídos para o applicationId do PocketTerm e chave pública confiável.
