# Importação de circuitos Falstad/CircuitJS

O ProtoCraft lê arquivos de texto legados iniciados por `$`, arquivos XML
CircuitJS iniciados por `<cir>` (mesmo quando a extensão é `.txt`), atalhos
Windows `.url` e links HTTPS oficiais do Falstad. Links longos com `cct` ou
`ctz` são decodificados localmente; links curtos `/s.py?s=...` consultam o
servidor Falstad e têm prazo de 15 segundos. A importação não executa código,
diretivas ou modelos recebidos. O arquivo original não é modificado.

Na interface, use **Arquivo → Importar Falstad…** e escolha o arquivo ou cole
o link. Também é possível passar o caminho ou a URL diretamente na abertura
do executável desktop. Salve o resultado como um novo `.protocraft` e consulte
a aba **Importação** antes de editar ou simular.

Pela CLI:

```powershell
protocraft-cli import-falstad circuito.txt --output circuito.protocraft
protocraft-cli import-falstad 'https://www.falstad.com/s.py?s=dKcFWE' --output circuito-link.protocraft
protocraft-cli import-falstad circuito.xml --output circuito.protocraft --open
```

A CLI emite JSON com contagem de componentes, fios, avisos, objetos sem modelo
e `electrical_complete`. Nunca substitui o destino existente. `--open` depende
do executável desktop instalado ao lado da CLI.

## Escopo real

- Posições, terminais e fios ortogonais são convertidos para geometria inteira.
  Fios diagonais preservam a ligação dos extremos como rótulos, com aviso de
  diferença visual.
- Resistores, capacitores, indutores, terra, fontes DC/seno/quadrada, chaves,
  diodos, LEDs, Zeners, BJTs NPN/PNP e MOSFETs de três terminais são
  identificados. Amplificadores operacionais `a` são convertidos para o modelo
  genérico de ganho e alimentação do ProtoCraft. Rótulos `ln` viram nomes de
  redes nativas e conectam pontos com o mesmo nome. Os tipos não lineares usam
  modelos genéricos; a CLI e a aba Importação avisam que os resultados podem
  diferir do CircuitJS. Diodos transferem Is, N, resistência série e tensão
  Zener quando presentes no modelo. A resistência interna de fontes é
  preservada como resistor série nativo.
- Para um tipo de elemento desconhecido, o importador não afirma a quantidade
  nem a ordem dos pinos. Mantém apenas as duas pontas geométricas do registro,
  marca a pinagem como desconhecida e bloqueia a simulação do circuito.
- Condições iniciais não nulas de capacitores/indutores, resistência interna
  negativa de fonte, dispositivos desconhecidos, MOSFETs com terminal de corpo
  externo e formas de onda não suportadas mantêm o projeto parcial e bloqueiam
  a simulação, em vez de serem silenciosamente descartados. Resistência interna
  positiva de fonte é preservada como resistor série genérico.
- Sondas `p`, marcadores de saída `O` e anotações `x` não têm efeito elétrico;
  são omitidos com aviso, sem bloquear a simulação. Gráficos, instrumentos e a
  disposição visual original não são reconstruídos.
- Anotações, gráficos, instrumentos, parâmetros de simulação e apresentação
  próprios do CircuitJS não são reconstruídos no editor. A aparência não é
  uma reprodução da interface Falstad.
- Definições de modelos, subcircuitos, scopes, ajustes e marcadores de teste
  são reconhecidos como metadados, mas não são executados nem reconstruídos.
- Entrada limitada a 2 MiB de circuito e 4096 registros, com limites de
  coordenadas, entidades XML, expansão do link comprimido e redirecionamentos.
  Apenas hosts HTTPS Falstad permitidos podem ser consultados por link curto.

Os modelos de diodo, chave, BJT e MOSFET são aproximações explícitas do solver
próprio; eles permitem executar vários circuitos importados, mas não reproduzem
as equações completas, eventos temporizados ou capacitâncias parasitas do
CircuitJS. Os arquivos de exemplo pessoais não são distribuídos; as regressões
automatizadas usam circuitos sintéticos autorais.

Referências de formato: [site oficial](https://www.falstad.com/circuit/),
[carregador de circuitos](https://raw.githubusercontent.com/pfalstad/circuitjs1/master/ts/CircuitLoader.ts),
[desserializador XML](https://raw.githubusercontent.com/pfalstad/circuitjs1/master/ts/CircuitXMLDeserializer.ts),
[sonda](https://raw.githubusercontent.com/pfalstad/circuitjs1/master/ts/ProbeElm.ts) e
[saída](https://raw.githubusercontent.com/pfalstad/circuitjs1/master/ts/OutputElm.ts).
O decodificador `ctz` adapta o formato LZ-String sob MIT, com aviso em `NOTICE.md`.
