# Otimização de rotas, limpeza e revisão de fabricação

As operações deste guia usam o domínio Rust e a netlist única do projeto. Funcionam sem interface gráfica, contas ou rede e preservam o arquivo de entrada. Os parâmetros não alteram o formato `.protocraft` v7.

## Tentativas alternativas de autorroteamento

```powershell
ProtoCraft-CLI.cmd pcb-route placa.protocraft --output roteada.protocraft --passes 4 --max-expansions 600000
```

`--passes` aceita de 1 a 8; o padrão é 1. Cada tentativa parte do cobre original, alterna a ordem e rotaciona deterministicamente a lista de ligações. O motor escolhe o plano com menos airwires; em empate, considera comprimento e penalidade de vias. Cobre preexistente é preservado. As buscas continuam sujeitas ao DRC, às classes de rede e aos limites geométricos atuais de duas camadas.

O orçamento de expansões é repartido entre as tentativas e inclui as otimizações e tentativas rejeitadas pelo DRC. Um plano válido anterior é preservado se uma tentativa posterior falhar. `stats` informa `routing_passes_attempted`, `routing_passes_rejected`, `selected_pass` (base 1), `search_expansions` e o tempo total. Ao fechar todas as ligações, o motor encerra as tentativas restantes. O motor não faz rip-up seletivo de trilhas; a CLI oferece `--ripup-all` para refazer todo o cobre em uma cópia do projeto.

Repartir o mesmo orçamento em mais tentativas pode reduzir o trabalho disponível por tentativa e fechar menos conexões que uma tentativa única. A opção permite experimentar outras ordens; compare airwires, comprimento e vias, em vez de presumir melhoria para toda placa. A saída com rotas parciais permanece editável e registra as falhas individuais.

## Unir cobre colinear

```powershell
ProtoCraft-CLI.cmd pcb-cleanup placa.protocraft --merge-collinear --output limpa.protocraft
```

Sem a opção, a limpeza conserva o comportamento de remover duplicatas exatas. Com ela, também une trechos contíguos ou sobrepostos da mesma rede, largura e face, preservando o menor ID. Trechos disjuntos ou de camadas, larguras ou redes distintas não são unidos. Pads, vias e ramificações interiores são protegidos; o DRC e as classes de rede são conferidos antes de aceitar uma união. Uma união que deixaria um escape de pad acima do comprimento permitido é descartada.

O plano é um lote atômico reversível no Engine. `removed_tracks` conta segmentos excluídos; `merged_tracks` conta uniões, incluindo trechos totalmente cobertos. O número final de segmentos diminui em `removed_tracks`. A operação recusa aumento de airwires e limita a etapa colinear a 256 segmentos de entrada e 100.000 comparações, com erro explícito se exceder o orçamento. Esse limite torna o custo previsível; placas maiores exigem uma evolução do índice de busca.

## Exigir conclusão na CLI

O comando padrão grava a melhor solução encontrada mesmo quando algumas conexões permanecem pendentes. O JSON retorna `status: "complete"` ou `status: "partial"`; portanto, automações devem conferir esse campo ou o contador `stats.remaining_airwires`.

Para fluxos que só aceitam placas totalmente conectadas, use:

```powershell
ProtoCraft-CLI.cmd pcb-route placa.protocraft --output roteada.protocraft --require-complete
```

Se houver airwires, a CLI retorna código diferente de zero, imprime `status: "incomplete"` e não cria o arquivo de saída. O projeto original permanece intacto.

## Cobertura dos exemplos de referência

O teste lento `autoroutes_reference_boards_without_new_physical_violations` remove trilhas e vias em uma cópia de trabalho via `--ripup-all`, executa o autorroteador com orçamento alto e sem otimização e compara o relatório com o DRC. Ele exige conclusão do 555 e do shield; no buck, aceita somente pendências `UNROUTED` e ainda não comprova roteamento integral. Rode-o explicitamente com:

```powershell
cargo test -p eda-cli --test routing_cli autoroutes_reference_boards_without_new_physical_violations -- --ignored
```

Na medição inicial de 2026-11-09, o 555 concluiu 15/15 conexões; o shield Pi 4 concluiu 4/7; e o buck concluiu 22/57. Após as melhorias de fanout e orçamento, o shield foi reroteado de cobre limpo com 7/7 airwires resolvidos, zero vias e zero diagnósticos DRC em 21,6 s. O buck foi medido em cópias incrementais: 44/57 na primeira passagem, +6 na segunda e +3 com grade de 0,5 mm, totalizando 53/57. Restaram quatro airwires (VIN12 duas vezes, VCC e FB), somente com diagnóstico `UNROUTED`. A grade de 0,125 mm não reduziu essa pendência. O gerador buck continua produzindo seu exemplo inicial conectado (187 trilhas, zero airwires/erros), mas isso não equivale a reroteamento de cobre limpo.

Essas medições são comparativas, não uma garantia de conclusão. A validação integral do buck e a revisão independente dos arquivos Gerber/Excellon permanecem abertas no `BACKLOG.md`; o teste de reroteamento continua lento e ignorado no conjunto regular. Nenhum dos resultados atesta capacidade de corrente, térmica, EMI ou fabricabilidade.

## Rever limites do processo de fabricação

```powershell
ProtoCraft-CLI.cmd pcb-dfm --default-limits
ProtoCraft-CLI.cmd pcb-dfm placa.protocraft --limits docs\examples\manufacturing-limits.json --output revisao.json
ProtoCraft-CLI.cmd pcb-manufacture placa.protocraft --output fabricacao-nova --kicad caminho\kicad-cli.exe --dfm-limits docs\examples\manufacturing-limits.json
```

O arquivo de limites é JSON com sete campos inteiros em nanômetros; `1 mm = 1.000.000 nm`. O [perfil de exemplo](examples/manufacturing-limits.json) usa largura/folga de 0,15 mm, furo de via/PTH de 0,30 mm, NPTH de 0,50 mm, anel de 0,10 mm e cobre–borda de 0,25 mm. São valores genéricos para demonstrar o recurso. Substitua pelos limites contratados para a fábrica, stack-up e processo utilizados. Campos desconhecidos, valores inválidos e arquivos maiores que 64 KiB são recusados.

O relatório identifica objetos, componentes, pads, posições, códigos estáveis e, nas verificações DFM específicas, valor real e mínimo em nm. Os limites mais estritos de folga/borda são aplicados a um snapshot; as regras existentes do projeto continuam valendo. Escapes locais autorizados no projeto não dispensam o furo e a largura mínima exigidos pelo processo. Por exemplo, as vias de escape de 0,20 mm do buck Raspberry Pi são reprovadas pelo perfil genérico com furo mínimo de 0,30 mm; é necessário escolher um processo compatível ou revisar o layout.

O DFM verifica largura de trilha, ferramenta de furação de vias/PTH/NPTH, anel pelo envelope do pad e o DRC nativo sob as folgas solicitadas. Para slots, a menor dimensão é o diâmetro da ferramenta. Pads metalizados furados com geometria personalizada exigem revisão manual do anel: recebem aviso e `complete: false`. O mesmo ocorre se o DRC nativo atingir seu limite de diagnósticos. `complete: true` indica conclusão desse subconjunto de análise.

O comando grava/imprime o relatório mesmo quando há violações e termina com código diferente de zero se houver erros ou análise incompleta. `--output` exige um arquivo JSON novo. O projeto não é editado. Na fabricação, `--dfm-limits` bloqueia o processo antes de chamar KiCad ou criar a pasta quando a revisão falha. Se o processo concluir, os limites e resultados ficam em `dfm.json` na pasta de fabricação, fora do ZIP Gerber; o DRC externo continua obrigatório.

Máscara, pasta, silk, dissipação, capacidade de corrente, integridade de sinal, stack-up e processo físico de montagem exigem verificações adicionais. Zonas, multicamadas, push-and-shove, pares diferenciais, ajuste de comprimento e controle de impedância permanecem no backlog.

## Referência funcional

As páginas públicas da Labcenter descrevem [autorroteamento e ferramentas de PCB](https://www.labcenter.com/pcb-layout/) e [verificações e pré-produção](https://www.labcenter.com/verification/). Orientaram a seleção dos fluxos desta rodada; os algoritmos e contratos implementados são próprios do ProtoCraft. Os recursos aqui são incrementais e não representam equivalência integral ao Proteus.
