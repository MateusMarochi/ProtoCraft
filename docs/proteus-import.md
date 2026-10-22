# Importação experimental de projetos Proteus

O ProtoCraft converte um perfil restrito de projetos de esquemático Proteus ISIS para seu formato próprio `.protocraft`. A importação é pensada para inspeção e edição inicial. Não equivale a abrir o projeto no Proteus e não recupera todos os comportamentos do original.

## Usar

Na interface, escolha **Arquivo → Importar Proteus…** e selecione um `.pdsprj` ou `.dsn`. Ao terminar, confira a aba **Importação**, faça zoom/inspeção nos pinos e redes e salve como `.protocraft`. O arquivo original não é alterado.

Pela linha de comando:

```powershell
protocraft-cli import-proteus entrada.pdsprj --output convertido.protocraft --report importacao.json
protocraft-cli import-proteus ROOT.DSN --output convertido.protocraft
```

Para `.dsn` avulso, um arquivo `ROOT.CDB` com o mesmo nome-base e diretório é opcional e fornece referência, dispositivo, valores, números físicos dos pinos e propriedades. Os comandos e proteções de caminhos estão descritos em [proteus-cli.md](proteus-cli.md).

## Perfil lido

| Entrada | Comportamento verificado |
| --- | --- |
| `.pdsprj` | Contêiner ZIP do perfil `FILEVER=800`, `PROJECT.XML`, `ROOT.DSN` obrigatório e `ROOT.CDB` opcional |
| `.dsn` | Assinatura ISIS e uma seção de circuito; bibliotecas embutidas, instâncias, pinos, fios, labels, junções explícitas e terra |
| `.cdb` | Uma folha; referências, valores textuais, números físicos dos pinos e propriedades inertes |
| Componentes | Resistores, capacitores, indutores e fontes numéricas de tensão/corrente; dispositivos sem mapeamento são preservados como genéricos |
| Geometria | Posições inteiras exatas; espelhos e rotações suportados; fios diagonais e rotações não múltiplas de 90° são recusados |

O conversor não usa proximidade para decidir conectividade. Componentes sem modelo preservam posições, nomes/números dos pinos, valor e propriedades sem executar conteúdo externo. Não converte scripts, modelos, firmware, instrumentos, probes, geradores não reconhecidos, desenho decorativo, barramentos ou hierarquia.

## Estado elétrico e segurança

Todo projeto importado recebe `electrical_complete=false` até que o suporte a objetos e a equivalência elétrica sejam comprovados para aquele perfil. A marca é gravada no `.protocraft`; ao reabrir, continua visível. Simulação e exportação SPICE ficam bloqueadas. O ERC apresenta `INCOMPLETE_IMPORT` sem acusar falsamente a ausência de GND, fontes ou ilhas por causa de elementos que o parser pode ter omitido. Dispositivos genéricos também não recebem modelo elétrico inventado.

Entradas são tratadas como não confiáveis. Os leitores impõem limites de bytes, contagens, tamanho expandido, profundidade XML, tabelas, strings e trabalho de busca. O ZIP é lido em memória; outros membros são verificados e ignorados, nunca extraídos ou executados. Falha de leitura/conversão não produz saída `.protocraft` parcial.

## Amostras externas observadas

Os seis projetos de exemplo já disponíveis fora do repositório foram lidos e convertidos localmente; suas fontes e saídas não são distribuídas pelo Git. As contagens extraídas do DSN e CDB são:

| Projeto | Componentes no DSN | Pinos | Polilinhas de fio | Junções | Componentes `.protocraft` | Segmentos `.protocraft` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Circuito RC | 4 | 8 | 10 | 3 | 5 | 19 |
| Portas lógicas | 14 | 42 | 61 | 21 | 15 | 99 |
| Multiplexador | 12 | 36 | 51 | 18 | 13 | 85 |
| Flip-flop RS | 8 | 24 | 36 | 12 | 8 | 64 |
| Inversor CMOS | 6 | 14 | 22 | 6 | 7 | 36 |
| Somador completo | 42 | 126 | 192 | 79 | 43 | 318 |
| **Total** | **86** | **250** | **372** | **139** | **91** | **621** |

Os 250 pinos examinados coincidiram exatamente com vértices de fios nessas amostras; essa coincidência não demonstra que todos os objetos elétricos foram recuperados. Os CDB continham, respectivamente, 5/3, 18/14, 17/12, 12/8, 9/3 e 48/42 instâncias/peças. A ordem de enumeração não determina número de pino; o mapeamento Q2 do exemplo CMOS foi verificado como D=2, G=1, S=3.

Essas são seis amostras de um perfil, não um teste de compatibilidade geral com Proteus 8 ou 9. Veja os detalhes observados em [DSN](proteus-dsn.md) e [CDB](proteus-cdb.md). Os arquivos de origem ficam em `C:\Users\Mateus\Downloads\RESULTADO_EXEMPLOS_PROTEUS\PROJETOS`; importações de trabalho são gravadas em `imports/proteus/`, ignorada pelo Git e pelo pacote.
