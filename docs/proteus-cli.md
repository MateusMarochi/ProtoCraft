# Importar projetos Proteus pela CLI

O comando converte um arquivo Proteus do perfil experimental suportado para o
formato próprio `.protocraft`. O relatório informa contagens, avisos, dispositivos
sem suporte completo e se a interpretação elétrica ficou completa.

```powershell
cargo run -p eda-cli -- import-proteus 'C:\projetos\circuito.pdsprj' --output 'C:\projetos\circuito.protocraft' --report 'C:\projetos\importacao.json'
cargo run -p eda-cli -- import-proteus 'C:\projetos\ROOT.DSN' --output 'C:\projetos\esquema.protocraft'
```

Após compilar a versão portátil, também é possível usar `protocraft-cli.exe`
diretamente com os mesmos argumentos. As pastas de destino precisam existir.
`--output` é obrigatório e deve apontar para um nome `.protocraft`; `--report` é
opcional e exige um caminho novo. O relatório existente é preservado.

A CLI rejeita caminhos em que entrada, saída, backup ou relatório apontem para
o mesmo arquivo. Projetos `.protocraft` existentes passam pelo salvamento validado
do armazenamento, que conserva um backup do conteúdo válido anterior. Um erro
de leitura ou interpretação não gera uma saída parcial de projeto.

Os avisos aparecem no terminal mesmo sem `--report`. Componentes preservados
como genéricos não ganham automaticamente um modelo de simulação. A indicação
de interpretação elétrica completa também não comprova equivalência numérica
com o Proteus: revise os avisos, pinos e valores antes de simular.

Para testar as amostras externas locais sem adicioná-las ao repositório:

```powershell
$env:PROTOCRAFT_PROTEUS_SAMPLES = 'C:\Users\Mateus\Downloads\RESULTADO_EXEMPLOS_PROTEUS\PROJETOS'
cargo test -p eda-cli --test proteus_import
```

O teste compara contagens da saída com o relatório, reabre o `.protocraft`, exercita
ZIP e DSN solto e verifica que o projeto de entrada não foi alterado. Sem a
variável, executam os testes próprios de opções, proteção de caminhos e uma
importação completa pela CLI de um DSN sintético autoral com fio.
