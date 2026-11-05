# CLI do ProtoCraft

O executável `protocraft-cli` usa os mesmos crates de domínio que a interface desktop. `run` aplica comandos tipados pelo `Engine`, valida o projeto e pode salvar uma cópia em `.protocraft`, sem abrir janela.

`conversion-formats` lista a matriz de conversão atual. `convert` importa/exporta os subconjuntos descritos, recusa perdas por padrão e pode gravar um relatório JSON; leia a [matriz de formatos e limitações](conversion-formats.md) antes de converter projetos externos.

No Windows, `ProtoCraft-CLI.cmd` compila a versão release do workspace e encaminha os argumentos para a CLI. Ele pode ser chamado da raiz do repositório ou por outro terminal apontando para o caminho do `.cmd`.

## Criar e abrir projetos

`templates` lista os IDs e os nomes de todos os modelos. `new` cria um arquivo novo sem substituir um arquivo já existente; por padrão, cria uma folha esquemática vazia.

O CLI também cria diretamente o shield físico de exemplo da Raspberry Pi 4 Model B. Esse comando monta esquemático e PCB com header GPIO, saídas de teste identificadas, LEDs de alimentação, cantos de 3 mm e furação de montagem:

```powershell
ProtoCraft-CLI.cmd raspberry-pi-4-shield --output projetos\shield-pi4.protocraft
ProtoCraft-CLI.cmd pcb-check projetos\shield-pi4.protocraft
ProtoCraft-CLI.cmd pcb-export projetos\shield-pi4.protocraft --format kicad --output shield-pi4.kicad_pcb
```

Os furos usam a disposição nominal de 58 × 49 mm da Pi 4. A placa de exemplo tem 85 × 56 mm conforme solicitado; isso não garante compatibilidade com todo gabinete, HAT ou conector lateral. Revise posição/altura dos conectores antes de fabricar. Veja o [guia detalhado do shield](raspberry-pi-4-shield-prototype.md).

Para começar uma placa na geometria mecânica Raspberry Pi HAT, sem os componentes do shield de demonstração, use:

```powershell
ProtoCraft-CLI.cmd raspberry-pi-hat --output projetos\hat-base.protocraft
```

O template usa contorno de 65 × 56 mm para header through-hole, cantos de 3 mm, quatro furos de 2,75 mm no padrão 58 × 49 mm e o header GPIO completo. Ele é um ponto de partida mecânico: não inclui EEPROM, declaração de device tree, recortes de flex, keepouts de conectores PoE nem validação de conformidade HAT/HAT+.

Consulte [catálogo e limites Raspberry Pi](raspberry-pi-catalog.md) para a lista de pinagens incluídas e os comandos `devices` e `device`.

```powershell
ProtoCraft-CLI.cmd templates
ProtoCraft-CLI.cmd new --output projetos\meu-circuito.protocraft --name "Meu circuito" --open
ProtoCraft-CLI.cmd open projetos\meu-circuito.protocraft
```

O desktop precisa estar distribuído ao lado de `protocraft-cli.exe` para usar `--open` ou `open`. O atalho `Iniciar-ProtoCraft.cmd` continua disponível para abrir o aplicativo diretamente.

### Modelo 555 com LED perto de 1 Hz

O modelo cria uma fonte de 5 V, 555 astável, RA=47 kΩ, RB=47 kΩ, C1=10 µF, LED, resistor limitador de 680 Ω e bypass de 10 nF no pino CTRL. A estimativa pela equação clássica é `f ≈ 1.44 / ((RA + 2 RB) C)`, cerca de 1,02 Hz. A simulação usa o motor Misto e componentes genéricos.

```powershell
ProtoCraft-CLI.cmd new --template 555-led-1hz --output projetos\pisca-1hz.protocraft --open
ProtoCraft-CLI.cmd simulate projetos\pisca-1hz.protocraft --engine mixed --transient 0.00025 3 --csv projetos\pisca-1hz.csv
```

É possível informar `--ra-ohms`, `--rb-ohms`, `--c-farads` e `--led-resistor-ohms`. Use `--frequency-hz` para calcular C1 automaticamente a partir de RA e RB; ele não pode ser combinado com `--c-farads`. Os valores são em unidades SI. Por exemplo, `--frequency-hz 1` calcula C1 em aproximadamente 10,21 µF. A CLI informa a frequência teórica estimada e o CSV permite conferir o resultado transiente. O preset é um ponto de partida de esquemático, não uma PCB roteada.

Para um exemplo físico completo, com footprints, PCB de duas camadas, roteamento e instruções de fabricação, use [`555-led-1hz-pcb.protocraft`](../examples/555-led-1hz-pcb.protocraft) e consulte o [guia da placa 555](555-led-1hz-pcb.md).

O comando `examples <pasta>` continua gravando o conjunto completo, agora sem substituir nenhum arquivo existente; caso encontre um conflito, interrompe antes de criar os exemplos.

O conjunto inclui cinco projetos físicos nativos com classes de rede: placa básica de trilhas/vias/furos, 555 roteado em duas camadas, layout STM32+ESP32 com conexões pendentes para autorroteamento, shield Raspberry Pi 4 e fonte buck para Pi 4. O desktop também abre os mesmos exemplos no menu **Exemplos → Exemplos de PCB**. Consulte [a galeria PCB](pcb-examples.md) para os critérios e limitações de cada fixture.

## Inspecionar um projeto

```powershell
protocraft-cli info examples/divisor.protocraft
protocraft-cli commands
protocraft-cli commands --filter component
```

`info` imprime JSON com versão do formato, contagens, redes, ERC e estado da importação. `commands` lista IDs, descrições e parâmetros aceitos.

## Editar usando um arquivo de ações

Crie `acoes.json` como uma lista de comandos. Os nomes de `ComponentKind` seguem os identificadores JSON atuais, por exemplo `Resistor`, `VoltageSource` e `Ground`. Coordenadas são passos inteiros da grade.

```json
[
  {
    "command": "set_component",
    "params": { "id": 2, "reference": "R1", "value": 2200.0 }
  },
  {
    "command": "rename_project",
    "params": { "name": "Divisor revisado" }
  }
]
```

Execute e grave o resultado em um novo arquivo:

```powershell
protocraft-cli run examples/divisor.protocraft --actions acoes.json --output saida.protocraft
```

Também é possível fornecer comandos diretamente, repetindo `--cmd` e `--params`:

```powershell
protocraft-cli run examples/divisor.protocraft `
  --cmd set_component --params '{"id":2,"reference":"R1","value":2200}' `
  --cmd rename_project --params '{"name":"Divisor revisado"}' `
  --out saida.protocraft
```

`run` aceita no máximo 1.000 ações. Um lote é uma única transação: se um comando ou a validação final falhar, nenhuma alteração parcial é salva. `--output`/`--out` é opcional; sem ele, o JSON de resultado descreve a edição em memória. A entrada nunca pode ser usada como saída, e um projeto válido já existente mantém backup `.bak` pelo mesmo mecanismo de armazenamento do desktop. Uma edição que gere uma importação elétrica parcial continua bloqueada para simulação, inclusive pela CLI.

## Outros comandos

```powershell
protocraft-cli validate examples/divisor.protocraft
protocraft-cli netlist examples/divisor.protocraft
protocraft-cli simulate examples/divisor.protocraft
protocraft-cli simulate examples/rc.protocraft --transient 0.0001 0.02 --csv rc.csv
protocraft-cli spice examples/rc.protocraft --transient 0.0001 0.02 --output rc.cir
protocraft-cli import-proteus origem.pdsprj --output convertido.protocraft --report importacao.json
protocraft-cli import-falstad circuito.txt --output circuito.protocraft
protocraft-cli import-falstad 'https://www.falstad.com/s.py?s=dKcFWE' --output circuito-link.protocraft
```

`protocraft-cli --help` lista as opções principais. Os comandos atuais são finitos e headless; não há servidor persistente, MCP ou controle remoto nesta versão.

## Motores e modelos genéricos

`protocraft-cli engines` lista motores; `protocraft-cli components` lista tipos, valores e parâmetros em JSON. `simulate --engine stable|dynamic|mixed` seleciona o integrador. `run --cmd set_model --params '{"id":1,"parameters":{"frequency_hz":250,"offset_v":1}}'` substitui os parâmetros de um componente. Campos omitidos usam o padrão; o lote inteiro falha se algum parâmetro for inválido. O [guia dos motores](simulation-engines.md) contém os modelos, convenções e exemplos completos.


### Circuitos mistos

A CLI usa Estável por padrão; para 555, comparador, Schmitt, flip-flop ou contador, selecione Misto explicitamente:

```powershell
protocraft-cli simulate examples/555-astavel.protocraft --engine mixed --transient 0.00001 0.01 --csv timer.csv
protocraft-cli simulate examples/contador-decada.protocraft --engine mixed --transient 0.0001 0.02
```

O [guia de simulação mista](mixed-simulation.md) documenta pinagens, parâmetros, estados iniciais e limites. Componentes com estado não são exportados silenciosamente como aproximações SPICE; a exportação recusa esses modelos.


## Biblioteca física e PCB

`devices --filter termo --limit N --offset N` retorna JSON paginado; `device <modelo>` retorna pinagem, footprint e origem completos. `device-project <modelo> --output circuito.protocraft` cria um projeto com o dispositivo e PCB inicial. `pcb-info`, `pcb-check` e `pcb-export --format kicad|svg` inspecionam, verificam e exportam a placa. `pcb-manufacture <projeto> --output pasta-nova --kicad caminho/kicad-cli.exe [--preset jlcpcb|standard]` exige DRC aprovado e usa ferramenta externa explícita para Gerber/Excellon. O padrão `jlcpcb` gera `JLCPCB.zip` com RS-274X/Protel em mm, escala 1:1 e Excellon absoluto; `standard` conserva os arquivos avulsos anteriores. O JSON de resultado informa `preset` e `archive`. Consulte [o preset completo](mcu-pcb.md#preset-jlcpcb--duas-camadas).

`pcb-route <projeto> --output roteado.protocraft [--grid-mm 0.25] [--track-width-mm 0.25] [--max-expansions 600000] [--no-optimize]` executa o autorroteador nativo e grava uma cópia do projeto; a saída não pode substituir a entrada. O plano é limitado por orçamento, usa as duas camadas e valida o DRC antes de ser salvo. O JSON lista rotas realizadas e falhas restantes. Consulte [o guia de autorroteamento](manual-routing.md#autorroteamento-inicial).

`pcb-netclass <projeto> list` imprime as classes em JSON. `set` cria ou substitui uma classe, repetindo `--net` para cada rede e informando `--width-mm`, `--clearance-mm`, `--via-diameter-mm` e `--via-drill-mm`; `remove --name <classe>` exclui uma classe. Set e remove exigem `--output` com um caminho novo. As regras são mínimas, aplicam-se ao DRC e ao autorroteador e não representam impedância controlada ou regras específicas de fabricante. Consulte a [galeria de PCB](pcb-examples.md) para projetos prontos.

As ações físicas estão em `commands` e no [guia STM32/ESP32 e PCB](mcu-pcb.md). Toda coordenada física de ação usa nanômetros; a extensão nativa permanece `.protocraft` (v7). O CLI não emula firmware.

### Edição de trilhas e furos

As ações `update_track`, `update_via`, `add_hole`, `update_hole` e `delete_hole` estão disponíveis em `commands`. Todos os comprimentos são inteiros em nm. Exemplo de furo NPTH de 3,2 mm em X=5/Y=5 mm:

```powershell
protocraft-cli run placa.protocraft --cmd add_hole --params '{"hole":{"id":1,"position":{"x":5000000,"y":5000000},"drill_nm":3200000}}' --output placa-com-furo.protocraft
protocraft-cli pcb-check placa-com-furo.protocraft
```

`update_track` recebe `{track:{id,start,end,width_nm,layer,net}}`; `update_via` recebe `{via:{id,position,diameter_nm,drill_nm,net}}`; `update_hole` recebe o objeto `hole` completo. Atualizações exigem um ID já existente. A CLI valida o domínio, e `pcb-check` confere a geometria; a fabricação exige DRC aprovado. O desktop também usa o DRC antes de confirmar cada edição de cobre/furação.

## Servidor MCP

Para conectar um cliente compatível com MCP, inicie `protocraft-cli mcp --project <arquivo.protocraft>`. O processo atende um projeto headless por stdio e encerra quando o cliente fecha o pipe. Ferramentas, schemas, protocolo, controle por revisão e limites da sessão estão no [guia MCP](mcp.md).

## Fluxos ampliados de construção PCB

Consulte [Construção de PCB e relatórios de montagem](pcb-workflows.md) para `pcb-sync`, `pcb-sync-report`, `pcb-move`, `pcb-cleanup` e `pcb-assembly`, incluindo unidades e limitações. `pcb-info` agora inclui métricas geométricas por net. `examples <pasta>` exporta cinco projetos nativos de PCB, do básico a um conversor buck de 3 A. A fabricação gera a subpasta `montagem/` com BOM, posições e JSON de pendências, fora do ZIP de fabricação. A preferência `PROTOCRAFT_LANGUAGE=en|pt-BR|system` controla apenas o processo CLI.

### Fonte chaveada buck 12 V para Raspberry Pi 4

Para roteamento com ordens alternativas (`pcb-route --passes`), união de segmentos (`pcb-cleanup --merge-collinear`), revisão DFM (`pcb-dfm`) e limites opcionais na fabricação (`pcb-manufacture --dfm-limits`), consulte [otimização e revisão de fabricação](pcb-optimization.md).

O catálogo offline inclui o LM61460 e um cálculo de primeira ordem, ambos reutilizáveis em automações. O cálculo é restrito à faixa do controlador (entrada de 3–36 V, saída até 95% de VIN, corrente de saída dentro do limite de 6 A e frequência de 200 kHz–2,2 MHz). Valores de eficiência são hipóteses; não substituem simulação, protótipo ou medição. Os detalhes, critérios de seleção dos componentes e limites elétricos estão em [projeto do conversor buck](power-conversion.md).

```powershell
ProtoCraft-CLI.cmd power-design buck
ProtoCraft-CLI.cmd power-design buck --vin-v 12 --vout-v 5.1 --iout-a 3 --switching-hz 400000 --inductance-uh 4.7
ProtoCraft-CLI.cmd power-design copper --current-a 3 --copper-um 70 --length-mm 30 --drop-v 0.05
ProtoCraft-CLI.cmd buck-pi4-12v --output projetos\pi4-buck.protocraft
ProtoCraft-CLI.cmd pcb-check projetos\pi4-buck.protocraft
```

`power-design buck` calcula valores ideais de primeira ordem. `power-design copper` combina uma estimativa térmica histórica IPC-2221 com queda DC; não certifica ampacidade nem substitui cálculo térmico. O comando de projeto entrega uma PCB no perfil mecânico HAT de 65 × 56,5 mm, com quatro furos e header de 40 pinos. As classes nominais de entrada, saída e nó chaveado usam largura mínima de 0,8 mm; escapes locais de QFN são validados separadamente pelo DRC. A alimentação chega aos pinos físicos 2 e 4 de 5 V e aos pinos GND; não conecte uma fonte USB-C ao Pi em paralelo com esta placa. O DRC geométrico não valida capacidade de corrente, temperatura, EMI nem fabricação.


## Fluxos adicionais de projeto e análise

`pcb-assembly-variant`, `dc-sweep` e `simulate-measure` estão documentados com exemplos e limites em [advanced-workflows.md](advanced-workflows.md). `pcb-dfm` aceita `min_drill_clearance_nm` e considera a geometria orientada dos rasgos. Use `<comando> --help` para a sintaxe.
