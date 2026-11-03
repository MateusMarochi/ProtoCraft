# Backlog de evolução do ProtoCraft

Avaliação em 03/11/2026. Escopo: fluxos de esquemático, simulação e PCB comparados às funções publicamente descritas do Proteus; visualização 3D fica para o futuro e não integra este backlog. Esta é uma análise de lacunas e riscos, não uma lista exaustiva de todos os recursos ou edições do produto comercial. Cada item exige descoberta e critérios próprios antes da implementação. Nenhuma compatibilidade integral é presumida.

## Evidências e método

Foram examinados README, roadmap, arquitetura, guias de importação, roteamento e simulação, código e alterações locais. Recursos assinalados como existentes são evidência de implementação ou documentação, não garantia de validação integral nesta avaliação. A documentação histórica de testes não equivale a uma nova execução. Fontes primárias: [captura de esquemas](https://www.labcenter.com/schematic/), [PCB e fabricação](https://www.labcenter.com/pcb-layout/), [roteamento manual](https://www.labcenter.com/manualrouting/), [regras](https://www.labcenter.com/verification/), [alta velocidade](https://www.labcenter.com/highspeed/) e [simulação](https://www.labcenter.com/simulation/). Algumas funções dependem da edição/licença Proteus. Não foi feita nova descompilação: PDS.EXE é o gerenciador, e as fontes públicas e o repositório bastam para identificar estas lacunas; observações anteriores de Ghidra constam em `MECHANICS.md` e não foram verificadas independentemente aqui. Usar engenharia independente e verificar direitos antes de reutilizar qualquer material.

Prioridades: **P0** confiabilidade e preservação; **P1** completar fluxo básico; **P2** recursos ampliados; **P3** avançado e dependente de decisão. P3 não antecipa MCU sobre as bases. Itens não marcados indicam trabalho pendente ou necessidade de investigação, inclusive onde há implementação parcial.

## Estado observado

| Área | Existe no repositório | Limite principal |
|---|---|---|
| Núcleo | Engine headless, comandos, revisão e histórico; formato versionado | Validar consistência de histórico, backups e recuperação sob falhas |
| Esquema | Símbolos, fios, junções, labels, ERC e netlist | Movimento/rotação não acompanha fios; hierarquia e barramentos pendentes |
| Simulação | DC/transiente e eventos mistos próprios, gráficos e CSV | Modelos genéricos, limites numéricos e sem AC ou execução de firmware |
| PCB | Duas faces, trilhas/vias/furos, DRC, autorroteador A* | Placa retangular, sem zonas, regras por classe ou roteamento avançado |
| Fabricação | Exportação via KiCad CLI, Gerber/Excellon e preset | Validação independente limitada a fixtures; não garante fabricação |
| Importação | Perfil restrito Proteus, com bloqueio de simulação | Não prova equivalência elétrica nem importa todos os objetos |
| Automação | CLI e MCP headless | Sessão MCP não controla documento desktop aberto |

## P0 — Integridade e confiança

- [ ] **P0-01 Histórico:** decidir profundidade configurável; a alteração local fixa 20 snapshots, enquanto `docs/architecture.md` registra 100. Testar limite, sequência de undo/redo, ramificações, IDs e consumo de memória.
- [ ] **P0-02 Backups:** revisar rotação local de dez gerações: cópias e remoções não são uma transação; uma falha intermediária pode deixar gerações inconsistentes. Testar falhas de I/O, arquivo ausente, permissões, interrupção e recuperação, sem sobrescrever projeto inválido. Documentar retenção real.
- [ ] **P0-03 Autosave e recuperação:** arquivo separado, identificação de revisão e escolha explícita de recuperação; testar falha de energia, colisões e múltiplas instâncias. Não sobrescrever projeto válido silenciosamente.
- [ ] **P0-04 Conectividade ao editar:** mover/rotacionar/excluir componentes e fios sem criar ou manter conexões indevidas; especificar política de arraste, junções, T, cruzamentos, rótulos e pinos. Cobrir undo/redo, salvamento e netlist antes/depois.
- [ ] **P0-05 Consistência esquema–PCB:** detectar remoções, alterações de pinos/nets/footprints e cobre órfão; oferecer comparação e reconciliação explícita sem perder posicionamento. Testar curtos e mudanças de identidade.
- [ ] **P0-06 Segurança de fabricação:** ampliar testes geométricos para folgas, furos, anel, contorno, pads e conexão elétrica, confrontando DRC e arquivos com ferramenta independente; bloquear saída em erros e explicar limites. Confirmar que terminação de trilha próxima a furo metalizado representa cobre conectado na saída real.
- [ ] **P0-07 Entradas e persistência:** fuzzing e limites de contagem, profundidade, tamanho e trabalho para projetos e importadores; arquivos truncados, versões futuras, migrações e round-trip elétrico; escrita atômica testada no Windows.
- [ ] **P0-08 Regressões das mudanças pendentes:** testes específicos para limite de histórico, dez backups e falhas, sufixo CLI e captura; corrigir divergências de documentação e formatação. Confirmar se reduzir histórico e remover backups antigos é comportamento desejado.
- [ ] **P0-09 Comunicação de escopo:** alinhar ajuda da interface e roadmap: MCU não precede as bases por decisão implícita; não afirmar integração 3D externa sem contrato aprovado. Revisar strings e fallback de idiomas.

## P1 — Editor de esquemas e conectividade

- [ ] Seleção múltipla, caixa, mover/duplicar/alinhar, exclusão e propriedades em lote com transação e undo coerentes.
- [ ] Edição completa de fios: inserção/remoção de vértices, dividir/unir, snap, preview e cancelamento; encontros em T e cruzamentos testados.
- [ ] Barramentos e entradas de barramento com nomes, membros e diagnósticos de inconsistência; sem ligar redes apenas pela aparência.
- [ ] Folhas múltiplas e hierarquia: portas, escopos locais/globais, referências e navegação; testar colisões e netlist determinística.
- [ ] Anotação automática/manual, renumeração segura, referências duplicadas e busca cruzada de componentes, nets e diagnósticos.
- [ ] ERC configurável por tipos de pino: saídas em conflito, alimentação ausente, pinos não conectados intencionalmente e vínculos aos objetos; distinguir avisos de erros.
- [ ] Edição de símbolos e componentes: desenho, propriedades, pinos numerados/tipados, mapeamento explícito para pad e modelo; biblioteca local versionada e snapshots reprodutíveis.
- [ ] Melhorias de produtividade: impressão/PDF, folhas e carimbo, texto/gráficos não elétricos, alinhamento, atalhos configuráveis e acessibilidade; testar DPI, teclado e interação real.

## P1 — PCB e fabricação básica

- [ ] Contornos fechados arbitrários, recortes e fendas; geometria inteira, autointerseção e exportação verificadas independentemente.
- [ ] Editor de footprints e pads THT/SMD, formas e furos alongados, rotação/face, máscara e pasta; validar associação pino–pad e saída fabril.
- [ ] Posicionamento por seleção múltipla, alinhamento, bloqueio, espelhamento e agrupamento; atualização de ratsnest e DRC em tempo real.
- [ ] Edição de trilhas conectadas sem rompimento silencioso, arraste de cantos e segmentos, remoção de rota e retomada; manter regras e undo atômico.
- [ ] Zonas de cobre: preenchimento determinístico por rede/camada, isolamento, ilhas, térmicos, prioridade e atualização após edição; comparar geometria exportada e conexões.
- [ ] DRC abrangente: matriz cobre–cobre, cobre–borda, furos, máscara, largura, anel e conectividade; localizar objetos, severidade e exceções justificadas; fixtures positivas e negativas.
- [ ] Exportação: conjuntos Gerber/Excellon completos e verificados por leitor independente, origem/unidades, PTH/NPTH, silkscreen, máscara, pasta e contorno. Testar rastreabilidade entre revisão e pacote e impedir saída parcial.
- [ ] BOM e posição de montagem: referências, valores, footprint, lado, rotação, origem, unidades e variantes; validar convenções com fixtures de montagem.
- [ ] Validar fluxo completo com placas autorais de complexidade crescente, DRC externo, inspeção visual e, somente se autorizado, retorno de fabricação; documentar falhas sem chamar o autorroteador de profissional.

## P2 — PCB avançada

- [ ] Empilhamento e multicamadas, pares de camadas, materiais e espessuras; vias cegas/enterradas somente após modelo e exportação verificáveis.
- [ ] Classes de rede e regras por objeto/região/camada com precedência explícita, perfis de fabricante e diagnóstico de conflitos.
- [ ] Roteamento interativo com desvio de obstáculos, push-and-shove, edição preservando conectividade e controle de DRC; limites de desempenho e cancelamento.
- [ ] Pares diferenciais, espaçamento e skew, ajuste de comprimento, grupos e relatórios com tolerâncias; impedância apenas com modelo e parâmetros de stackup verificáveis.
- [ ] Autorroteador: rip-up/reroute controlado, regiões e classes, métricas reproduzíveis, orçamento/cancelamento e comparação em placas reais; não prometer conclusão.
- [ ] Planos divididos, costura de vias, via fences e teardrops com regeneração e DRC; testes de conectividade e fabricação.
- [ ] Pontos de teste, cobertura e relatórios; variantes de montagem, perfis de saída reutilizáveis e verificação pré-produção.

## P2 — Simulação, instrumentos e bibliotecas

- [ ] Validar mais casos analíticos e contra ngspice independente: tolerâncias, unidades, convergência, singularidade, circuitos flutuantes, estabilidade e limites de escala.
- [ ] AC de pequenos sinais, varreduras, frequência, fase e análise paramétrica; comparar com soluções analíticas e backend independente. Ruído e Fourier só com critérios próprios.
- [ ] Transiente com controle de erro/passo adaptativo, condições iniciais editáveis e eventos; comparar RC, RLC e descontinuidades, inclusive cancelamento e obsolescência.
- [ ] Probes, osciloscópio, multímetro, geradores e gráficos múltiplos: unidades, referência, escalas, cursores, exportação e limites explícitos dos modelos.
- [ ] Catálogo original ampliado, pesquisa, editor e versionamento de símbolos, modelos e footprints; registrar proveniência/licença e nunca inferir equivalência de modelo genérico a peça comercial.
- [ ] Backend SPICE opcional supervisionado: decisão de licença/distribuição, timeout, cancelamento, restrição de diretivas e arquivos; preservar revisão da simulação e não executar scripts arbitrários.
- [ ] Diagnósticos de convergência e modelos sem suporte devem identificar componente e condições; resultados antigos devem ser identificados após edição.

## P2 — Interoperabilidade e automação

- [ ] Importação Proteus: hierarquia, barramentos, geradores, terminais, instrumentos e modelos por perfis explícitos; mapa de perdas por objeto e fixtures autorizadas. Comparar netlists exportadas pelo original antes de liberar simulação; manter bloqueio até comprovar equivalência.
- [ ] Importação/exportação de formatos abertos mediante escopo e licença; preservar originais, unidades, IDs, nets e mapeamentos, com testes diferenciais.
- [ ] Ponte opcional entre MCP e desktop com autenticação/consentimento apropriados, revisão esperada e conflitos; hoje sessões são separadas.
- [ ] CLI/MCP: documentação de schemas, mensagens traduzidas, erros estruturados estáveis, limites e testes de compatibilidade; sem executar conteúdo não confiável.
- [ ] Clips de projeto e reutilização de blocos esquema–PCB; controlar colisões de referências, dependências de biblioteca e conectividade.

## P3 — Digital e firmware

- [ ] Selecionar uma arquitetura e emulador apenas após decisão explícita de licença, isolamento e escopo; especificar GPIO, temporizadores, interrupções e tempo virtual.
- [ ] Definir ponte digital/analógica, limiares, eventos, sincronização e falhas; testes determinísticos e limites de recursos para firmware não confiável.
- [ ] Debug, breakpoints, memória e periféricos suportados apenas por modelo comprovado; não anunciar suporte amplo a famílias MCU.

## Critérios gerais de aceite

Cada item deve identificar responsável, dependências, objetos afetados e decisão de contrato/formato quando necessária; implementar no núcleo antes da interface; incluir testes positivos, negativos e undo/redo aplicáveis; validar saída independente onde houver cálculo ou fabricação; registrar testes executados, licenças e limitações; verificar interface real quando alterada. Atualizar roadmap e documentação somente após comprovação. Funcionalidades não observadas diretamente devem ser tratadas como hipótese a confirmar, não como ausência definitiva.
