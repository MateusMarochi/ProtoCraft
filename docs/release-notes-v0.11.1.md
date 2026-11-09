# ProtoCraft v0.11.1

## Novidades

- Autorroteador PCB aprimorado para pads SMD densos: fanout local com vias curtas aplica as regras de largura e dimensões de via da classe de rede e verifica cada escape pelo DRC.
- CLI de roteamento com limite de expansões por conexão, priorização opcional de conexões curtas, reconstrução em cópia com `--ripup-all` e modo `--require-complete` para impedir saída parcial.
- Relatórios de roteamento identificam estado completo/parcial, conexões pendentes e estatísticas de busca. O arquivo de entrada permanece inalterado.
- README ampliado com prévias SVG do shield GPIO para Raspberry Pi 4 e da fonte buck 12 V → 5,1 V / 3 A, vinculadas aos projetos nativos e às limitações documentadas.

## Validação e limites conhecidos

Os projetos salvos do shield e do buck foram inspecionados pela CLI: respectivamente 49 e 98 pads, zero diagnósticos DRC e zero airwires. Isso valida apenas as regras geométricas implementadas para esses arquivos; não confirma encaixe físico, capacidade de corrente, temperatura, EMI ou fabricação.

O shield consegue ser roteado de cobre limpo pelo autorroteador nos limites registrados. O reroteamento do buck a partir de cobre vazio ainda pode deixar quatro conexões pendentes (duas em `VIN12`, uma em `VCC` e uma em `FB`). O projeto de referência salvo do buck continua roteado e sem airwires; esse resultado não significa que uma nova execução do autorroteador irá reproduzi-lo.

A fonte buck não tem simulação chaveada nem validação física. A revisão de footprints, stack-up, caminhos de retorno, térmica, EMI e arquivos de fabricação continua necessária antes de fabricar ou conectar uma Raspberry Pi.
