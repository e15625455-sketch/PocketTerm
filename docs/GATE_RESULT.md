# Resultado do gate de execução (Item 3)

**Estado: inconclusivo; gate não aprovado.** O linker executou um ELF em `filesDir` no aparelho. A prova completa do shim no processo de uma instalação atual de PocketTerm não foi concluída. Itens 4–6 permanecem bloqueados por este gate.

## Aparelho e contexto

- Aparelho: Samsung Galaxy M54 5G (`SM-M546B`, `m54xxx`).
- Android: 16, API 36.
- ABI: `arm64-v8a`.
- App presente durante a primeira prova: `com.pocketide.pocketterm`, `targetSdk=36`, versão `0.2.0`, debuggable.
- Contexto da execução: `adb shell run-as`, que reportou `context=u:r:runas_app:s0:c208,c256,c512,c768`. Isso não é o processo normal `untrusted_app` do app e, portanto, não valida as restrições de execução vistas pelo processo real.

## Resultados reproduzidos

Um ELF PIE arm64 foi compilado com clang e copiado para o diretório privado do app (`files/gate/pt-gate`). A chamada:

```text
adb shell run-as com.pocketide.pocketterm /system/bin/linker64 /data/user/0/com.pocketide.pocketterm/files/gate/pt-gate
```

produziu:

```text
pt-gate: linker execution OK
exit code: 0
```

Um ELF pai chamou um ELF filho via `system()`, ambos no mesmo diretório privado, invocando o pai pelo linker. Resultado:

```text
pt-gate: child OK
exit code: 0
```

Execução direta do ELF filho e de um script `#!/system/bin/sh` em `files/gate` também funcionou sob `run-as`. Isso não reproduz o contexto do processo real do app e não prova que o shim é necessário ou suficiente.

## Shim

`native/libpt-exec.c` foi compilado localmente como `libpt-exec.so` com clang, sem erros. Não foi possível testar seu carregamento/intersceptação no aparelho: o pacote antigo foi removido antes dessa prova. Após a remoção, a tentativa de acesso retornou exatamente:

```text
run-as: unknown package: com.pocketide.pocketterm
```

O shim ainda é um spike incompleto: `execve`, `execv`, `execvp` e `posix_spawn` precisam de prova individual no processo real; resolução de interpretadores/shebangs e chamadas `system()`/subprocess precisam de prova no mesmo contexto.

## Decisão e próximo passo

Não há erro `Permission denied` nem AVC/SELinux observado nesta prova; a execução pelo linker retornou sucesso sob `run-as`. O resultado não permite concluir o comportamento do app com `targetSdk=36` em seu processo normal. Retomar o gate depois de instalar uma build debuggable de PocketTerm, copiar um ELF e um script para o `filesDir` real da instalação, executar as provas pelo terminal da Activity e coletar `logcat`/AVC. Não reduzir `targetSdk` nem usar root.

Se o gate falhar em Android recente, alternativas a avaliar são proot com rootfs ou VM pelo Android Virtualization Framework (AVF), sem contornar o modelo de segurança do Android.
