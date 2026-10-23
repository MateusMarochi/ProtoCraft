# Estado e próximas entregas

## Implementado na versão 0.1

- Workspace modular e núcleo sem GUI.
- Coordenadas inteiras, pinos definidos, IDs estáveis, comandos atômicos e undo/redo.
- Editor esquemático com componentes básicos, fios, junções, rótulos e propriedades.
- Derivação determinística de redes e ERC inicial.
- Persistência própria v1 (v5 atual), migração, backup e rejeição de arquivos inválidos/futuros.
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

## Autorroteador inicial no estado atual

- Motor original em Rust com busca octilinear A* em duas camadas, índice espacial de obstáculos, custo de curvas/vias/congestionamento e refinamento limitado.
- Planejamento validado por DRC e aplicado em lote atômico, com undo/redo no desktop e cópia de saída pela CLI.
- A compilação e o Clippy foram verificados; o comportamento em placas reais ainda precisa de validação prática.
- Rip-up de cobre manual, zonas, pares diferenciais, regras por classe de rede e roteamento multicamada seguem como trabalho futuro.

## Próxima fase: completar a bancada de circuitos

1. Seleção múltipla, roteamento que acompanha componentes, split/merge de fios e hierarquia de folhas.
2. Instrumentos com probes, vários gráficos, tolerâncias e passo adaptativo.
3. Backend ngspice opcional supervisionado e comparação independente dos casos analíticos; revisar distribuição e licença antes de empacotar.
4. Expandir definição de símbolos/modelos e biblioteca original versionada, com metadados de pinos e modelos genéricos documentados.
5. Condições iniciais analógicas editáveis, ampliação dos eventos digitais e AC, com novas referências analíticas.

## Fases posteriores ainda não implementadas

- PCB avançada: contornos não retangulares, zonas de cobre, regras por netclass, pares diferenciais, rip-up e reroute, tolerâncias, montagem, 3D e verificação fabril mais abrangente.
- Ampliação da simulação digital e primeira arquitetura MCU, com licença de emulador, tempo virtual, GPIO e testes determinísticos.
- Interoperabilidade: ampliar os parsers ISIS 8 restritos para outros perfis, tipos de objetos e formatos abertos, com validação elétrica independente. Não há compatibilidade geral com Proteus.
- 3D, autorroteamento, grandes bibliotecas, múltiplas folhas e integrações online conforme casos concretos.

Nenhum item futuro deve ser anunciado como funcional antes de implementação e validação.

