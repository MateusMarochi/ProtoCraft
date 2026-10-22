# Formato de projetos e recuperação

Sim. `.protocraft` é o formato nativo próprio do ProtoCraft, diferente dos formatos do Proteus. É JSON UTF-8 legível e documentado, não um contêiner binário fechado. `format_version` é obrigatório e atualmente tem valor `6`. O restante do documento representa `Project`, com nome, mapas de componentes/fios/junções/labels e contador de IDs. Coordenadas são passos inteiros da grade; valores elétricos usam SI. A conectividade é derivada da geometria e não é persistida como uma segunda fonte de verdade.

A leitura e a gravação validam todas as invariantes de `Project::validate`. Arquivos acima de 8 MiB, JSON inválido, dados inconsistentes e versões diferentes de 1, 2, 3, 4, 5 ou 6 são rejeitados. Projetos v1/v2/v3/v4/v5 são migrados em memória para v6, preservando IDs, geometria e valores. Ao salvar por cima de um v1 válido, o backup mantém exatamente os bytes anteriores. Não há migração de versões futuras. O tamanho também é limitado durante a leitura para evitar crescimento do arquivo entre a inspeção e a leitura.

A versão 2 adiciona `Component.imported` opcional: origem, nome do dispositivo, valor original, propriedades inertes e pinos numerados com offsets inteiros. O tipo `Imported` representa dispositivos sem modelo nativo; simulação e SPICE os rejeitam. Pinos personalizados também podem existir em componentes nativos mapeados, preservando conexões ao mover/rotacionar. `Project.import_info` conserva notas e `electrical_complete`; importação parcial bloqueia o motor e informa a limitação no ERC. A interface usa os mesmos pinos do núcleo.

A versão 3 acrescenta `Component.parameters` tipado para formas de onda, chave, cursor, ganho, limites de saída e modelos genéricos. Campos omitidos recebem valores padrão validados. Novos tipos de componentes e geometrias de três terminais são descritos no [guia dos motores](simulation-engines.md). A extensão nativa continua sendo `.protocraft`. Configurações pessoais de motor e visualização não pertencem ao projeto.

A versão 4 acrescenta dez tipos de componentes e parâmetros MOS/histerese/atraso/estado inicial, com pinagens próprias de até 15 terminais. Arquivos v3 recebem padrões nos novos campos. A versão impede aplicativos antigos de interpretar modelos que desconhecem. O parser JSON usa conversão `f64` com round-trip exato. Consulte [simulação mista](mixed-simulation.md).

A versão 5 acrescenta `Component.device` e `Component.footprint` opcionais, incorporando snapshots reprodutíveis de biblioteca com origem/licença. `DeviceDefinition.id` é o identificador canônico e `requested_part` conserva a escolha do usuário; cada pino tem número, nome, tipo, posição e pads explícitos. `PinRef.pin` agora usa 16 bits; os índices antigos preservam os mesmos valores. Número de pad/pino físico é string, permitindo BGA alfanumérico.

`Project.board` opcional contém dimensões, regras, posições/face/rotação dos footprints, trilhas e vias. As coordenadas físicas são inteiras em nanômetros, separadas da grade inteira do esquema. Trilhas/vias referenciam `PinRef`; a netlist continua derivada. Footprints conservam corpo, pads, furos, camadas, polígonos, margens de máscara/pasta, desenho e exclusões. IDs, correspondência pino/pad e referências são validados antes de gravar. Campos ausentes em v1–v4 permanecem vazios; nenhuma PCB é inventada na migração. Veja [mcu-pcb.md](mcu-pcb.md).

O salvamento serializa e valida os dados antes de alterar arquivos. Ao substituir um projeto existente válido, primeiro grava uma cópia exata em `<arquivo>.bak`; depois grava o projeto em arquivo temporário na mesma pasta, sincroniza os dados e substitui o destino atomicamente. Falha antes da substituição preserva o projeto anterior. Há um único backup, substituído a cada salvamento bem sucedido; isto não é histórico completo. O formato não oferece coordenação de edições concorrentes em processos diferentes nem garantia de sincronização da entrada do diretório após falha de energia.

Se o projeto existente estiver corrompido ou tiver versão desconhecida, o salvamento é recusado, preservando o original. Para recuperar, carregue a cópia `.bak` pela aplicação, confira seu conteúdo e salve em **outro nome** `.protocraft`. Pela CLI, `protocraft-cli validate projeto.protocraft.bak` verifica a cópia. Se for restaurar o nome original manualmente, primeiro renomeie o arquivo defeituoso para preservá-lo e então copie o backup; a aplicação não faz restauração automática.

CSV de resultados usa formato longo UTF-8: `serie,unidade,tempo_s,valor`, com separador vírgula, ponto decimal e nomes/unidades entre aspas (aspas internas duplicadas). Cada linha é uma amostra; em DC, o campo de tempo é zero. O CSV também é substituído atomicamente e rejeita amostras não finitas. CSV não mantém backup e não deve ser usado como arquivo de projeto.

Exemplos da CLI:

```powershell
protocraft-cli examples examples
protocraft-cli validate examples/divisor.protocraft
protocraft-cli netlist examples/divisor.protocraft
protocraft-cli simulate examples/divisor.protocraft --csv divisor.csv
protocraft-cli simulate examples/rc.protocraft --transient 0.0001 0.02 --csv rc.csv
protocraft-cli spice examples/rc.protocraft --transient 0.0001 0.02 --output rc.cir
```

As exportações SPICE contêm texto do circuito; este comando não executa ngspice ou código externo.

## Versão 6 — trilhas, vias e furos mecânicos

`Board.holes` é um mapa de IDs `u64` para `MountingHole {id, position:{x,y}, drill_nm}`. São furos circulares não metalizados (NPTH), sem rede ou cobre. As coordenadas e os diâmetros usam nanômetros; diâmetro válido entre 0,1 e 50 mm. Versões anteriores recebem mapa vazio. As coleções de trilhas, vias e furos somadas têm limite de 20.000 objetos; os orçamentos do DRC são mais restritos conforme a geometria.

`UpdateTrack`, `UpdateVia`, `AddHole`, `UpdateHole` e `DeleteHole` passam pelo Engine, com validação atômica e undo/redo. Edições não mudam os IDs. Uma via é um furo metalizado associado à rede; um furo mecânico é independente de componentes e preservado ao remover um footprint.

Grade, ângulo de dobra e ferramentas de roteamento são estado de apresentação. Apenas os segmentos confirmados pertencem ao projeto; encerrar/cancelar a prévia não apaga os trechos anteriores. O formato não persiste uma segunda netlist de PCB.
