# Leitor experimental de esquemas ISIS/Proteus

Investigação de 22/10/2026 e implementação original em `crates/eda-import/src/dsn.rs`.
Os seis arquivos externos do usuário foram apenas lidos. Nenhum executável Proteus
foi executado e nenhum código ou desenho proprietário foi incorporado ao repositório.
As descrições abaixo foram obtidas por comparação de dados e validação estrutural.

## Escopo comprovado

O leitor reconhece a assinatura `ISIS SCHEMATIC FILE` seguida de `0x1A` e localiza
a primeira seção `ISIS CIRCUIT FILE`/`OBJECT DATA`. Antes dessa seção existem
bibliotecas embutidas. Essas definições não devem ser tratadas como componentes
instanciados. O segundo cabeçalho `ISIS CIRCUIT FILE` observado nos exemplos contém
dados de circuito/modelos e encerra o trecho de esquema processado.

| Exemplo externo | Componentes recuperados | Pinos | Fios | Junções | Labels, incluindo terra |
| --- | ---: | ---: | ---: | ---: | ---: |
| Circuito RC | 4 | 8 | 10 | 3 | 1 |
| Portas lógicas | 14 | 42 | 61 | 21 | 1 |
| Multiplexador | 12 | 36 | 51 | 18 | 1 |
| Flip-flop RS | 8 | 24 | 36 | 12 | 0 |
| Inversor CMOS | 6 | 14 | 22 | 6 | 1 |
| Somador completo | 42 | 126 | 192 | 79 | 33 |

Todos os 250 pinos recuperados coincidiram **exatamente** com vértices dos fios
dos respectivos arquivos. Não houve busca pelo fio mais próximo nem ajuste por
arredondamento. Essa correspondência não comprova equivalência com a netlist
completa do Proteus: existem geradores e outros objetos fora do subconjunto lido.

## Bibliotecas e nós de conexão

Uma definição é identificada pelo nome com comprimento `u8`, seguido de um campo
de 4 bytes e um tamanho `u32` little-endian. O tamanho inclui seus próprios 4 bytes.
O leitor valida o intervalo e a presença de `$MKRORIGIN` antes de aceitar o bloco.

Marcadores de origem e nó têm `i32 x`, `i32 y`, `i16 ângulo` e `u16 flags` nos
12 bytes anteriores ao nome terminado em NUL. Os pinos de símbolo referenciam
templates como `$PINSHORT` e `$PINDEFAULT`; após o nome do template há duas strings
NUL: nome e número do pino. Quando o número é vazio, preserva-se o nome como número.

O ponto elétrico é `$MKRNODE` do template, relativo a `$MKRORIGIN`, transformado
pela instância do pino e depois pela instância do componente. Os espelhos são
aplicados antes da rotação. Bit 0 espelha X; bit 1 espelha Y. Os ângulos observados
são em décimos de grau. O leitor aceita apenas múltiplos de 900 para manter a
transformação inteira exata; outras transformações produzem diagnóstico e pinos
incompletos.

Exemplo observado: `$PINSHORT` tem nó `(-254000,0)`, mas essa distância é lida da
definição embutida, nunca fixada no código. O resistor usa pinos em `(254000,0)`
e `(1016000,0)`; o segundo tem espelho X. Os extremos elétricos resultantes são
`(0,0)` e `(1270000,0)` antes da transformação do componente.

## Componentes

O marcador de atributo `PROPERTIES\0` é seguido por quatro bytes nulos e um
comprimento `u32`. Nos exemplos, o texto das propriedades aparece diretamente
após esse comprimento ou já está no atributo precedente. O leitor valida o
cabeçalho de instância em ambas as posições, sem interpretar bytes arbitrários
como propriedades.

O subconjunto de instância reconhecido contém tipo `u16` 2 ou 3, nome de símbolo
com comprimento `u8`, posição `i32 x/y`, ângulo `i16`, flags `u16` e ID `u32`.
Os atributos anteriores `COMPONENT ID` e `COMPONENT VALUE` são associados à
instância, mantendo valores e referências originais. `source_id` permite cruzar
os registros com o CDB, quando esse ID também estiver disponível lá.

O deslocamento entre texto contado de atributo e fonte tem 34 bytes nos registros
estudados. O leitor verifica as tags `FF`, o limite contado, a string da fonte e
o marcador do atributo. Formatos de texto alternativos podem exigir extensão.

## Fios, nomes de rede e junções

Um fio começa pelo marcador `02 7F WIRE 00`, seguido de `u16` quantidade de
atributos. Cada atributo `WIRE LABEL` preserva seu texto; depois dos atributos
há `u16` quantidade de vértices e pares `i32 x/y`. No somador, 28 dos 192 fios
possuem atributos, somando 32 labels; ignorá-los perde ligações e nomes de rede.

Labels são associados ao primeiro vértice do próprio fio, que pertence à mesma
rede elétrica. A posição de apresentação do texto não é usada como conexão.
Repetições de um mesmo nome no mesmo fio são preservadas no IR e podem ser
deduplicadas na integração com o domínio.

Uma junção explícita tem tag `01`, posição e uma sequência de referências `u32`
a registros de fio, encerrada por zero. As referências apontam para o início do
estilo, 23 bytes antes do marcador do fio. O leitor exige três referências válidas
distintas ou mais e que a coordenada exista em todos os fios referenciados.
Cruzamentos visuais, sem esse registro, não geram junções.

## Terra

A instância de `$TERGROUND` é validada pela tag `10`, posição, ângulo e flags.
O ponto elétrico é lido dos marcadores de sua biblioteca. Nos exemplos, o nó
local está em `(0,254000)`, diferente da origem do desenho. O resultado é um
label de rede `0`, preservando o vínculo de terra sem copiar o símbolo gráfico.
O IR usa `DsnLabel.is_ground=true` apenas para esse terminal decodificado. Um
nome de rede textual `0` não é classificado automaticamente como terra físico.

## Limites e validação

- Entrada máxima: 4 MiB; até 20 mil componentes, 20 mil fios e 100 mil vértices.
- Limites de intervalos são conferidos; falhas fatais incluem deslocamento em hexadecimal.
- A resolução dos vínculos de junção usa índice por deslocamento.
- Bibliotecas de pinos já processadas são mantidas em cache por nome.
- Até 128 nomes distintos de biblioteca, com orçamento acumulado de 64 MiB de
  varredura, incluindo candidatos sobrepostos. Buscas sem resultado também ficam
  no cache. Símbolos com mais de 64 pinos são diagnosticados e não recebem um
  subconjunto de pinos aparentemente completo. Textos NUL são limitados a 255
  bytes; propriedades por componente, a 64 KiB.
- Oito testes Rust com dados sintéticos originais cobrem fio, truncamento,
  transformação, biblioteca embutida, número do pino, instância de componente,
  limites de recursos, limites de atributos/seções e 2 mil casos determinísticos
  de bytes aleatórios com marcadores reconhecidos. Nenhum desses casos provocou panic.
- Harness externo em `C:\ghidra\build\analysis\dsn_harness.rs` leu os seis DSN;
  saída em `dsn_harness.txt`. Esses arquivos não fazem parte dos fixtures do projeto.

Ainda não há decodificação integral de folhas adicionais, barramentos, terminais
além do terra, geradores, probes, desenhos, instrumentos ou configuração de
simulação. O leitor sempre emite aviso de compatibilidade parcial. A presença de
todos os pinos de um componente não significa que todos os objetos do projeto
foram importados ou que ele está pronto para simular.
Na conversão para ProtoCraft, `electrical_complete` permanece falso para esses
projetos importados: a preservação da geometria não comprova cobertura integral
dos objetos e configurações elétricas do arquivo.
