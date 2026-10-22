# Motores e bancada de simulação — versão 0.5

Em **Configurações → Motor e visualização**, escolha o motor. A troca encerra a execução atual e limpa seus resultados; inicie outra análise para usar a nova escolha. **Salvar configurações** persiste a preferência em `%APPDATA%/ProtoCraft/settings.json` no Windows. Os exemplos ficam no menu **Exemplos** e na pasta `examples/`.

| Motor próprio | Método de transiente | Uso e limite |
|---|---|---|
| Estável (`stable`) | Backward Euler | Amortece oscilações numericamente; útil para mudanças abruptas. |
| Dinâmico (`dynamic`) | Trapezoidal | Menor amortecimento em RLC; pode produzir oscilações numéricas em descontinuidades. |
| Misto (`mixed`) | Backward Euler e eventos | Estados digitais, atraso inercial e componentes alimentados. Consulte [pinagens e exemplos](mixed-simulation.md). |

Os três são implementações próprias em Rust que compartilham MNA e a netlist. Estável/Dinâmico diferem na discretização de C/L. Misto acrescenta agenda de eventos e modelos com estado. Em DC analógico resolvem as mesmas equações. Nenhum executável externo é necessário. Não são motores licenciados de terceiros nem uma reprodução completa de outro simulador.

O Dinâmico usa backward Euler no primeiro passo e após uma alteração de parâmetros, para reinicializar o histórico de integração. Fontes começam em condições iniciais nulas. Não há controle adaptativo de erro nem análise AC de pequenos sinais.

## Execução contínua

Clique em **▶ Contínua**. **Pausar**, **Continuar**, **Um passo** e **Parar** controlam a sessão. Ela mantém tensão nos capacitores e corrente nos indutores entre avanços. É uma simulação em tempo virtual; a velocidade de parede depende da complexidade e dos passos por atualização.

- Dois cliques em uma chave ou entrada lógica alternam seu estado por um comando reversível.
- Selecione um componente para editar parâmetros e clique em **Aplicar propriedades**.
- Alterações de valores preservam o estado elétrico. Mudanças de pinos, componentes, fios, junções ou nomes de redes reiniciam o estado e o tempo.
- Fios ficam verdes para tensão positiva, vermelhos para negativa e cinza perto de zero. A escala é configurável.
- Pontos nas ligações de componentes de dois terminais indicam corrente convencional. Não são uma estimativa de corrente em cada segmento de fio.
- LED tem brilho proporcional à corrente direta. O tooltip e o inspetor mostram o último estado elétrico, associado à revisão do esquema.

A janela de gráficos contínuos mantém as últimas **512 amostras por sinal**. Exportar CSV durante a execução exporta o snapshot disponível nessa janela, sem histórico ilimitado. Parar preserva o último resultado; a próxima execução começa do zero.

## Componentes e parâmetros

São **36 tipos disponíveis na biblioteca**, incluindo GND e conector. `Imported` é um tipo adicional para preservar dados de importação, sem modelo de simulação.

| Grupo | Componentes | Parâmetros e convenções |
|---|---|---|
| Passivos | R, C, L | Valores em Ω, F e H; condições iniciais zero. |
| Fontes | Tensão/corrente DC; tensão senoidal, quadrada/pulso, triangular e dente de serra | `value` é amplitude de pico das ondas; `frequency_hz`, `offset_v`, `phase_deg`; quadrada usa `duty_cycle`. Em DC, ondas usam o offset. |
| Interativos | Chave SPST, potenciômetro, entrada lógica | `closed` alterna chave/nível lógico. Chave tem resistências finitas ON/OFF; cursor do potenciômetro usa `wiper` entre 0 e 1. |
| Semicondutores | Diodo, LED, Zener, NPN e PNP | Modelos genéricos; sem parasitas nem dependência térmica. |
| Operacional | Amplificador operacional genérico | Ganho aberto, limites de tensão e resistência de saída; alimentação implícita. |
| Lógica | NOT, AND, OR, XOR, NAND e NOR | Entradas comparadas ao limiar; saída alta `value` e baixa 0 V, com resistência de saída. Atraso inercial no Misto; resposta instantânea nos outros motores. |

Na chave, os padrões são 0,01 Ω fechada e 1 GΩ aberta. No potenciômetro, `R(A,cursor)=R*wiper` e `R(cursor,B)=R*(1-wiper)`, com mínimo de 1 µΩ por segmento. Isso evita singularidades nos extremos e modela uma resistência residual.

Diodo e LED usam Shockley: `I=Is*(exp(V/(n*Vt))-1)`, com `Vt=25,85 mV`; `value` representa Is. O LED padrão usa Is=1e-30 A para uma curva educativa de maior queda direta. O Zener usa `value` como tensão de ruptura e acrescenta uma exponencial reversa; sua tensão depende da corrente, não é um clamp ideal.

NPN/PNP usam Ebers–Moll de baixa frequência, `value=beta`, corrente de saturação configurável e beta reverso fixo em 1. Ordem de pinos: **base, coletor, emissor**. O operacional usa entradas **+, −, saída**, ganho limitado pelas tensões configuradas e resistência de saída padrão de 100 Ω. Portas de duas entradas usam **A, B, saída**; NOT usa **entrada, saída**. Entradas de operacionais/portas têm fuga de 1 TΩ para GND. Alimentação e consumo internos não são modelados.

`U(ref)`, `I(ref)` e `P(ref)` aparecem nos resultados e CSV. Nos componentes de dois terminais, tensão e corrente seguem pino 1 → pino 2. Para BJT, U é Vce e I é a corrente que entra no coletor; P considera os três terminais. Para operacional/porta, U é a tensão da saída ao GND e I entra pela saída; potência negativa representa energia fornecida pela alimentação implícita. No potenciômetro, I entra por A e P soma a dissipação dos dois segmentos. `operating_points.pin_voltages` preserva as tensões de todos os pinos.

## CLI e automação

```powershell
protocraft-cli engines
protocraft-cli components
protocraft-cli simulate examples/rc.protocraft --engine dynamic --transient 0.00001 0.005 --csv rc.csv
protocraft-cli run examples/rc-senoidal.protocraft --cmd set_model --params '{"id":1,"parameters":{"frequency_hz":250,"offset_v":1,"phase_deg":45}}' --output onda.protocraft
protocraft.exe examples/led-chave.protocraft --engine dynamic --continuous
protocraft.exe --settings
```

`engines` e `components` retornam JSON. Parâmetros omitidos de `set_model` recebem os padrões; o comando substitui o conjunto inteiro. Valores inválidos rejeitam o lote completo. O arquivo `.protocraft` v5 persiste `Component.parameters`; preferências do usuário ficam fora do projeto.

## SPICE e limites

Exportação SPICE abrange RLC, fontes DC/seno/quadrada sem fase, chave em estado fixo, potenciômetro, diodo/LED/Zener, BJT, MOS Level 1 e fontes controladas E/G. Triângulo, serra, quadrada com fase, portas, operacional e chips com estado retornam erro explícito de exportação. Não há omissão silenciosa de elementos. A exportação usa modelos genéricos e não promete equivalência numérica irrestrita entre simuladores.

O solver tem limites de 64 incógnitas, 256 componentes com modelo, 20.000 passos por análise e dois milhões de amostras por lote. A sessão contínua retém 512 amostras e avança em blocos limitados. Newton admite 100 iterações com limitação das mudanças de tensão nas junções. Cancelamento e timeout também se aplicam à sessão contínua. Entradas com importação eletricamente incompleta continuam bloqueadas.

No Estável/Dinâmico, fontes quadradas são amostradas no passo fixo: pulsos menores que o passo podem desaparecer. Misto para nas bordas conhecidas e nos eventos pendentes; refina cruzamentos detectados em comparador, Schmitt e 555. Há aviso quando o passo supera 1/20 do período de uma fonte. Isso não substitui controle adaptativo de erro: ondas suaves exigem passo adequado. O [guia Misto](mixed-simulation.md) detalha os novos componentes, limites e exemplos. MCU, memórias, ADC/DAC, JFET, transformadores, linha de transmissão, modelos comerciais e ruído permanecem fora desta versão.

## Estudo funcional e implementação independente

Foram consultadas as [instruções oficiais do CircuitJS](https://www.falstad.com/circuit/directions.html) e a [interface JavaScript documentada](https://www.falstad.com/circuit/jsinterface.html). A referência funcional inclui circuitos interativos, fontes configuráveis, leitura de estados elétricos e gráficos. A implementação Rust deste projeto usa equações públicas, modelos tipados e exemplos próprios. Nenhum código, asset, circuito de exemplo ou runtime do CircuitJS foi incorporado.

As próximas ampliações exigem modelos e referências próprias: passo adaptativo com estimativa de erro; ampliação dos eventos digitais; condições iniciais editáveis; sondas diferenciais e vários gráficos; dispositivos de potência; instrumentos e co-simulação de firmware. O documento registra o que foi entregue, sem atribuir compatibilidade integral aos simuladores estudados.

O catálogo físico STM32/ESP32 e a PCB estão no [guia MCU/PCB](mcu-pcb.md). Esses dispositivos não acrescentam emulação de firmware aos motores acima.
