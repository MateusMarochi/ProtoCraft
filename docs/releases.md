# Publicar uma versão

As publicações automáticas atuais geram os pacotes principais para Windows x64:

- Instalador `.exe` criado com Inno Setup.
- Arquivo `.zip` portátil com o aplicativo, a CLI, exemplos, documentação e bibliotecas.
- Arquivo `SHA256SUMS.txt` com os hashes dos dois pacotes.

O workflow executa as verificações do workspace e compila em modo release antes de criar ou atualizar a release do GitHub. Tags com hífen, como `v0.6.1-rc.2`, são publicadas como pré-releases; tags sem hífen são releases estáveis. A versão removendo o `v` da tag é aplicada ao instalador e aos nomes dos pacotes.

## Criar uma publicação

1. Atualize a versão em `Cargo.toml` e `Cargo.lock`.
2. Envie o commit para `main`.
3. Crie e envie uma tag anotada que corresponda à versão:

   ```powershell
   git tag -a v0.6.1-rc.2 -m "ProtoCraft v0.6.1-rc.2"
   git push origin v0.6.1-rc.2
   ```

O envio da tag inicia `.github/workflows/release-windows.yml`. A execução aparece na aba **Actions**; quando terminar, os pacotes e os hashes ficam anexados à release correspondente.

As atualizações são instaladas executando o novo instalador sobre a versão anterior, desde que a identidade e o caminho de instalação definidos em `installer/ProtoCraft.iss` continuem estáveis. O campo `AppUpdatesURL` leva à página de releases; o aplicativo ainda não possui verificação automática de atualizações.
