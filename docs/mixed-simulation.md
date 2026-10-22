# Simulação mista e componentes complexos — 0.4

O motor **Misto** (`mixed`) sincroniza o solver analógico original com estados digitais e atrasos de propagação. A biblioteca tem **36 tipos selecionáveis** e **20 exemplos próprios**. Não depende de DLLs, instalação ou licença de outro simulador.

## Usar na interface

1. Abra **Exemplos → 555 astável**, **Flip-flop D** ou **Contador de década**.
2. Ao carregar um circuito que exige estados digitais, o aplicativo seleciona Misto. Também é possível escolhê-lo em **Configurações → Motor e visualização**.
3. Clique em **▶ Contínua**, ou execute uma análise **Transiente**. O 555 astável não tem estado DC estável: use transiente.
4. Selecione um componente para editar atraso, estado inicial, resistência de saída ou parâmetros analógicos. Clique em **Aplicar propriedades**. Reinicie a sessão para aplicar outro estado inicial.
5. No exemplo do flip-flop, dois cliques na entrada `CLRn` ativam/desativam o clear. Pausar e avançar preserva os estados e eventos pendentes.

Nomes de redes iguais conectam os pinos dos exemplos, mesmo sem fios desenhados. Os componentes alimentados exigem VCC e GND ligados explicitamente. Abaixo de 1 V entre alimentação e referência, as saídas ficam ligadas à referência pela resistência OFF; não há níveis altos gerados por alimentação implícita nesses novos modelos.

```powershell
protocraft.exe examples/555-astavel.protocraft --engine mixed --continuous
protocraft-cli simulate examples/555-astavel.protocraft --engine mixed --transient 0.00001 0.01 --csv timer.csv
protocraft-cli simulate examples/contador-decada.protocraft --engine mixed --transient 0.0001 0.02
```

A CLI continua usando `stable` por padrão. Especifique `--engine mixed` para componentes com estado. Uma escolha incompatível retorna erro; os componentes nunca são omitidos.

## Modelos novos

| Tipo | Comportamento |
|---|---|
| NMOS / PMOS | Canal longo com corte, triodo e saturação; modulação de canal, canal reverso, diodo do corpo ligado à fonte e fugas. |
| Comparador | Saída retida dentro da faixa de histerese diferencial, alimentação explícita e resistência de saída. |
| Schmitt inversor | Comuta em 1/3 e 2/3 da alimentação; retém estado entre limiares. |
| Temporizador 555 | Latch, reset prioritário, controle de limiares, divisor resistivo e transistor de descarga equivalente. Exemplos astável e monoestável. |
| Flip-flop D | Captura D na subida de CLK; Q/Qn, preset e clear assíncronos ativos em zero. PREn e CLRn simultaneamente ativos retornam erro. |
| Contador de 4 bits | Conta módulo 16 na subida de CLK; reset e habilitação ativos em nível alto. |
| Contador de década | Conta módulo 10; uma das dez saídas fica alta. Reset e habilitação ativos em nível alto. |
| VCVS | Fonte ideal de tensão controlada: Vout = ganho × (Vctrl+ − Vctrl−). |
| VCCS | Fonte ideal de corrente controlada: corrente de OUT+ para OUT− = gm × (Vctrl+ − Vctrl−). |

Estável e Dinâmico também aceitam MOSFET e fontes controladas. Os seis tipos alimentados com estado exigem Misto. Portas combinacionais anteriores continuam disponíveis nos três motores; no Misto ganham atraso inercial configurável. Operacional e portas anteriores conservam alimentação implícita e suas limitações.

### Ordem dos pinos

Numeração do ProtoCraft, começando em 1. Somente o temporizador segue aqui a ordem funcional usual do 555 de oito pinos; os demais são símbolos genéricos, sem associação automática a encapsulamentos comerciais.

| Componente | Pinos em ordem |
|---|---|
| NMOS / PMOS | G, D, S; corpo internamente ligado a S |
| Comparador | IN+, IN−, OUT, VCC, GND |
| Schmitt | IN, OUT, VCC, GND |
| 555 | GND, TRIG, OUT, RSTn, CTRL, THR, DIS, VCC |
| Flip-flop D | D, CLK, PREn, CLRn, Q, Qn, VCC, GND |
| Contador binário | CLK, RST, EN, VCC, GND, Q0, Q1, Q2, Q3 |
| Contador de década | CLK, RST, EN, VCC, GND, Q0 até Q9 |
| VCVS / VCCS | CTRL+, CTRL−, OUT+, OUT− |

No 555, CTRL é ligado internamente a VCC por 5 kΩ e a GND por 10 kΩ. TRIG abaixo de CTRL/2 arma o latch; THR acima de CTRL o desarma quando TRIG não está ativo. RSTn abaixo de 0,7 V domina ambos. DIS descarrega quando a saída está baixa. O atraso padrão de 100 ns é configurável e representa uma escolha do modelo genérico, sem caracterização de fabricante.

### Parâmetros e resultados

| Campo persistido | Padrão | Uso |
|---|---|---|
| `value` no MOSFET | 0,02 A/V² | Parâmetro k de condução, equivalente a KP com W/L=1. |
| `mos_threshold_v` | 2 V | Módulo do limiar MOS; a polaridade é determinada pelo tipo. |
| `channel_modulation` | 0,01 V⁻¹ | λ do MOSFET. |
| `hysteresis_v` | 0,05 V | Largura total da faixa diferencial do comparador. |
| `propagation_delay_s` | 100 ns | Atraso inercial, mínimo de 1 ps. |
| `initial_state` | 0 | Estado ao iniciar: 0/1 para latch/portas; 0–15 ou 0–9 para contadores. |
| `output_resistance` | 100 Ω | Estágio de saída finito dos modelos alimentados. |

`set_model` substitui todo o conjunto de parâmetros; os omitidos recebem padrões. O formato atual `.protocraft` v5 migra versões 1 a 4; a v4 introduziu os modelos mistos. A leitura JSON preserva a representação `f64` após salvar/reabrir, incluindo valores muito pequenos de Is.

Os resultados incluem `pin_voltages`, `pin_currents` e `digital_state` nos modelos novos. Correntes de terminal são positivas entrando no componente. Para MOSFET, U=Vds e I=Id; para chips, U=VCC−GND e I entra em VCC. P soma V×I em todos os terminais. Saídas de chips retiram corrente das suas redes de alimentação; os testes verificam KCL. Contadores mostram a contagem no corpo do símbolo.

## Sincronização e limites

O Misto usa backward Euler. Divide o passo solicitado nas bordas conhecidas das fontes quadradas, eventos de propagação e cruzamentos detectados dos limiares de comparador, Schmitt e 555. O refinamento temporal usa bisseção, sem modificar estado nas tentativas. Saídas simultâneas são atualizadas deterministicamente; uma reavaliação elétrica ocorre após o evento. As curvas podem conter amostras separadas por 1 ps para mostrar ambos os lados de uma transição.

Isso não é controle adaptativo do erro de integração. Use um passo pequeno para ondas suaves, RC e clocks gerados por circuitos analógicos: cruzamentos que acontecem e retornam dentro de um mesmo passo podem não ser detectados. As entradas de clock dos dispositivos sequenciais usam metade da alimentação como limiar; não têm setup/hold nem metastabilidade. O 555 usa um estágio equivalente resistivo, sem estrutura interna transistor a transistor.

O limite global permanece em 64 incógnitas, 256 componentes, 20.000 passos solicitados e dois milhões de amostras por análise, incluindo os eventos. Há limite adicional de 100.000 passos internos por avanço e orçamento de 150 milhões de operações estimadas, cobrado em cada iteração de Newton. Cancelamento e timeout continuam ativos. DC admite até 256 ciclos de estabilização digital; osciladores sem ponto fixo retornam erro e devem usar transiente. Não há adaptação automática do tamanho de circuito.

Alterações de topologia reiniciam a sessão. Alterações de parâmetros preservam estados elétricos e saídas atuais, cancelam transições ainda pendentes e reavaliam as entradas; editar `initial_state` só afeta uma nova sessão. Os gráficos contínuos guardam 512 amostras por sinal, incluindo eventos.

Exportação SPICE suporta MOS Level 1 com corpo ligado à fonte e fontes E/G. Chips comportamentais continuam recusados explicitamente. MCU/firmware, memórias, ADC/DAC, JFET, caracterização térmica, parasitas e modelos comerciais ainda não estão implementados.

## Referências funcionais públicas

O estudo usou a descrição pública de [simulação mista da Labcenter](https://www.labcenter.com/simulation/) para entender o fluxo de bancada, sem incorporar seu motor. O comportamento do temporizador foi conferido no [datasheet TI NE555](https://www.ti.com/lit/ds/symlink/ne555.pdf); a captura e os controles assíncronos no [datasheet TI SN74HC74](https://www.ti.com/lit/ds/symlink/sn74hc74.pdf). O modelo MOS e sua exportação usam equações públicas de canal longo, com referência ao [manual oficial ngspice](https://ngspice.sourceforge.io/docs/ngspice-46-manual.pdf). São modelos próprios e genéricos; não representam validação de dispositivos específicos desses fabricantes.
