# Publicar uma versão

As publicações automáticas geram pacotes para Windows x64 e Linux x86_64:

- Windows: instalador `.exe`, `.zip` portátil com aplicativo, CLI, exemplos, documentação e bibliotecas; hashes em `SHA256SUMS-windows.txt`.
- Linux: `.tar.gz` com aplicativo, CLI, exemplos, documentação e bibliotecas; hashes em `SHA256SUMS-linux.txt`.

Ao receber uma tag, o workflow cria primeiro a entrada da release e depois compila Windows e Linux em paralelo. Cada pacote executa as verificações do workspace; quando ambos terminam, os arquivos e checksums são anexados à release. Assim, a página e a tag ficam disponíveis antes dos binários. O título segue `ProtoCraft v<versão>` e os nomes dos arquivos seguem `protocraft-<versão>-windows-x64.exe`, `protocraft-<versão>-windows-x64-portable.zip` e `protocraft-<versão>-linux-x86_64.tar.gz`. O pacote Linux é um tarball x86_64, não um AppImage ou pacote `.deb`, e depende das bibliotecas de desktop disponíveis no sistema. Tags com hífen, como `v0.10.0-rc.1`, são pré-releases; tags sem hífen, como `v0.10.0`, são releases estáveis. A versão sem o `v` da tag deve coincidir com `Cargo.toml`.

## Criar uma publicação

1. Atualize a versão em `Cargo.toml` e `Cargo.lock`.
2. Envie o commit para `main`.
3. Crie e envie uma tag anotada que corresponda à versão:

   ```powershell
   git tag -a v0.6.1 -m "ProtoCraft v0.6.1"
   git push origin v0.6.1
   ```

O envio da tag inicia `.github/workflows/release.yml`. A entrada da release aparece primeiro; os dois pacotes são anexados quando os jobs de plataforma terminam na aba **Actions**.

As atualizações são instaladas executando o novo instalador sobre a versão anterior, desde que a identidade e o caminho de instalação definidos em `installer/ProtoCraft.iss` continuem estáveis. O campo `AppUpdatesURL` leva à página de releases; o aplicativo ainda não possui verificação automática de atualizações.
