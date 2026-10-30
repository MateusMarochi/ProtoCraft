# Placa 555 com LED perto de 1 Hz

O exemplo completo está em [`examples/555-led-1hz-pcb.protocraft`](../examples/555-led-1hz-pcb.protocraft). Ele contém esquemático, footprints passantes, posicionamento, placa de duas camadas, trilhas roteadas e uma via. O formato aberto do projeto continua sendo `.protocraft`.

## Circuito e lista de materiais

U1 usa um temporizador 555 genérico de 8 pinos, com pinagem compatível com um NE555 em DIP-8. A frequência aproximada em astável é `f ≈ 1,44 / ((RA + 2 RB) C1)`. Os valores escolhidos usam resistores comerciais e resultam em aproximadamente 1 Hz no modelo do ProtoCraft.

As conexões do esquemático são representadas por etiquetas de rede (23 labels e nenhum segmento de fio), como no template 555 existente. O mapa de pinos e as trilhas da PCB mantêm esses nets.

| Referência | Valor | Encapsulamento / observação |
| --- | --- | --- |
| J1 | Entrada de 5 V | Bloco terminal THT de 2 posições, passo 5,08 mm; pino 1 = +5 V, pino 2 = GND |
| U1 | Temporizador 555 | DIP-8, soquete opcional |
| RA | 47 kΩ | Resistor axial THT, passo 7,62 mm |
| RB | 48,7 kΩ | Resistor axial THT, passo 7,62 mm |
| C1 | 10 µF | Eletrolítico radial THT, passo 2,50 mm; pino 1 positivo |
| C2 | 10 nF | Cerâmico radial THT, passo 2,50 mm, entre CTRL e GND |
| D1 | LED de 5 mm | THT; pad redondo 1/ânodo no pino 3 do 555, pad quadrado 2/cátodo |
| RLED | 680 Ω | Resistor axial THT em série com o LED |

Com C1=10 µF, o motor Misto simulou período de 1,00234 s (0,997665 Hz) após a partida e corrente de pico do LED de 6,41 mA. São modelos genéricos; tolerância do capacitor, variação do temporizador, alimentação e temperatura alteram a frequência e a corrente no circuito físico.

## PCB e fabricação

- Placa retangular de 65 × 45 mm, espessura de 1,6 mm.
- Duas camadas de cobre, largura de trilha de 0,35 mm, 45 segmentos de trilha e uma via; as conexões usam trechos ortogonais e a 45° para manter o desenho uniforme.
- As trilhas apontam ao centro dos pads passantes e param 0,05 mm além da borda de cada furo, sem cobrir a abertura. As junções de segmentos têm extremidades arredondadas na visualização.
- Os componentes foram agrupados por função: entrada à esquerda, 555 ao centro, rede de temporização à direita e LED/resistor próximos à saída.
- Pads THT com furação metalizada; o ZIP contém arquivos de furação PTH e NPTH separados.
- O preset JLCPCB gera Gerber RS-274X com extensões Protel, em milímetros e escala 1:1, com origem de furação absoluta.

O roteador nativo concluiu as 15 conexões sem falhas, com 174,29 mm de cobre total. O DRC do ProtoCraft terminou com zero diagnósticos e zero airwires. O KiCad CLI 10.0.7 reportou zero erros e zero conexões abertas. A verificação completa lista oito avisos sobre bibliotecas de footprints não registradas na instalação portátil do KiCad; não há violações de cobre ou itens sem conexão.

Arquivos gerados nesta validação local:

- `artifacts/555-1hz/BOM.csv` — lista de materiais em CSV delimitado por ponto e vírgula.
- `artifacts/555-1hz/fabricacao-terminais-radiais/JLCPCB.zip` — pacote Gerber/Excellon com aproximações radiais e furação desobstruída.
- `artifacts/555-1hz/fabricacao-terminais-radiais/projeto.kicad_pcb` — placa temporária exportada para validação externa.
- `artifacts/555-1hz/fabricacao-terminais-radiais/drc-kicad.json` — DRC do KiCad com severidade error.
- `artifacts/555-1hz/fabricacao-terminais-radiais/drc-all.json` — DRC completo; os oito avisos são somente bibliotecas locais não registradas.
- `artifacts/555-1hz/fabricacao-terminais-radiais/drc-protocraft.json` — resultado do DRC nativo.
- `artifacts/555-1hz/simulation-final.csv` — simulação transiente de 4 s.
- `artifacts/555-1hz/board-rerouted.svg` e `board-rerouted-preview.png` — vista vetorial e prévia ampliada da placa.

`artifacts/` é ignorado pelo Git; o projeto `.protocraft` e este guia ficam em `examples/` e `docs/` para que o agente possa reutilizá-los.

## Repetir com a CLI

Os comandos partem da raiz do repositório. O caminho de `kicad-cli.exe` deve apontar para uma instalação do KiCad; escolha uma pasta de saída nova para cada fabricação.

```powershell
protocraft-cli validate examples/555-led-1hz-pcb.protocraft
protocraft-cli pcb-check examples/555-led-1hz-pcb.protocraft
protocraft-cli simulate examples/555-led-1hz-pcb.protocraft --engine mixed --transient 0.002 4 --csv artifacts/555-1hz/simulation.csv
protocraft-cli pcb-manufacture examples/555-led-1hz-pcb.protocraft --preset jlcpcb --output artifacts/555-1hz/nova-fabricacao --kicad "C:\Program Files\KiCad\10.0\bin\kicad-cli.exe"
```

O DRC de severidade `error` é obrigatório para a fabricação. A verificação completa do KiCad pode continuar exibindo avisos de biblioteca quando a instalação não tem as bibliotecas de footprints cadastradas; isso não altera os Gerbers exportados a partir das geometrias incorporadas no projeto.

Não houve fabricação nem teste físico desta placa.
