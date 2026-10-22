# STM32, ESP32 e PCB — versão 0.5

O suporte desta entrega é **físico e elétrico no esquemático**: nomes e números dos pinos, encapsulamento, dimensões, pads e associação às redes. Não há CPU, firmware, GPIO em execução ou periféricos simulados desses dispositivos. A simulação recusa explicitamente esses componentes.

## Cobertura incorporada

São **2.331 entradas aceitas**: **2.283 STM32** em 20 famílias e **48 entradas Espressif** entre SoCs, módulos e placas. O catálogo normaliza 543 conjuntos de pinos e 162 footprints, incluindo cinco footprints auxiliares para R/C e DIP-8. O maior símbolo tem 448 pinos; índices internos usam 16 bits e números físicos podem ser alfanuméricos, como as bolas de BGA.

| Grupo STM32 | Entradas |
|---|---:|
| C0 | 48 |
| F0 / F1 / F2 / F3 / F4 / F7 | 111 / 125 / 47 / 93 / 211 / 123 |
| G0 / G4 | 170 / 133 |
| H5 / H7 | 73 / 200 |
| L0 / L1 / L4 / L5 | 179 / 132 / 258 / 38 |
| MP1 | 84 |
| U0 / U5 | 48 / 159 |
| WB / WL | 35 / 16 |

As entradas Espressif abrangem variantes clássicas e das linhas S2, S3, C2/ESP8684, C3, C5, C6, H2 e P4 conforme a disponibilidade do catálogo. A lista exata, incluindo chip, módulo ou placa, é consultável pela CLI. Isto é uma cobertura ampla dos dados selecionados; não representa todos os produtos comercializados por ST/Espressif.

O [relatório completo](../assets/library/coverage.json) registra commits, contagens, hash do catálogo e rejeições. Uma entrada só é aceita quando todos os seus pinos encontram pads e não sobram pads elétricos sem mapeamento. Divergências não são completadas por suposição. Tentativas rejeitadas de uma fonte podem ter uma alternativa válida de outra fonte; `excluded` elimina esses casos e `source_rejections` preserva a auditoria.

O KiCad usa nomes como `STM32F103C8Tx` para agrupar sufixos. A busca também reconhece `STM32F103C8T6` quando existe uma única correspondência; preserva no projeto tanto o nome canônico como o código solicitado. Ela não troca encapsulamentos por semelhança de nome. Identificadores agrupados como `ESP32-C6-MINI-1/U` foram separados em `ESP32-C6-MINI-1` e `ESP32-C6-MINI-1U`, cada qual com seu footprint. Chip, módulo e DevKit são entradas distintas.

## Usar na interface

1. Na biblioteca **Dispositivos**, escolha **STM32 / ESP32** e pesquise modelo, família ou encapsulamento.
2. Selecione o dispositivo e clique no esquemático. O inspetor mostra pinos, dimensões do corpo, footprint, mapeamento pino → pad e datasheet. `R` gira; Ctrl+C/Ctrl+V copia os dados e atribui nova referência.
3. Abra **Visualizar → PCB — dimensões reais**, ou use **Abrir PCB e sincronizar** no inspetor.
4. **Criar / sincronizar footprints** preserva posições existentes e posiciona os componentes novos. Eles podem exigir reposicionamento ou aumento da placa; execute DRC.
5. Arraste os corpos para mover footprints, use **Girar 90°** e **Trocar face**. O movimento na PCB não move o símbolo no esquemático. A grade física inicial é de 0,25 mm e pode ser alterada; `F` enquadra, roda do mouse altera zoom e arrastar com botão do meio/direito move a vista.
6. Selecione **Trilha** ou pressione **X**, escolha camada, largura e modo 45°/90°/livre, e clique em pad, via ou trilha da camada ativa. **Espaço** inverte a dobra; **V** confirma até o cursor, insere uma via configurável e troca a camada; **H** ativa furos mecânicos NPTH. Selecione geometrias para editar/arrastar ou use Delete. Undo/redo inclui as alterações da PCB.
7. Execute **Verificar DRC e conexões**. As linhas de conexão pendentes são derivadas da mesma netlist usada no esquema.

Para componentes básicos, o inspetor oferece footprints do catálogo com a mesma numeração de pinos. Os auxiliares incluem R/C 0603 e 0805 e DIP-8. A atribuição é explícita e reversível; não transforma um componente genérico em um modelo caracterizado de fabricante.

O menu **Exemplos → STM32 + ESP32-S3 · pinagem e PCB** abre uma demonstração com LQFP-48 e módulo ESP32-S3-WROOM-1, alimentação/terra e duas redes UART. Ela contém footprints posicionados e conexões a rotear. Não é uma placa de aplicação completa: faltam, entre outros itens, desacoplamento, alimentação, programação e definição de boot/reset adequados à aplicação.

## Geometria e verificações

- Dados físicos são inteiros em nanômetros: **1 mm = 1.000.000 nm**. O contorno inicial é retangular, com duas camadas de cobre.
- Os projetos incorporam snapshots das definições e footprints. Abrir o arquivo em outra máquina preserva o significado, independentemente da instalação do catálogo.
- Cada pad mantém número, posição, tamanho, forma, rotação, camadas, furo, polígonos e margens de máscara/pasta. Pads térmicos repetidos com o mesmo número representam um único terminal elétrico, preservando as formas físicas separadas.
- O corpo vem do contorno `F.Fab` ou, nas bibliotecas que usam essa convenção, `Eco2.User`. A origem é mostrada no inspetor. Courtyard, marcações e áreas de exclusão são mantidos separadamente.
- Trilhas e vias guardam uma referência estável a um pino. O módulo de netlist deriva a rede; a PCB não cria uma segunda conectividade autoritativa.
- O DRC detecta footprint ausente, componente sem posição, cobre perto da borda, curto, folga insuficiente, anel de via inferior a 0,1 mm, sobreposição de áreas de montagem na mesma face, invasão de exclusão/antena e ilhas de cobre a conectar.

O DRC é inicial e conservador: pads arredondados/ovais usam o retângulo externo no cálculo de folgas; áreas de montagem usam caixas envolventes. Polígonos de pads customizados são preservados. Assim, podem ocorrer rejeições conservadoras. Não há zonas de cobre, ajuste de comprimento, pares diferenciais, impedância, regras de alta tensão, análise RF, tolerâncias mecânicas completas ou verificação 3D. Limites: 100.000 pinos por projeto, 10.000 objetos de cobre por análise DRC e 2.000 diagnósticos exibidos antes de pedir nova análise. A comparação de cobre e a busca de conexões têm orçamento estimado de dois milhões de operações; excesso retorna erro, sem aprovar a placa.

## Trilhas e furos

O editor possui ferramentas para roteamento manual 45°/90°/livre e furos mecânicos NPTH. Use **X** para trilhas, **V** para via durante o traçado, **H** para furos e **Del** para excluir a seleção. Largura, via, furação e grade são configuráveis. Consulte o [guia de trilhas e furos](manual-routing.md).

## Exportar e fabricar

**Exportar PCB KiCad** gera `.kicad_pcb` com pads, redes, trilhas, vias, contorno e exclusões, sem exigir KiCad instalado. **Exportar desenho SVG em escala** cria uma prévia com dimensões em milímetros. Esses comandos permitem revisar uma placa ainda incompleta; não declaram que ela está pronta para fabricar.

**Gerar Gerber / Excellon** usa um `kicad-cli.exe` externo escolhido explicitamente, em segundo plano. Primeiro exige DRC nativo sem erros; depois executa o DRC externo com severidade `error`. Só então gera nove camadas Gerber e os furos Excellon. Cada processo tem prazo de 60 segundos. A saída usa uma pasta nova, preservando arquivos existentes; se o DRC externo falhar, seus diagnósticos são guardados na pasta de destino. As preferências conservam o caminho da ferramenta.

O pacote de fabricação inclui PCB intermediária, relatórios dos dois DRCs, mapa de pinos/pads, logs, Gerbers e Excellon. Avisos, empilhamento, montagem, máscara/pasta, RF e regras da fábrica ainda precisam de revisão. Não há aprovação automática de engenharia. O gerador externo não é incluído no ZIP do ProtoCraft nem é exigido para edição/esquemático/PCB.

### Preset JLCPCB — duas camadas

Selecione **JLCPCB — 2 camadas** na seção **Fabricação e exportação** do painel PCB e clique em **Gerar ZIP JLCPCB…**. Este é o preset inicial; a escolha fica salva nas preferências. O caminho do KiCad CLI é solicitado na primeira geração. A pasta de saída contém **JLCPCB.zip**, pronto para selecionar no formulário de fabricação; o programa não envia arquivos à JLCPCB.

| Configuração | Preset |
| --- | --- |
| Gerber | RS-274X, sem atributos X2, precisão 4.6 |
| Extensões | Protel: `.GTL`, `.GBL`, `.GTS`, `.GBS`, `.GTO`, `.GBO`, `.GTP`, `.GBP`, `.GM1` |
| Unidades | mm em todos os arquivos |
| Furação | Excellon decimal, absoluta, PTH/NPTH separados, ranhuras em modo alternate |
| Escala e orientação | 1:1, sem espelhamento adicional |
| Contorno | Retângulo fechado exclusivo em Edge.Cuts / `.GM1` |
| Entrega | ZIP com arquivos CAM na raiz, sem projetos, logs ou relatórios |

Antes de compactar, o preset confere cabeçalhos, unidades, formato absoluto e fechamento/dimensões do contorno. O arquivo `preset-jlcpcb.json`, fora do ZIP, registra as opções, dimensões e nomes incluídos. A opção **Padrão KiCad** conserva a exportação anterior em arquivos avulsos. O preset é de exportação: não muda espaçamentos, espessura, acabamento, seleção de materiais ou regras elétricas do projeto. BOM e posicionamento para montagem SMT não estão incluídos.

Referências: [orientações oficiais JLCPCB](https://jlcpcb.com/help/article/how-to-generate-gerber-and-drill-files-in-kicad-9) e [opções do KiCad CLI 10](https://docs.kicad.org/10.0/en/cli/cli.html). A visualização CAM da fábrica continua sendo a etapa de revisão antes de encomendar.

## CLI

```powershell
protocraft-cli devices --filter STM32F103 --limit 20
protocraft-cli devices --filter ESP32 --limit 100
protocraft-cli device STM32F103C8T6
protocraft-cli device-project STM32F103C8T6 --output stm32.protocraft
protocraft-cli run stm32.protocraft --cmd place_device --params '{"device":"ESP32-S3-WROOM-1","position":{"x":60,"y":0}}' --output duplo.protocraft
protocraft-cli pcb-info stm32.protocraft
protocraft-cli pcb-check stm32.protocraft
protocraft-cli pcb-export stm32.protocraft --format kicad --output stm32.kicad_pcb
protocraft-cli pcb-export stm32.protocraft --format svg --output stm32.svg
protocraft-cli pcb-manufacture placa-roteada.protocraft --preset jlcpcb --output fabricacao-nova --kicad 'C:\caminho\kicad-cli.exe'
protocraft.exe examples/stm32-esp32.protocraft --pcb
```

As ações `place_device`, `assign_footprint`, `set_board`, `place_on_board`, `remove_from_board`, `add_track`, `delete_track`, `add_via` e `delete_via` passam pelo mesmo motor de comandos e participam de lotes atômicos. `commands` lista seus parâmetros. Ações físicas usam nanômetros e índices de pino internos a partir de zero; `pcb-info` expõe os números físicos originais e seus pads para evitar confusão.

## Origem, licença e reprodução

Os dados foram convertidos das bibliotecas [KiCad](https://gitlab.com/kicad/libraries/kicad-symbols) e [Espressif](https://github.com/espressif/kicad-libraries), sob **CC-BY-SA 4.0 com exceção KiCad**. A coleção adaptada mantém essa licença, atribuição, fontes e dados legíveis em `assets/library/`. Isso é distinto da implementação Rust e da licença de um circuito criado pelo usuário. Consulte os avisos distribuídos e a [licença oficial das bibliotecas](https://www.kicad.org/libraries/license/).

`python scripts/build-device-library.py` reproduz a conversão com commits fixados e cache em `artifacts/device-sources/`. O catálogo é incorporado aos executáveis: após alterá-lo, é necessário recompilar. Não há download de bibliotecas durante o uso normal. O conversor não executa modelos ou scripts dos componentes.

A validação automatizada cobre todas as entradas aceitas estruturalmente, pinagem de referência do STM32F103C8, dimensões do módulo S3, BGA acima de 255 pinos, variantes de antena, persistência, CLI, roteamento, vias, exclusões e interação egui. A validação externa usa KiCad CLI oficial e uma placa de teste roteada, conforme o [registro de integração](validation.md). Os datasheets de referência incluem [ST STM32F103x8/xB](https://www.st.com/resource/en/datasheet/stm32f103c8.pdf) e os publicados pela [Espressif](https://www.espressif.com/en/support/documents/technical-documents). O catálogo não foi conferido manualmente contra todos os datasheets nem testado em placas físicas.

Na máquina desta entrega, a ferramenta oficial de validação está em `artifacts/kicad-tools/portable/bin/kicad-cli.exe` dentro do repositório. Ela foi extraída de instalador oficial com SHA-256 e assinatura Authenticode conferidos. Essa pasta é local e não integra o ZIP portátil.

Grupos explícitos de pinos do KiCad 10, como `[1,15,38,39]`, são expandidos nos números físicos individuais. Pinos declarados NC permanecem no símbolo/footprint, sem criar redes ou conexões fictícias; o ERC acusa fio/rótulo ligado a NC e o núcleo recusa usá-lo como âncora de trilha. O ESP32-WROOM-32E e suas variantes UE/R2 fazem parte dessa cobertura.
