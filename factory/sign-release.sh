#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: sign-release.sh PRIVATE_KEY_PEM RELEASE_FILE SIGNATURE_OUTPUT" >&2
  exit 2
fi

key_file=$1
release_file=$2
signature_file=$3
expected_public_key=cc086f7a8574b4bc230a41b2c148fe1a239d3c07ea9a6842f679862fe50a3068

# Verify that the GitHub secret matches the public key embedded in PocketTerm.
# Only the public DER is read from openssl output; private material stays in the key file.
actual_public_key=$(openssl pkey -in "$key_file" -pubout -outform DER 2>/dev/null | tail -c 32 | od -An -v -tx1 | tr -d ' \n')
if [[ "$actual_public_key" != "$expected_public_key" ]]; then
  echo "PT_SIGNING_KEY does not match PocketTerm's embedded Ed25519 public key" >&2
  exit 1
fi

openssl pkeyutl -sign -rawin -inkey "$key_file" -in "$release_file" -out "$signature_file"
size=$(wc -c < "$signature_file" | tr -d ' ')
if [[ "$size" != 64 ]]; then
  echo "Ed25519 signature must be 64 raw bytes; got $size" >&2
  exit 1
fi
