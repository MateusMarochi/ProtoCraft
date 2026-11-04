# Galeria interna de PCB

O menu **Exemplos → Exemplos de PCB** abre projetos `.protocraft` incorporados ao aplicativo. A mesma galeria sai pela CLI com `protocraft-cli examples <pasta>`. Os arquivos são projetos normais: podem ser editados, roteados, verificados e salvos em outro nome.

| Exemplo | O que demonstra | Estado inicial |
|---|---|---|
| **Básico · trilhas, vias e furos** | Geometria de cobre, furação e uma classe de rede. | DRC sem erros. |
| **Intermediário · pisca-pisca 555 roteado** | Esquema ligado à PCB, duas faces, 45 segmentos, uma via e classes separadas para sinal e alimentação. | DRC sem erros; acompanha documentação de exportação. |
| **Avançado · STM32 + ESP32-S3** | Catálogo físico, mapeamento de pads e parâmetros de largura por classe. | Mantém conexões pendentes para experimentar o autorroteador; não é uma placa pronta para fabricação. |
| **Complexo · shield Raspberry Pi 4** | Header GPIO de 40 pinos, placa arredondada, furos mecânicos, indicadores 5 V/3,3 V e regras de rede. | Projeto nativo com rotas e DRC; compatibilidade mecânica com gabinetes exige revisão própria. |
| **Avançado · buck 12 V para Raspberry Pi 4** | Conversor síncrono LM61460, dimensionamento de primeira ordem, nets de potência e roteamento com trilhas de 0,8 mm na saída de 3 A. | Protótipo roteado, sem erros geométricos no DRC; não validado por simulação chaveada, ensaio térmico, EMI ou fabricação. |

As classes de rede ligam nomes exatos do netlist a largura mínima de trilha, folga mínima e dimensões mínimas de via. O DRC sinaliza atribuições que não encontram mais a rede, trilhas estreitas, vias abaixo das dimensões e folgas insuficientes entre redes. O autorroteador usa essas dimensões ao planejar cada conexão. Redes sem classe herdam as regras globais da placa.

As classes também podem ser editadas sem a interface. `list` imprime o mapa atual em JSON; `set` cria ou substitui uma classe; `remove` exclui uma classe. Os comandos de gravação exigem uma saída nova e nunca substituem o projeto de entrada:

```powershell
ProtoCraft-CLI.cmd pcb-netclass placa.protocraft list
ProtoCraft-CLI.cmd pcb-netclass placa.protocraft set --name ALIMENTACAO --net 5V --net 3V3 --width-mm 0.5 --clearance-mm 0.25 --via-diameter-mm 0.7 --via-drill-mm 0.3 --output placa-regras.protocraft
ProtoCraft-CLI.cmd pcb-netclass placa-regras.protocraft remove --name ALIMENTACAO --output placa-sem-classe.protocraft
```

O DRC e o autorroteador consomem essas regras nomeadas. A implementação ainda usa uma regra global por classe: regiões, prioridades entre regras sobrepostas, pares diferenciais, impedância controlada e regras por camada continuam no backlog.

Para gravar os projetos nativos em uma pasta vazia:

```powershell
ProtoCraft-CLI.cmd examples projetos\galeria-pcb
ProtoCraft-CLI.cmd pcb-check projetos\galeria-pcb\pcb-555-led-1hz.protocraft
ProtoCraft-CLI.cmd pcb-route projetos\galeria-pcb\pcb-stm32-esp32.protocraft --output projetos\galeria-pcb\stm32-esp32-roteado.protocraft
```

O exemplo STM32/ESP32 inclui mais redes e componentes que os exemplos curtos; o autorroteador pode deixar conexões pendentes ao atingir os limites de busca ou não encontrar um caminho. Confira `pcb-check` depois de cada edição. As classes são regras mínimas e não substituem impedância controlada, cálculo térmico, análise de corrente, revisão elétrica ou inspeção de fabricação.
