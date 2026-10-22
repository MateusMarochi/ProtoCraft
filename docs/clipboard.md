# Copiar, recortar e colar componentes — 0.6.1

Selecione um componente e use os atalhos ou **Editar → Copiar / Recortar / Colar componente**.

| Atalho | Comportamento |
| --- | --- |
| Ctrl+C | Copia um snapshot do componente para a área de transferência do sistema. |
| Ctrl+X | Recorta a seleção. No esquema, retira o componente; na PCB, retira seu footprint e conserva o componente lógico. |
| Ctrl+V | Cola no cursor do canvas atual. Uma cópia recebe ID e referência únicos; um recorte local válido mantém os originais. |
| Ctrl+Z / Ctrl+Y | Desfaz/refaz alterações de modelo, inclusive recorte e colagem. |

O snapshot conserva tipo, valor, parâmetros, rotação, pinos importados, definição STM32/ESP32, footprint e proveniência dos dados. Fios, junções e rótulos externos não fazem parte da cópia de um componente.

## Esquemático

Colar coloca o componente no cursor, respeitando a grade. Se o cursor estiver fora do canvas, usa deslocamentos sucessivos de quatro unidades a partir do original. Ao concluir, a nova seleção é o componente colado e a ferramenta volta para Selecionar.

Recortar e colar no mesmo documento restaura o componente com seu ID e referência, alterando sua posição. Se ele tinha posição na PCB e cobre ancorado, esses dados são restaurados nas coordenadas físicas originais. Os fios do esquema não acompanham automaticamente o movimento.

Recortar e colar são duas alterações: um Ctrl+Z após colar volta ao estado recortado; outro restaura o estado anterior ao recorte. Desfazer o recorte antes de colar faz a colagem seguinte funcionar como cópia, com nova identidade.

## PCB

A seleção usada é o footprint da PCB. Copiar e colar cria um componente lógico novo e o posiciona fisicamente no cursor, preservando rotação e face. Seu símbolo é acrescentado em uma área isolada à direita do esquema, evitando conexões elétricas acidentais por sobreposição de pinos. Associe as conexões desejadas no esquemático.

Recortar na PCB preserva o componente lógico e retira o posicionamento. Ao colar na PCB do mesmo documento, o footprint é reposicionado com o mesmo ID/referência. Trilhas e vias ancoradas nele são guardadas no recorte e restauradas nas posições originais; IDs de cobre ocupados por novas edições são remapeados sem sobrescrever objetos. As trilhas não se deslocam automaticamente com o footprint: confira as conexões pendentes.

Colar na PCB exige footprint e passa pelo DRC de edição. Uma colagem recusada mantém o modelo e o recorte disponíveis. Se o recorte foi feito na PCB, conclua-o nessa visualização ou use Ctrl+Z para restaurar. Furos mecânicos independentes não são copiados ou recortados junto com componentes.

## Clipboard e edição de texto

Em campos de texto, Ctrl+C/X/V permanecem sob controle do editor de texto. Copiar texto não recorta componentes, e colar texto não duplica o último componente copiado.

A área de transferência usa texto identificado por `PROTOCRAFT_COMPONENT_V1`, com JSON validado e limite de 8 MiB. Não há execução de arquivos ou modelos ao colar. Texto comum, JSON inválido e componentes inconsistentes são recusados no canvas, sem usar silenciosamente um snapshot antigo.

O clipboard do sistema permite transferir componentes entre projetos ou instâncias compatíveis do ProtoCraft. Em outro documento, a operação cria uma cópia com identidade nova. A continuidade de um recorte com ID original pertence à sessão/documento de origem; abrir outro documento descarta essa continuidade.
