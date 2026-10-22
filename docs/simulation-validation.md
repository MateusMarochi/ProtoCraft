# Validação da simulação

Os motores `ProtoCraft Estável`, `ProtoCraft Dinâmico` e `ProtoCraft Misto` são implementações originais em Rust, sem dependência do Proteus, ngspice ou processos externos. Usam análise nodal modificada, eliminação gaussiana com pivoteamento parcial e normalização de linhas. Operam com `f64` em SI.

Suporte e parâmetros estão no [guia dos motores](simulation-engines.md): RLC, fontes DC e de formas de onda, chave, potenciômetro, diodo/LED/Zener, NPN/PNP, operacional, entrada lógica e portas combinacionais. Os modelos são genéricos e educativos. Newton limita mudanças de tensão nas junções a 150 mV por iteração, com máximo de 100 iterações. Expoentes são limitados a [-80,80] para diodos e [-60,60] para BJT.

Em DC, capacitores são abertos e indutores são curtos ideais. O Estável usa backward Euler e o Dinâmico usa trapezoidal, com backward Euler no primeiro passo e após editar parâmetros. Estável/Dinâmico usam passo fixo. Misto insere subdivisões de eventos/limiares; o último passo do lote pode ser reduzido para terminar no instante solicitado. Capacitores começam com tensão zero, indutores com corrente zero e fontes são aplicadas em t=0+. A amostra t=0 precede a energização; não é solução estática das fontes. A sessão contínua preserva estado entre blocos, mantém 512 amostras e reinicia quando a topologia muda. Não há controle adaptativo de erro.


## Casos automatizados

Os testes estão em `crates/eda-simulation/tests/analytic.rs`, `expanded_models.rs`, `mixed_models.rs` e `import_safety.rs`. A tabela inicial registra a referência de regressão do motor Estável.

| Caso | Referência | Critério |
| --- | --- | --- |
| Divisor, 10 V, R1=R2=1 kΩ | saída 5 V | erro <1 nV |
| Lei de Ohm no mesmo divisor | corrente fornecida 5 mA | erro <1 pA; sinal negativo na fonte indica energia fornecida |
| RC, 1 kΩ e 1 µF, degrau de 10 V | 10(1-exp(-t/1 ms)) | passo 10 µs, duração 5 ms, erro absoluto <20 mV em toda curva |
| RL, 1 kΩ e 1 H | tensão no indutor 10exp(-t/1 ms) | erro <20 mV após t=0; indutor é curto em DC |
| Diodo, Is=1 pA, fonte 10 V e R=1 kΩ | corrente resistiva igual à equação Shockley | resíduo de corrente <1 nA e tensão entre 0,5 e 0,7 V |
| Falhas | ausência de GND, circuito singular, NaN e excesso de passos | erro retornado, sem resultado numérico silencioso |
| SPICE | somente identificadores numéricos gerados | nomes externos não entram em diretivas |
| Controle | cancelamento prévio através de clone e timeout de duração zero | erros tipados `Cancelled` e `Timeout`, via API pública e backend dinâmico |

## Limites e segurança

Máximo de 64 incógnitas MNA (nós sem GND mais ramos de fontes de tensão e indutores), 256 componentes com modelo, dois milhões de amostras por análise e 20.000 passos e orçamento de 150 milhões de operações estimadas; o fator de iterações de diodos entra nesse orçamento. Estável/Dinâmico mantêm a estimativa conservadora inicial. Misto cobra o custo estimado de cada iteração efetivamente executada, incluindo refinamento e eventos. `SimulationControl` oferece cancelamento compartilhado entre clones e timeout de parede configurável (10 s por padrão), verificado antes/depois da preparação, em cada passo e a cada iteração não linear. Não é uma interrupção imediata: validação/netlist e uma fatoração em andamento terminam antes da próxima checagem. A GUI deve usar thread de trabalho e associar resultados à revisão original. `simulate` usa controle padrão; `simulate_controlled` recebe o controle externo. O trait `SimulationBackend` permite substituir o backend; `MnaBackend` preserva o padrão Estável. `simulate_with_engine` seleciona a integração e `TransientSession` conserva estado entre avanços.

Valores de componentes são finitos, positivos nos tipos que exigem isso e limitados a magnitude de 1e12 no solver. Parâmetros têm intervalos validados no núcleo. Passo mínimo de 1 ps, duração máxima de 1 milhão de segundos e magnitude máxima de incógnitas de 1e15. Pivôs normalizados abaixo de 1e-14 são rejeitados. Não há temperatura variável, parasitas, ruído, setup/hold, metastabilidade, firmware, modelos comerciais, AC de pequenos sinais nem compatibilidade SPICE completa.


`export_spice` gera texto `.op` ou `.tran ... UIC`, elementos e modelos genéricos definidos pelo projeto; modelos sem equivalente retornam erro explícito de exportação. Não executa o arquivo, não insere nomes do usuário, `.include`, scripts ou comandos externos. A referência dos testes automatizados é analítica. A comparação histórica externa não valida automaticamente os modelos acrescentados na versão 0.3. Uma comparação externa efetivamente executada com ngspice 47 para divisor, RC e diodo está documentada em [ngspice-validation.md](ngspice-validation.md), com tolerâncias e diferenças de temperatura explícitas.

## Execução

Na raiz: `cargo test -p eda-simulation`. O estado final de compilação e execução dos testes é registrado no relatório de integração; esta documentação descreve os critérios efetivamente codificados, sem alegar execução antes da integração.



## Critérios novos da versão 0.3

| Caso | Critério |
|---|---|
| RC Dinâmico, 1 kΩ/1 µF, 10 V, passo 10 µs | Em 1 ms, erro <1 mV e menor que 1/10 do erro Estável. |
| RLC, R=10 Ω, L=1 mH, C=1 µF, degrau de 1 V | Solução analítica amortecida, passo 1 µs, t=100 µs: erro Dinâmico <2 mV e menor que no Estável. |
| Fontes | Seno/quadrada/triângulo/serra, frequência, offset e pulso: erro <1e-10 V nos pontos verificados; DC usa offset. |
| Potenciômetro | Cursor 25%, fonte 10 V, sem carga: 7,5 V com erro <1 nV. |
| Chave | Divisor finito ON/OFF: saída fechada a <1 mV de 10 V, aberta <20 µV. |
| LED/Zener | KCL <10 nA e soma das potências <100 nW, condução direta/reversa. |
| NPN/PNP | Beta=100 em região ativa: erro de beta <0,01; KCL de coletor <1 nA. |
| Operacional | Seguidor <0,1 mV; saturação considera a resistência de saída de 100 Ω. |
| Portas | Todas as tabelas verdade de NOT/AND/OR/XOR/NAND/NOR. |
| Contínua | Preserva a tensão RC ao mudar fonte, limita histórico a 512 amostras; topologia nova reinicia tempo. |
| Onze exemplos | Projeto válido, ERC sem erros e DC/transiente finitos nos dois motores. |
| SPICE | Todo novo modelo exportável aparece; modelos não equivalentes são recusados. |

Testes adicionais exercitam o worker com pausa, avanço, revisão e cancelamento; o canvas com eventos de duplo clique e undo; a CLI como subprocesso selecionando motores e persistindo parâmetros. O registro de integração distingue esses testes de uma validação física ou de modelos comerciais.

## Critérios novos da versão 0.4

| Caso | Referência / critério |
|---|---|
| NMOS/PMOS | Corte, triodo e saturação analíticos nos três motores: erro de Id <10 nA; KCL <1 pA. |
| VCVS/VCCS | Ganho e transcondutância, sinal e KCL <1 pA. |
| Comparador | Retém estado dentro da histerese; corrente da carga retirada da alimentação, KCL <1 pA. |
| Schmitt | Cruzamento triangular a 1/3 ms mais 100 ns: erro temporal <2 ns. |
| Flip-flop D | Divisão por dois com passo maior que período; atraso de 100 ns resolvido com erro <20 ps. |
| Contadores | Retorno módulo 16/10, contagem com passo de 4 ms e clock de 1 kHz; década tem uma saída alta. |
| 555 astável | Período ln(2)×(RA+2RB)×C: erro <1,5%, RA=RB=10 kΩ, C=1 µF, passo 200 µs. |
| 555 monoestável | Pulso ln(3)×RC: erro <1,5%, R=10 kΩ, C=100 nF, passo 10 µs. Reset domina trigger/threshold e ativa descarga. |
| Estados | Preset/clear assíncronos; saídas sem alimentação; eventos preservados entre blocos; atraso inercial filtra pulso de 10 µs com atraso de 100 µs. |
| Integração | 20 exemplos válidos e ERC sem erros; análises nos motores compatíveis. Oscilador 555 usa somente transiente. |
| Persistência/CLI/SPICE | Migração v3→v4 e round-trip dos parâmetros; CLI real simula 555 em Misto e recusa Estável; exportação MOS/E/G e recusa explícita dos chips. |

Os resultados independentes dos novos modelos analógicos são registrados em [ngspice-validation.md](ngspice-validation.md). As tolerâncias dos testes não caracterizam dispositivos comerciais nem substituem validação física.
