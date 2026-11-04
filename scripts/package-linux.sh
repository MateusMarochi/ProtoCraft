#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="${1:-}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "Informe uma versão SemVer para o pacote Linux." >&2
  exit 2
fi

cd "$root"
manifest_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
if [[ "$manifest_version" != "$version" ]]; then
  echo "A versão solicitada ($version) diverge do Cargo.toml ($manifest_version)." >&2
  exit 2
fi

for binary in target/release/protocraft target/release/protocraft-cli; do
  if [[ ! -x "$binary" ]]; then
    echo "Executável release ausente ou sem permissão: $binary" >&2
    exit 1
  fi
done

mkdir -p dist
stage="$(mktemp -d "dist/.protocraft-linux-${version}.XXXXXX")"
trap 'rm -rf -- "$stage"' EXIT
payload="$stage/ProtoCraft"
mkdir -p "$payload/assets" "$payload/examples"

cp target/release/protocraft target/release/protocraft-cli \
  README.md README.en.md README.pt-BR.md NOTICE.md LICENSE-MIT LICENSE-APACHE \
  "$payload/"
cp assets/protocraft-logo.png assets/protocraft-icon.png "$payload/assets/"
cp -a assets/i18n assets/library "$payload/assets/"
cp -a docs "$payload/"

examples=(examples/*.protocraft)
if (( ${#examples[@]} == 0 )); then
  echo "Nenhum exemplo .protocraft foi encontrado." >&2
  exit 1
fi
cp "${examples[@]}" "$payload/examples/"

archive="protocraft-${version}-linux-x86_64.tar.gz"
tar -czf "dist/$archive" -C "$stage" ProtoCraft
(cd dist && sha256sum "$archive" > SHA256SUMS-linux.txt)
echo "Pacote Linux criado: dist/$archive"
