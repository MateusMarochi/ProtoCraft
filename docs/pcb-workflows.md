# Construção de PCB e relatórios de montagem

Os comandos abaixo operam sobre o mesmo domínio Rust do desktop. Comandos que gravam projetos exigem um destino novo e preservam o original. Firmware e visualização 3D não participam destes fluxos.

## Sincronizar e inspecionar

```powershell
protocraft-cli pcb-sync-report esquema.protocraft
protocraft-cli pcb-sync esquema.protocraft --output placa.protocraft
protocraft-cli pcb-info placa.protocraft
protocraft-cli pcb-check placa.protocraft
```

A sincronização adiciona os footprints já associados aos componentes que ainda não têm posição. Preserva os posicionamentos e o cobre existentes. Novos componentes são organizados em linhas abaixo dos corpos existentes; falta de espaço recusa o plano inteiro. Ela não associa automaticamente footprints a componentes sem definição física nem promete posicionamento ideal: confira o relatório DRC retornado. O desktop usa o mesmo serviço no comando de criar/sincronizar footprints.

`pcb-info` também apresenta `routing_metrics`: comprimento geométrico por rede e face, número de segmentos, vias e pinos, com nomes derivados da netlist. A soma considera o centro dos segmentos, inclusive sobreposições; não inclui o comprimento vertical de vias nem demonstra continuidade, atraso de propagação ou impedância. IDs das redes são derivados da revisão corrente.

## Mover preservando conexões

```powershell
protocraft-cli pcb-move placa.protocraft --component 1 --x-mm 25 --y-mm 30 --output placa-movida.protocraft
```

A operação translada um footprint na mesma face e rotação, atualizando extremidades de trilhas em contato com seus pads, inclusive contatos fora do centro. Preserva os IDs e redes e valida o lote com o DRC antes de aplicar. Contatos ambíguos, trilhas atravessando pads, ramificações ou vias em contato são recusados para impedir uma desconexão silenciosa. Movimentos que introduzem novas violações físicas ou mais conexões pendentes são recusados. Posição inalterada é uma operação vazia.

No desktop, **Manter trilhas conectadas ao arrastar** vem habilitado. A opção afeta a translação por arraste; rotação e troca de face ainda exigem revisão e reconexão manual. Desabilitar a opção mantém somente a verificação física da edição, sem prometer acompanhar trilhas.

## Remover duplicatas

```powershell
protocraft-cli pcb-cleanup placa.protocraft --output placa-limpa.protocraft
```

Remove trilhas exatamente coincidentes da mesma rede, largura e camada, inclusive com início e fim invertidos, preservando o menor ID. A operação é reversível como lote pelo Engine e passa pelo DRC. Não une segmentos parcialmente sobrepostos ou colineares. No desktop, a ferramenta está em **Trilhas e furos → Remover trilhas duplicadas**.

## Lista de materiais e posições

```powershell
protocraft-cli pcb-assembly placa.protocraft
protocraft-cli pcb-assembly placa.protocraft --format bom-csv --output bom.csv
protocraft-cli pcb-assembly placa.protocraft --format placements-csv --output positions.csv
```

A saída padrão é JSON, incluindo `bom`, `placements`, `missing_footprint` e `unplaced`. A BOM agrupa tipo, valor, unidade, dispositivo e footprint; exclui GND, que não representa peça física. Dispositivos distintos não são agrupados apenas por terem o mesmo encapsulamento. Componentes incompletos permanecem na BOM e nas listas de pendências; só componentes com footprint e posicionamento geram linhas de posição.

CSV usa cabeçalhos de domínio estáveis, vírgulas como separadores e aspas duplicadas para preservar texto. Dados textuais são preservados; ao abrir CSV de entrada não confiável em uma planilha, configure importação como texto. Coordenadas são inteiras em **nanômetros**, no referencial da placa; ângulo é `rotation × 90°`. A face inferior é espelhada antes de rotacionar, conforme o modelo. Estas saídas são genéricas e exigem conversão/revisão para a convenção de cada montadora; não são um preset de montagem JLCPCB. O desktop oferece as duas exportações CSV em **Fabricação** e recusa sobrescrever arquivos existentes.

O comando de fabricação também cria uma subpasta `montagem/` contendo `assembly.json`, `bom.csv` e `placements.csv`, a partir do mesmo snapshot usado no projeto KiCad exportado. Essa pasta fica fora do `JLCPCB.zip`; o ZIP conserva os Gerbers e arquivos de furação. Confira pendências e convenções antes de usar os relatórios para compras ou montagem.

## Idioma da automação

A CLI aceita `PROTOCRAFT_LANGUAGE=en`, `pt-BR` ou `system` como preferência do processo. Na ausência da variável, usa o idioma do sistema. Valores inválidos produzem erro. Chaves JSON, IDs e cabeçalhos CSV continuam estáveis; ajuda e mensagens humanas passam pelos catálogos de idioma.

## Validação e limites

As regressões cobrem contato de cobre e furação em folga zero, camadas opostas, via conectando faces, tolerância do DRC, sincronização recusada sem mutação, movimento conectado e undo/redo, remoção de duplicatas e fluxos reais de CLI. Um teste de autorroteamento parte de uma rede desconectada e verifica criação de trilhas e encerramento de airwires; orçamento insuficiente continua sendo informado como conexão pendente.

O exemplo autoral 555 foi exportado com KiCad CLI 10.0.7: DRC nativo sem diagnósticos/conexões pendentes e DRC externo sem violações de severidade error nem itens desconectados, respeitadas as verificações ignoradas no relatório. O pacote contém nove Gerbers, dois arquivos de furação e os relatórios de montagem separados. Isso valida essa fixture e o fluxo de exportação; não constitui aprovação física ou fabril de qualquer placa.
