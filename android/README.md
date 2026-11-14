# PocketTerm Android terminal

O aplicativo é um terminal Android nativo, sem WebView nem frontend web. O núcleo Rust é acessado por JNI. O APK de debug é compilado localmente no Termux arm64:

```sh
./gradlew assembleDebug
```

O wrapper configura JDK 17, Gradle 9.9.1, SDK/API 37 e AAPT2 local em `.tools/`. O build compila `pocketterm-jni` e `libpt-exec.so` para `arm64-v8a` com alinhamento de página de 16 KB. O APK fica em `android/app/build/outputs/apk/debug/app-debug.apk`.

O núcleo implementa comandos de arquivos e prepara execução de ELF ARM64 pelo `/system/bin/linker64`. O shim é carregado somente nos processos filhos do terminal. No Galaxy M54 5G (Android 16/API 36/arm64), a build atual abriu e executou um ELF privado via linker no processo normal do app (`gate: child ELF OK`, código 0). O shim e os processos filhos ainda não foram provados. Mirrors e instalação de pacotes continuam pendentes até concluir o gate.
