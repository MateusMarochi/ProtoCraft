# Estado e próximas entregas

## Implementado na versão 0.1

- Workspace modular e núcleo sem GUI.
- Coordenadas inteiras, pinos definidos, IDs estáveis, comandos atômicos e undo/redo.
- Editor esquemático com componentes básicos, fios, junções, rótulos e propriedades.
- Derivação determinística de redes e ERC inicial.
- Persistência própria v1 (v7 atual), migração, backup e rejeição de arquivos inválidos/futuros.
- MNA original para RLC/fontes DC/diodo genérico, DC e transiente, limites e testes analíticos.
- Apresentação dos resultados, gráficos, CSV e exportação SPICE.
- CLI e exemplos originais.

O registro `validation.md` informa o que foi efetivamente executado e observado na entrega.

## Implementado na versão 0.2

- Importação experimental `.pdsprj` perfil 800 e `.dsn`, por GUI e CLI.
- Parsers limitados de CDB e DSN; posições de pinos, fios, junções, labels e terra.
- Preservação de dispositivos sem modelo com símbolo genérico próprio e metadados.
- Formato `.protocraft` v2 com migração v1 e avisos persistentes.
- Conversão parcial explícita, bloqueio de simulação e regressão dos seis projetos externos.

Próxima prioridade de importação: geradores, outros terminais/probes, reconciliação completa com CDB, múltiplas folhas e validação contra netlist exportada pelo Proteus. A liberação de simulação exige confirmação dessa equivalência.

## Implementado na versão 0.3

- Motores próprios Estável/Dinâmico selecionáveis em configurações persistentes e CLI.
- Sessão contínua com estado de capacitores/indutores, pausa, avanço manual e alterações interativas.
- Fontes senoidal/quadrada/triangular/serra, chave, potenciômetro, LED/Zener, NPN/PNP, operacional, entrada lógica e seis portas.
- Tensão por cores, corrente/potência por componente, animação e brilho do LED.
- Formato v3 com parâmetros tipados e migração v1/v2; comando atômico `set_model`.
- Onze exemplos próprios e testes analíticos, tabelas verdade e interação de canvas.

## Implementado na versão 0.4

- Motor Misto com eventos determinísticos, atraso inercial e refinamento de limiares.
- MOSFET N/P, comparador, Schmitt, 555, flip-flop D, contadores binário/década e fontes controladas.
- Alimentação explícita dos novos chips e resultados de corrente por terminal/estado digital.
- Vinte exemplos próprios, 36 tipos selecionáveis e formato v4 com migração v1/v2/v3.
- Exportação SPICE MOS Level 1 e fontes E/G; chips sem equivalente são recusados.

## Implementado na versão 0.5

- Catálogo físico STM32/ESP32 com snapshots de pinos, encapsulamentos, footprint e proveniência.
- PinRef de 16 bits, mapeamento pino → pad validado e arquivo v5 com migração v1–v4.
- Editor PCB inicial de duas camadas, escala física, duas faces, trilhas, vias e sincronização que preserva posições.
- DRC de cobre/bordas/áreas de montagem/exclusões e conexões pendentes.
- Exportação KiCad/SVG e fabricação Gerber/Excellon via KiCad CLI explícito, com validação prévia.
- GUI/CLI pelo mesmo Engine; testes estruturais do catálogo, geometria, persistência e interação.

## Implementado na versão 0.6

- Roteamento manual 45°/90°/livre, prévia, inversão da dobra e encaixe em pads/trilhas/vias da camada ativa.
- Largura/grade configuráveis, via configurável no cursor e troca de camada durante o traçado.
- Seleção, edição, arraste e exclusão de trechos, vias e furos; undo/redo pelo Engine.
- Furos mecânicos NPTH independentes, formato v6 e migração v1–v5.
- DRC de furação/cobre/borda e confirmação de edições sem novas violações físicas.
- Exportação KiCad/SVG e Excellon NPTH/PTH integrada ao preset JLCPCB.

## Implementado na versão 0.6.1

- Ctrl+C/Ctrl+X/Ctrl+V por eventos nativos e clipboard do sistema, com os mesmos controles nos menus.
- Recorte local mantém identidade; cópias recebem novo ID e referência; foco de texto e ferramentas PCB não conflitam com os atalhos.
- Fluxos de esquema/PCB, preservação de snapshots e restauração do cobre cobertos por 14 testes egui adicionais; suíte completa reexecutada.

## Interface da versão 0.7.0

- Página inicial com abertura de projetos recentes e acesso aos fluxos de captura, simulação e PCB.
- Organização de menus, abas, ferramentas e painéis aproximada do fluxo de trabalho de CAD eletrônico, com temas claro e escuro.
- Marca renderizada com mipmaps para reduzir serrilhamento e perda de forma em tamanhos pequenos.

## Próxima fase: arquitetura Craft e automação

- Camada headless, UI e CLI usam o mesmo `Engine`; comandos em lote são validados e confirmados atomicamente.
- Revisão esperada protege edições automatizadas; undo/redo avançam a revisão e a alocação de IDs não volta ao desfazer.
- `eda-automation` publica esquemas JSON por comando; `eda-mcp` oferece sessão stdio com inspeção, validação, busca física, DRC e autorroteamento.
- `craft-ui` está fixado em revisão MIT/Apache compatível com egui 0.36; os avisos independentes foram adicionados.
- A sessão MCP é headless e separada do projeto aberto no desktop. Ponte para a UI e frontend WebAssembly seguem pendentes.

## Autorroteador inicial no estado atual

- Motor original em Rust com busca octilinear A* em duas camadas, índice espacial de obstáculos, custo de curvas/vias/congestionamento e refinamento limitado.
- Planejamento validado por DRC e aplicado em lote atômico, com undo/redo no desktop e cópia de saída pela CLI.
- A compilação e o Clippy foram verificados; o comportamento em placas reais ainda precisa de validação prática.
- Zonas, regras avançadas por camada/região, pares diferenciais, rip-up e reroute, além de roteamento multicamada, seguem como trabalho futuro. Classes nomeadas iniciais já controlam folga, largura e tamanho de via.

## Próxima fase: completar a bancada de circuitos

1. Seleção múltipla, roteamento que acompanha componentes, split/merge de fios e hierarquia de folhas.
2. Instrumentos com probes, vários gráficos, tolerâncias e passo adaptativo.
3. Backend ngspice opcional supervisionado e comparação independente dos casos analíticos; revisar distribuição e licença antes de empacotar.
4. Expandir definição de símbolos/modelos e biblioteca original versionada, com metadados de pinos e modelos genéricos documentados.
5. Condições iniciais analógicas editáveis, ampliação dos eventos digitais e AC, com novas referências analíticas.

## Fases posteriores ainda não implementadas

- PCB avançada: contornos arbitrários, zonas de cobre, regras por região/camada, pares diferenciais, rip-up e reroute, tolerâncias, montagem, 3D e verificação fabril mais abrangente. Classes por nome de rede são um primeiro subconjunto implementado.
- Ampliação da simulação digital e primeira arquitetura MCU, com licença de emulador, tempo virtual, GPIO e testes determinísticos.
- Interoperabilidade: ampliar os parsers ISIS 8 restritos para outros perfis, tipos de objetos e formatos abertos, com validação elétrica independente. Não há compatibilidade geral com Proteus.
- 3D, roteamento avançado, grandes bibliotecas, múltiplas folhas e integrações online conforme casos concretos.

Nenhum item futuro deve ser anunciado como funcional antes de implementação e validação.


## Entrega de PCB — 03/11/2026

Executado o ciclo inicial do [plano de PCB](plano-pcb.md), com detalhes de uso em [pcb-workflows.md](pcb-workflows.md):

- Sincronização headless de footprints ausentes, compartilhada com o desktop, preservando posições e cobre; recusa por falta de espaço.
- Translação conservadora de componentes acompanhando extremidades de trilhas, via CLI e arraste no desktop; contatos ambíguos, vias e ramificações recusados.
- Remoção reversível de trilhas exatamente duplicadas, pelo Engine, desktop e CLI.
- BOM por dispositivo/valor/unidade/footprint, posições na convenção do domínio e listas explícitas de peças incompletas; CSV/JSON e relatórios externos ao ZIP de fabricação.
- Comprimento geométrico por rede e face, sem inferir impedância ou atraso.
- Correções de DRC para contato de cobre/furos com folga zero e regressões de camadas e tolerâncias.
- CLI com preferência de idioma por processo e testes isolados nos dois idiomas.

Permanecem pendentes zonas de cobre, contornos arbitrários, regras hierárquicas por região/camada, multicamadas, rotação conectada, atualização avançada de footprints e roteamento profissional. Classes nomeadas básicas já se aplicam ao DRC e ao autorroteador. As funcionalidades acima não concluem os itens abrangentes do backlog.


## Avanços headless de 05/11/2026

Variantes de montagem DNP, varredura DC de resistor/fonte e medições de séries temporais/diferenciais estão implementadas em serviços de núcleo e CLI. DFM considera rasgos orientados como cápsulas e um limite independente entre furações. Os novos relatórios usam publicação atômica sem sobrescrita. Exemplos, critérios e limitações: [advanced-workflows.md](advanced-workflows.md). Zonas, multicamadas, pares diferenciais, AC, passo adaptativo, firmware e 3D continuam pendentes.

## Implementado no ciclo 0.11.0

- Importação Falstad/CircuitJS via desktop e CLI para texto legado, XML, atalhos `.url` e links suportados; vários semicondutores/chaves são simuláveis por modelos genéricos. O relatório identifica aproximações e bloqueia recursos sem suporte.
- Relatórios de medição transiente, incluindo medidas RMS/janela e diferenças entre traces, além de varredura DC linear/log de uma fonte DC ou resistor. A alteração dos pontos usa snapshots do Engine e não modifica o original.
- Variantes DNP para relatórios de montagem; essas variantes filtram a lista, sem reconciliar conectividade elétrica.
- Limites DFM configuráveis, diagnósticos de furação/folgas e relatórios CLI; a verificação continua geométrica e não certifica fabricação.

O formato nativo atual é v7. Permanecem pendentes os itens estruturais abaixo, incluindo hierarquia de esquemas, zonas de cobre, regras por região/camada, roteamento profissional e equivalência geral com Proteus/Falstad.