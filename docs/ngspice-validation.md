# Validação independente com ngspice 47

Executada em 22/10/2026 no Windows x64. O executável portátil foi usado apenas como ferramenta de validação externa, fora do produto. Nenhuma biblioteca ou binário ngspice foi incorporado ao ProtoCraft.

## Origem e integridade

A [página oficial de downloads](https://ngspice.sourceforge.io/download.html) indica o ngspice 47 e o pacote Windows. O [download oficial no SourceForge](https://sourceforge.net/projects/ngspice/files/latest/download) publicou SHA-256:

```text
59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a
```

O arquivo obtido tem 13.814.879 bytes e o hash calculado coincidiu exatamente antes da extração. Foi salvo em `C:\Users\Mateus\Downloads\ProtoCraft-Tools\ngspice-47_64-verified.7z` e extraído com 7-Zip para `C:\Users\Mateus\Downloads\ProtoCraft-Tools\ngspice-47`. O programa executado foi `Spice64\bin\ngspice_con.exe`; `--version` confirmou ngspice-47, compilação de 11/08/2026.

URLs diretas inicialmente retornaram HTML, não o arquivo. Esses conteúdos foram rejeitados por hash e nunca extraídos nem executados. A transferência válida usou a URL completa do `meta refresh` da página `latest/download`, incluindo o token temporário de download e mirror fornecido pelo SourceForge.

## Método

Usamos os projetos próprios `examples/divisor.protocraft`, `examples/rc.protocraft` e `examples/diodo.protocraft`. A CLI release do ProtoCraft gerou as netlists com `spice` e os resultados nativos com `simulate --csv`. O RC usou `--transient 0.00001 0.005` nos dois comandos. Os arquivos exportados originais foram preservados sem alteração.

Para leitura dos resultados no ngspice, cópias das netlists receberam somente `.print op v(1) v(2) i(V1)` ou `.print tran v(2)` e uma opção de precisão; os componentes e valores exportados foram mantidos. Execução em modo batch com `-n -b -o`, desabilitando o carregamento de `.spiceinit`. Foram executadas apenas netlists próprias de validação. Não foram usados scripts ou modelos de projetos externos.

A grade de tempo adaptativa do ngspice contém 511 pontos; o backend nativo retorna 501 pontos de passo fixo. A comparação RC interpola linearmente os valores do ngspice para os instantes nativos de 10 µs a 5 ms. A amostra nativa t=0 foi excluída dessa comparação, pois representa explicitamente o estado anterior ao degrau. Os números impressos pelo ngspice têm aproximadamente seis casas significativas; erros menores ficam limitados por essa resolução.

## Resultados observados

| Circuito | ProtoCraft | ngspice 47 | Comparação |
| --- | --- | --- | --- |
| Divisor 10 V, 1 kΩ + 1 kΩ, DC | VOUT=5 V; I(V1)=-5 mA | V(2)=5 V; I(V1)=-5 mA | Coincidência na resolução impressa |
| RC 1 kΩ, 1 µF, t=1 ms | 6,3028878767 V | 6,32122776 V, interpolado | Diferença 18,339883 mV |
| RC, t=5 ms | 9,9309262382 V | 9,932623 V | Diferença 1,696762 mV |
| Diodo Is=1 pA, n=1, R=1 kΩ, 10 V, DC | 0,5936362617 V | 0,5939808 V a 27 °C | Diferença 344,538 µV |
| Mesmo diodo com TEMP=TNOM=26,826793442 °C | 0,5936362617 V | 0,5936390 V | Diferença 2,738 µV |

O máximo desvio RC frente ao ngspice foi **18,339883 mV**, em 1 ms. Ficou abaixo do critério de 20 mV definido para esse caso. O máximo erro nativo frente à solução analítica 10(1-exp(-t/1 ms)) foi 18,317712 mV. A diferença é consistente com o amortecimento de backward Euler de passo fixo; ngspice usa integração e controle de passo próprios.

O diodo nativo fixa Vt=25,85 mV. O ngspice usa temperatura física, com padrão 27 °C, de modo que o modelo exportado com Is/n iguais não define exatamente a mesma tensão térmica. A segunda execução ajustou TEMP e TNOM para 26,826793442 °C, calculados por T=Vt/(k/q) com constantes SI. O erro residual de 2,738 µV ficou abaixo de 5 µV; ele inclui convenções e precisão numérica distintas. Esse ajuste existe apenas na fixture de validação, sem alterar silenciosamente a exportação do aplicativo.

Os testes comprovam os três circuitos simples e os limites de tolerância acima. Não constituem validação de um simulador SPICE completo, modelos comerciais, estabilidade geral, circuitos rígidos ou adequação de engenharia de um projeto físico.

## Evidências locais e reprodução

`artifacts/ngspice-validation/` (ignorado no Git) contém netlists exportadas, cópias com impressão, logs completos, CSVs nativos, `version.txt`, `compare.py`, `comparison.json` e `rc-comparison.csv`.

Exemplo de comandos, executados a partir da raiz:

```powershell
& target/release/protocraft-cli.exe spice examples/rc.protocraft --transient 0.00001 0.005 --output artifacts/ngspice-validation/rc-export.cir
& target/release/protocraft-cli.exe simulate examples/rc.protocraft --transient 0.00001 0.005 --csv artifacts/ngspice-validation/rc-native.csv
# Usar cópia rc-ngspice.cir com .print tran v(2) antes de .end.
& 'C:\Users\Mateus\Downloads\ProtoCraft-Tools\ngspice-47\Spice64\bin\ngspice_con.exe' -n -b -o artifacts/ngspice-validation/rc-ngspice.log artifacts/ngspice-validation/rc-ngspice.cir
python artifacts/ngspice-validation/compare.py
```

O ngspice é ferramenta externa opcional para esta validação; o aplicativo continua usando o backend original MNA.

## Ampliação 0.4 — MOSFET e fontes controladas

Em 22/10/2026 foram comparados mais dez casos DC autorais, usando a CLI release 0.4 e o mesmo ngspice 47 externo. O motor nativo escolhido foi Misto. A CLI gerou cada netlist sem alterações; uma cópia recebeu `.print op` e opção de precisão. Os originais exportados e os projetos `.protocraft` foram preservados em `artifacts/mixed-validation/`, junto do script `compare.py`, CSVs, logs e `comparison.json`.

| Casos | Parâmetros / resultado | Diferença absoluta dos valores impressos |
|---|---|---|
| NMOS e PMOS, corte | k=0,02 A/V², Vt=2 V, λ=0,01 V⁻¹, módulo Vgs=1 V e Vds=5 V | 5 pA de corrente no ramo de alimentação do dreno. |
| NMOS e PMOS, triodo | Módulo Vgs=4 V, Vds=1 V; módulo Id=30,3 mA | Aproximadamente 1,01 pA. |
| NMOS e PMOS, saturação | Módulo Vgs=4 V, Vds=5 V; módulo Id=42 mA | Aproximadamente 5,01 pA. |
| VCVS positivo/negativo | Vin=1 V, ganho ±5, carga 10 kΩ; Vout=±5 V | Zero na resolução impressa. |
| VCCS positivo/negativo | Vin=1 V, gm=±1 mA/V, carga 10 kΩ; Vout=∓10 V | Zero na resolução impressa. |

Critérios: erro absoluto menor que 10 nA nos seis casos MOS e menor que 1 nV nas quatro fontes controladas. Todos passaram. O modelo exportado MOS usa Level 1, W=L=1 µm e corpo ligado à fonte; as resistências explícitas de fuga foram mantidas. As polaridades e sinais também foram conferidos analiticamente nos testes Rust.

As tabelas `.print op` do ngspice exibem cerca de seis algarismos significativos neste modo; a diferença calculada entre valores impressos não mede a precisão interna do ngspice em pA para correntes de dezenas de mA. As diferenças de fugas/GMIN e temperatura não foram eliminadas. Estes dez casos validam as regiões e parâmetros acima, sem estender a conclusão ao diodo de corpo em condução direta, capacitâncias parasitas, transiente MOS ou modelos comerciais. O 555 e a lógica sequencial foram comparados com referências analíticas e funcionais; não receberam validação externa ngspice.

Repetir localmente: `python artifacts/mixed-validation/compare.py`. O script usa somente os projetos autorais que ele mesmo gera e executa ngspice com `-n -b -o`, sem `.include` nem scripts externos.
