# Validação em aparelho

## Galaxy M54 5G

- Data: 2026-11-13.
- Android: 16 (API 36).
- ABI: `arm64-v8a`.
- `targetSdk` da instalação antiga presente durante o teste: 36.
- `targetSdk` configurado no projeto atual: 37 (Android 17 estável).
- Linker `/system/bin/linker64` executando ELF em diretório privado via `adb shell run-as`: saída `pt-gate: linker execution OK`, código 0.
- `system()` pai/filho via linker: saída `pt-gate: child OK`, código 0.
- ELF executado diretamente e script shebang executado por `run-as`: sucesso, porém fora do processo normal do app.
- Shim `LD_PRELOAD` no processo real do app: não testado.
- Execuções `execve`, `execv`, `execvp` e `posix_spawn` sob o processo do app: não testado.
- Teste Python HTTP, SAF, foreground service e fluxo completo: não testado.
- Erro de permissão/AVC: nenhum observado na execução via `run-as`; resultado para o processo normal do app não testado.

Veja [GATE_RESULT.md](GATE_RESULT.md) para comandos e contexto detalhados.
