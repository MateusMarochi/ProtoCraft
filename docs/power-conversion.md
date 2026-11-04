# Conversão chaveada e protótipo buck para Raspberry Pi 4

Este documento registra o primeiro fluxo de projeto de potência do ProtoCraft. O objetivo é fornecer ao CLI e a agentes de projeto um cálculo repetível, uma referência de componentes e uma placa nativa editável. Os números calculados são estimativas de primeira ordem; não são uma simulação SPICE chaveada nem uma validação de hardware.

## Como gerar e inspecionar

```powershell
ProtoCraft-CLI.cmd power-design buck
ProtoCraft-CLI.cmd power-design buck --vin-v 12 --vout-v 5.1 --iout-a 3 --max-current-a 6 --switching-hz 400000 --ripple-fraction 0.25 --efficiency 0.92 --inductance-uh 4.7 --feedback-top-ohms 100000
ProtoCraft-CLI.cmd buck-pi4-12v --output projetos\pi4-buck.protocraft
ProtoCraft-CLI.cmd pcb-info projetos\pi4-buck.protocraft
ProtoCraft-CLI.cmd pcb-check projetos\pi4-buck.protocraft
```

`power-design buck` imprime JSON com os parâmetros e as equações usadas. `buck-pi4-12v` monta o exemplo, aplica as classes de rede, executa o autorroteador nativo e só grava o arquivo se não houver conexões pendentes nem erros de DRC. O caminho de saída precisa ser novo. A aplicação desktop também disponibiliza `pcb-pi4-12v-buck` na galeria nativa.

## Especificação da fixture

| Parâmetro | Valor do exemplo |
|---|---:|
| Entrada nominal | 12 V DC |
| Saída nominal | 5,115 V estimados com o divisor E96 selecionado (alvo 5,1 V) |
| Corrente de saída do exemplo | 3 A |
| Controlador | LM61460, buck síncrono, nominal até 6 A |
| Frequência configurada | 400 kHz |
| Indutor | 4,7 µH |
| Placa | 65 × 56,5 mm, raio de canto de 3 mm, quatro furos de 2,75 mm |
| Largura nominal mínima das classes `INPUT_12V`, `OUTPUT_3A` e `SWITCH_NODE` | 0,8 mm |
| Escape local nos pads QFN das classes largas | 0,25 mm, no máximo 2 mm, com via de 0,4/0,2 mm |
| Folga geométrica configurada | 0,1 mm |

O LM61460 tem faixa de entrada de 3 a 36 V, saída ajustável de 1 V a 95% de VIN e frequência configurável de 200 kHz a 2,2 MHz. O cálculo da fixture recusa entradas, frequências e correntes fora dos limites usados pelo LM61460, para que o JSON não descreva inadvertidamente um projeto fora da faixa desse componente. Isso não garante a capacidade térmica do circuito em qualquer combinação de entrada, corrente e ambiente.

## Equações e resultado estimado

O cálculo ideal usa:

```text
D       = VOUT / VIN
POUT    = VOUT × IOUT
IIN     ≈ POUT / (VIN × eficiência assumida)
perdas  ≈ POUT / eficiência assumida − POUT
ΔIL     = (VIN − VOUT) × D / (FSW × L)
Lmín    = (VIN − VOUT) × D / (FSW × ΔIL alvo)
RFBB    = RFBT × VREF / (VOUT − VREF)
```

Com 12 V, alvo de 5,1 V, 3 A, hipótese de eficiência de 92%, limite de 6 A, ondulação-alvo de 25% do limite, 400 kHz e 4,7 µH, o CLI calcula aproximadamente:

- duty cycle ideal: 42,5%;
- saída: 15,3 W;
- corrente de entrada estimada: 1,386 A;
- perdas estimadas: 1,33 W;
- ondulação estimada no indutor: 1,56 A pico a pico;
- pico estimado do indutor a 3 A: 3,78 A;
- indutância mínima pelo alvo de ripple: 4,89 µH;
- divisor 100 kΩ / 24,3 kΩ: saída nominal calculada de 5,115 V.

A estimativa não inclui tolerância do controlador e resistores, perdas do cobre e do indutor, capacitância cerâmica efetiva, ripple de comutação, transientes da carga, limitação por temperatura ou variação do adaptador de 12 V. O alvo calculado de indutância mínima é ligeiramente maior que 4,7 µH; o valor escolhido segue o valor padrão da aplicação de 5 V/400 kHz da TI e precisa ser confirmado com os requisitos de ripple e a curva de corrente real da placa.

## Componentes e redes

| Referência | Valor / seleção inicial | Função |
|---|---|---|
| U1 | LM61460AANRJRR | Conversor síncrono buck |
| L1 | 4,7 µH; conceito para Coilcraft XAL1010-472MED | Elemento de transferência de energia; selecionar com Isat acima do limite de pico do conversor e corrente RMS/térmica adequada |
| RFBT / RFBB | 100 kΩ / 24,3 kΩ | Divisor de feedback para aproximadamente 5,115 V |
| RT | 33,2 kΩ | Aproximadamente 400 kHz |
| CFF | 22 pF | Feed-forward conforme aplicação de 5 V/400 kHz |
| RBOOT | 0 Ω | Configuração indicada para eficiência; a TI permite resistor para controlar overshoot/EMI |
| CBOOT | 100 nF | Capacitor bootstrap entre CBOOT e SW |
| CVCC / CBIAS | 1 µF / 100 nF | Desacoplamentos locais do controlador |
| REN | 100 kΩ entre VIN e EN | Habilitação; revisar limiar e tolerâncias se adicionar UVLO |
| CIN1/CIN2 | 2 × 4,7 µF cerâmicos | Bypass de entrada, dividido perto dos pares VIN/PGND |
| CIN3/CIN4 | 2 × 100 nF cerâmicos | Alta frequência, um por par VIN/PGND |
| CBULK_IN | 47 µF eletrolítico conceitual | Reserva local para cabo/adaptador e transitórios de entrada |
| COUT1–COUT4 | 4 × 22 µF cerâmicos | Capacitores de saída |
| COUT_BULK | 560 µF eletrolítico, alvo ESR ≤ 100 mΩ | Melhor resposta a transientes de carga |

A referência de 5 V/400 kHz da TI lista 4,7 µH, RT de 33,2 kΩ, CFF de 22 pF, duas cerâmicas de 4,7 µF e duas de 100 nF na entrada, divisor de 100 kΩ/24,9 kΩ e quatro cerâmicas de 22 µF na saída. Para resposta de transiente melhor, a tabela de aplicação recomenda quatro cerâmicas de 22 µF e um eletrolítico de 560 µF com ESR de 100 mΩ. O divisor inferior foi recalculado para o alvo 5,1 V. Verifique tensão nominal, ESR, ripple, tolerância e capacitância efetiva sob polarização e temperatura para cada peça realmente comprada; o valor impresso no MLCC não é necessariamente o valor em circuito.

O indutor XAL1010-472MED é uma opção para a investigação de BOM: a Coilcraft publica Isat de 25,4 A e Irms de 17,5 A para ele, mas a corrente RMS permitida depende da elevação térmica e das condições de montagem. O footprint incluído é explicitamente conceitual; confirme a geometria de pads na revisão atual do datasheet do fabricante antes de fabricar.

### Conexões funcionais

- J1 recebe `VIN12` e `GND` de uma fonte de 12 V DC.
- VIN1/VIN2 recebem `VIN12`; PGND1/PGND2 e AGND estão ligados à rede comum `GND`.
- `SW` passa por L1 até `5V1`.
- O divisor `RFBT`/`RFBB` fecha `5V1` → FB → GND; CFF fica em paralelo com RFBT.
- BIAS acompanha `5V1`; os capacitores de bypass locais retornam a GND.
- Pinos físicos 2 e 4 do header J_PI recebem `5V1`; os pinos físicos de GND recebem `GND`.
- J_OUT é uma saída auxiliar 5,1 V/GND para medição ou carga compatível. Ela está no mesmo barramento do header e não é uma saída independente.

Os pinos 5 V do header podem ser entradas de alimentação da Raspberry Pi 4, mas o projeto não implementa negociação USB-C, seleção de fonte nem proteção contra retorno entre fontes. Desconecte o USB-C e qualquer outra fonte da Raspberry Pi antes de alimentar por esta placa. Não aplique 5,1 V nos pinos GPIO de sinal; a saída de 5 V se destina apenas aos pinos físicos de alimentação 2 e 4.

## Critério de trilhas e limites do DRC

O projeto define trilhas nominais de 0,8 mm para `INPUT_12V`, `OUTPUT_3A` (`5V1` e `GND`) e `SWITCH_NODE`; `CONTROL` usa 0,25 mm. Nos pads SMD do controlador, a saída estreita permitida tem 0,25 mm, termina no pad correto e não excede 2 mm. Quando necessário, ela recebe uma via de 0,4 mm com furo de 0,2 mm. Essa exceção é validada pelo DRC e só vale para o trecho ligado diretamente ao pad da mesma rede; ela não altera a regra mínima dos segmentos largos. A regra evita aplicar uma trilha de 0,8 mm diretamente a pads finos de QFN, mas não modela densidade de corrente no pad, capacidade térmica da via ou corrente compartilhada entre pinos.

O comando `power-design copper` oferece uma estimativa inicial combinando um ajuste histórico IPC-2221, resistência de cobre e limite de queda DC. Os parâmetros de cobre, comprimento, elevação térmica e margem são explícitos. Esse resultado não é dimensionamento certificado, não implementa IPC-2152 e não substitui cálculo térmico com stack-up e condições de montagem reais. Para a especificação padrão (3 A, cobre externo de 70 µm, 30 mm, queda máxima de 50 mV), a largura selecionada é arredondada para uma grade de 0,05 mm e a CLI informa largura mínima térmica, largura por queda e perda estimada.

Para avançar em uma placa de 3 A, especifique stack-up, espessura e peso de cobre externo (comece a revisão com cobre externo de 2 oz), calcule queda de tensão e elevação térmica, confira gargalos em pads e vias e distribua retorno de GND com caminho de baixa impedância. O ProtoCraft atual não modela ampacidade, espessura do cobre, planos de cobre ou temperatura no DRC. Um DRC sem erros demonstra apenas que as regras geométricas implementadas foram satisfeitas. A nota de layout do LM61460 recomenda uma stack-up de quatro camadas com plano de terra sob o CI; o protótipo atual é de duas camadas, portanto não reproduz essa recomendação e requer revisão do layout antes de qualquer fabricação.

O nó `SW` deve ser curto e compacto, os capacitores cerâmicos de entrada precisam ficar imediatamente junto aos pares VIN/PGND, e a realimentação deve ficar afastada de SW. O autorroteador deste exemplo distribui conexões e largura de classe, mas ainda não otimiza o loop de comutação, retorno de corrente, acoplamento magnético ou impedância de EMI. A placa gerada usa duas faces de cobre e não contém plano de terra.

## Itens que precisam ser fechados antes de fabricação

1. Revisar o footprint do VQFN-HR RJR0014A contra a página mecânica e o land pattern oficial, incluindo máscara, pasta, estêncil e dissipação. O símbolo/footprint no catálogo é uma geometria própria preliminar, não uma aprovação para montagem.
2. Conferir o footprint comprado do indutor e dos capacitores, altura, tensão nominal, ESR, ripple RMS, polaridade e corrente sob temperatura.
3. Definir proteção de entrada conforme a fonte e o ambiente: fusível, polaridade reversa, TVS e filtragem, se necessários. A fixture não inclui esses componentes.
4. Rever limite de entrada, rampa/UVLO, curto, partida, comportamento sem carga e queda nos conectores/cabos.
5. Revisar e aproximar o conjunto RBOOT/CBOOT do pino bootstrap e do nó SW, reduzir o loop de comutação conforme o datasheet, fazer cálculo térmico e de cobre, revisar EMI, medir estabilidade/transientes com carga eletrônica e ensaiar ruído conduzido/radiado. O layout gerado é geometricamente roteável, mas o autorroteador não otimiza esse loop crítico.
6. Validar o encaixe mecânico da placa com a revisão física da Pi, altura do header, orientação, gabinete e outros HATs. A base de 65 × 56,5 mm e furos segue o perfil mecânico HAT; isso não certifica conformidade HAT+.
7. Gerar arquivos de fabricação apenas após revisar o DRC e inspecionar Gerber/Excellon em uma ferramenta independente.

## Fontes técnicas

- Texas Instruments, [datasheet LM61460](https://www.ti.com/lit/ds/symlink/lm61460.pdf) e [página do produto](https://www.ti.com/product/LM61460).
- Raspberry Pi, [documentação de hardware](https://www.raspberrypi.com/documentation/computers/raspberry-pi.html) e [desenho mecânico PoE HAT](https://datasheets.raspberrypi.com/poe/poe-hat-mechanical-drawing.pdf).
- Coilcraft, [XAL1010-472MED](https://www.coilcraft.com/en-us/products/power/shielded-inductors/molded-inductor/xal/xal1010/xal1010-472/).
