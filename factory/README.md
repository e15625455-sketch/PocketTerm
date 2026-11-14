# PocketTerm package factory

The workflow builds the current `python` recipe and its complete dependency closure for `aarch64`, after setting the upstream `TERMUX_APP__PACKAGE_NAME` to `com.pocketterm.android`. The upstream properties derive these exact locations:

- `TERMUX_APP__DATA_DIR=/data/data/com.pocketterm.android`
- `TERMUX__ROOTFS=/data/data/com.pocketterm.android/files`
- `TERMUX__HOME=/data/data/com.pocketterm.android/files/home`
- `TERMUX__PREFIX=/data/data/com.pocketterm.android/files/usr`

It uses `build-package.sh -F` to build dependencies from source instead of reusing packages from the `com.termux` repository. The job timeout is 350 minutes, below the six-hour Actions limit.

## GitHub setup required

In the PocketTerm app repository's Actions secrets, provide:

- `PT_SIGNING_KEY`: the Ed25519 private key in unencrypted PKCS#8 PEM form. The workflow writes it only to a mode-0600 runner temporary file, checks its derived public key against the key compiled into the app, uses it to create a 64-byte raw Ed25519 signature, and deletes the file. Do not add the key to this repository or logs.
- `PT_PUBLISH_TOKEN`: a fine-grained token scoped only to `e15625455-sketch/PocketTerm-pkg`, with repository contents read/write access. The workflow needs this because its built-in `GITHUB_TOKEN` cannot push to a different repository.

Enable GitHub Pages in `PocketTerm-pkg` with source branch `gh-pages` and folder `/ (root)`. That repository currently has only its `main` README and Pages is not enabled. The first manual workflow run creates `gh-pages`; enable the source before or immediately after that run. The resulting base URL is `https://e15625455-sketch.github.io/PocketTerm-pkg/`.

The workflow uploads the exact signed repository as a 14-day Actions artifact before publishing. `create-repository.sh` enforces a 900 MB site budget, below GitHub Pages' 1 GB published-site cap. License notices and the pinned Termux recipe/patch directories are published under `licenses/` and `sources/`.

## Signing format

`Release.ed25519` is the raw 64-byte Ed25519 signature over the exact UTF-8 bytes of `dists/stable/Release`. This matches `verify_release` in `crates/pocketterm-core/src/mirrors.rs`. The Release lists SHA-256 hashes and byte sizes for the uncompressed and gzip-compressed `Packages` indexes.

## Comandos do cliente

Depois que o workflow publicar a primeira versão e o app tiver executado `pkg update`, os comandos são `pkg install python`, `pkg list`, `pkg verify` e `pkg remove python`. A versão e as dependências vêm do `Packages` assinado. A instalação valida o hash do `.deb`, recusa entradas fora de `data/data/com.pocketterm.android/files/usr` e mantém rollback dos arquivos substituídos. Ainda é obrigatório confirmar no aparelho que o build resultante funciona com `/system/bin/linker64` e com a política de `LD_PRELOAD`; a factory não é prova desse gate.
