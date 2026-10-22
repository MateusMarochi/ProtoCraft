# CLI do ProtoCraft

O executável `protocraft-cli` usa os mesmos crates de domínio que a interface desktop. `run` aplica comandos tipados pelo `Engine`, valida o projeto e pode salvar uma cópia em `.protocraft`, sem abrir janela.

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

As ações físicas estão em `commands` e no [guia STM32/ESP32 e PCB](mcu-pcb.md). Toda coordenada física de ação usa nanômetros; a extensão nativa permanece `.protocraft` (v6). O CLI não emula firmware.

### Edição de trilhas e furos

As ações `update_track`, `update_via`, `add_hole`, `update_hole` e `delete_hole` estão disponíveis em `commands`. Todos os comprimentos são inteiros em nm. Exemplo de furo NPTH de 3,2 mm em X=5/Y=5 mm:

```powershell
protocraft-cli run placa.protocraft --cmd add_hole --params '{"hole":{"id":1,"position":{"x":5000000,"y":5000000},"drill_nm":3200000}}' --output placa-com-furo.protocraft
protocraft-cli pcb-check placa-com-furo.protocraft
```

`update_track` recebe `{track:{id,start,end,width_nm,layer,net}}`; `update_via` recebe `{via:{id,position,diameter_nm,drill_nm,net}}`; `update_hole` recebe o objeto `hole` completo. Atualizações exigem um ID já existente. A CLI valida o domínio, e `pcb-check` confere a geometria; a fabricação exige DRC aprovado. O desktop também usa o DRC antes de confirmar cada edição de cobre/furação.
