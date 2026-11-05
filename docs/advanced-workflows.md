# Variantes de montagem, furação e análises de circuitos

Serviços headless compartilhados, disponíveis pela CLI. Não alteram o formato `.protocraft`; novos painéis desktop ainda não foram implementados.

## Furação e limites DFM

```powershell
protocraft-cli pcb-dfm --default-limits
protocraft-cli pcb-dfm examples/555-led-1hz-pcb.protocraft --limits limites.json --output dfm.json
```

`min_drill_clearance_nm` controla a distância mínima entre superfícies de duas furações. O padrão genérico é **250.000 nm = 0,25 mm**, também aplicado aos arquivos de limites antigos que não possuem esse campo. Ajuste-o ao processo contratado. O DRC nativo mantém as regras da placa.

Rasgos são cápsulas: eixo segmentado mais raio da ferramenta, respeitando rotação do pad/footprint e espelhamento da face inferior. Furos, cobre e borda usam essa geometria. Eixos rotacionados são arredondados ao nanômetro; a tolerância geométrica existente é de 1 nm. Há orçamento de dois milhões de comparações entre furos e outro para cobre/exclusões. Exceder mil diagnósticos de furação retorna erro explícito. A identificação dos dois furos ainda depende da mensagem; metadados estruturados completos de ambos os objetos seguem pendentes.

## Variantes de montagem

Use [555-without-led.json](../examples/assembly-variants/555-without-led.json):

```json
{"name":"555 sem indicador luminoso","do_not_populate":["D1","RLED"]}
```

```powershell
protocraft-cli pcb-assembly-variant examples/555-led-1hz-pcb.protocraft --config examples/assembly-variants/555-without-led.json
protocraft-cli pcb-assembly-variant examples/555-led-1hz-pcb.protocraft --config examples/assembly-variants/555-without-led.json --format bom-csv --output bom-sem-led.csv
protocraft-cli pcb-assembly-variant examples/555-led-1hz-pcb.protocraft --config examples/assembly-variants/555-without-led.json --format placements-csv --output posicoes-sem-led.csv
```

O JSON contém a variante, `omitted` e `assembly` com BOM, posições e pendências. Referências inexistentes, repetidas e terra são recusados. IDs, face, rotação e posições incluídos são preservados. Exclusões vazias produzem a montagem completa. Configuração: até 64 KiB, sem campos desconhecidos.

A variante filtra relatórios de montagem. Não muda esquema, netlist, PCB ou simulação do original. Substituições de peças/valores e reconciliação elétrica para DNP estão pendentes. CSV mantém coordenadas em nanômetros e a convenção do [relatório de montagem](pcb-workflows.md).

## Varredura DC

```powershell
protocraft-cli dc-sweep examples/divisor.protocraft --component V1 --start 2 --stop 10 --points 5 --output varredura-fonte.json
protocraft-cli dc-sweep examples/divisor.protocraft --component R2 --start 100 --stop 10000 --points 9 --scale log --engine stable --output varredura-carga.json
```

Valores em SI: volts para fonte DC e ohms para resistor. São aceitos 2–128 pontos, escala linear/log e motores `stable`, `dynamic` e `mixed`. Escala log exige extremos positivos; todos os valores passam pelo Engine. Extremos são incluídos exatamente, inclusive em varredura decrescente.

Cada ponto contém valor e resultado DC completo: tensões de redes e pontos de operação com correntes/potências/terminais dos modelos existentes. Comandos do Engine editam snapshots, preservando o original. Falhas identificam componente e ponto, sem lote parcial como sucesso. Cancelamento e prazo padrão de dez segundos abrangem planejamento e execução do lote inteiro.

Varia-se **um resistor ou uma fonte DC por vez**. AC, Monte Carlo, temperatura, parâmetros de modelo, múltiplas variáveis e transiente por ponto seguem pendentes. Modelos permanecem genéricos.

## Medições temporais e diferenciais

```powershell
protocraft-cli simulate-measure examples/divisor.protocraft --trace "V(VOUT)" --step 0.00001 --stop 0.001 --from 0.00001
protocraft-cli simulate-measure examples/divisor.protocraft --trace "V(VIN)" --reference "V(VOUT)" --step 0.00001 --stop 0.001 --from 0.00001 --threshold 2.5 --output medicao.json
```

Nomes de séries são os emitidos por `simulate`. A referência diferencial deve ter mesma unidade e instantes exatos; não há reamostragem implícita. `--from`/`--to` recortam a janela dentro dos tempos simulados; sem eles usa-se toda a série. A amostra inicial precede a energização: inclua-a quando fizer parte da medição desejada.

Mínimo/máximo, pico a pico, média, RMS e integral caracterizam a **interpolação linear das amostras**. Média e RMS ponderam os intervalos reais, inclusive passos irregulares. A integral tem unidade da série multiplicada por segundo. Não se estima erro de discretização nem se recupera sinal perdido por passo grande.

Com `--threshold`, há duty, subidas, período médio e frequência. Duty é a fração da janela com valor **maior ou igual** ao limiar. Subida exige abaixo → estritamente acima; tangência não conta. Patamar confirmado acima registra o primeiro contato observado na janela. Não são contadas bordas anteriores à janela. Frequência exige ao menos duas subidas confirmadas; caso contrário, é `null`.

A API recusa séries não finitas, tempos repetidos/reversos, menos de duas ou mais de dois milhões de amostras, janelas inválidas, referências incompatíveis e resultados não finitos. Todos os novos relatórios opcionais são publicados atomicamente, após cálculo/serialização completos, sem substituir arquivos existentes.

Para o exemplo 555 em Misto, uma regressão compara a frequência com 1/[ln(2)·(RA+2RB)·C], tolerância relativa de 1,5% com passo de 10 µs:

```powershell
protocraft-cli simulate-measure examples/555-astavel.protocraft --trace "V(VOUT)" --engine mixed --step 0.00001 --stop 0.03 --from 0.005 --threshold 2.5
```

## Limites e regressões

Testes cobrem casos analíticos, passos irregulares, janelas, tangências/patamares, rasgos e limites DFM, idiomas en/pt-BR, entradas inválidas e preservação do original. Consulte `eda-pcb/tests/assembly_variants.rs`, `eda-pcb/tests/dfm.rs`, `eda-simulation/tests/{sweep,measurements}.rs`, `eda-storage/tests/report_output.rs` e os testes CLI correspondentes.

Não houve validação física, fabricação ou execução gráfica neste ciclo. Zonas, multicamadas, push-and-shove, pares diferenciais, AC, passo adaptativo e modelos comerciais seguem pendentes.
