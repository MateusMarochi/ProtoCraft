# Arquitetura implementada

O workspace separa regras elétricas da interface e de I/O. Os arquivos `.protocraft` são dados próprios; nenhuma DLL do Proteus é usada na compilação ou execução.

| Crate | Responsabilidade | Dependências internas |
|---|---|---|
| eda-core | IDs, geometria inteira, componentes, projeto, comandos e undo/redo | nenhuma |
| eda-netlist | Derivação de redes e ERC | core |
| eda-simulation | MNA, controles de execução, resultados e exportação SPICE | core, netlist |
| eda-library | Catálogo físico offline normalizado, definições e proveniência | core |
| eda-pcb | Pads posicionados, vínculo às redes, conexões pendentes, DRC e exportação textual | core, netlist |
| eda-storage | JSON versionado, backup, CSV e fabricação externa supervisionada | core, simulation, pcb |
| eda-import | Contêiner Proteus, parsers CDB/DSN limitados e conversão com relatório | core, netlist |
| eda-app | Aplicativo desktop, ferramentas, renderer, estado transitório e apresentação | todos os módulos anteriores |
| eda-cli | Automação headless, inspeção JSON e ações de edição em lote pelo mesmo Engine usado na interface | core, netlist, simulation, storage |

O estado autoritativo fica no `Engine`. Cada edição cria um candidato, aplica o comando, valida o projeto inteiro e só então troca o snapshot atual. Falhas preservam o estado e o histórico. Undo/redo restaura snapshots com os mesmos IDs; até 100 edições anteriores são mantidas. A revisão aumenta também no undo/redo e identifica resultados numéricos que ficaram obsoletos.

Uma unidade inteira de coordenada corresponde a um passo de grade. Apenas o renderer converte para pixels. Fios são segmentos ortogonais com posições próprias. Pinos têm índice e coordenada explícitos, derivados de símbolo/rotação; o desenho usa o mesmo contrato. Mover/rotacionar altera os pinos, não estica os fios; portanto o ERC e a netlist refletem eventuais desconexões.

A netlist usa união de pontos elétricos, índices por linha/coluna e agrupamento determinístico. Pontos elétricos são pinos, extremidades de fios, junções e rótulos. Cruzamentos interiores sem esses pontos não conectam. Terras são reunidos em `NetId(0)`, nomes iguais conectam redes na folha e são case-sensitive. Todos os consumidores usam `build_netlist`.

O backend numérico original implementa `SimulationBackend`; `MnaBackend` recebe snapshot e `SimulationControl`. O resultado contém a revisão da entrada. A GUI executa o trabalho em thread separada e descarta resultados antigos ao trocar documentos. O solver não acessa GPU, janelas, arquivos ou rede. DC usa capacitores abertos e indutores ideais em curto; transiente oferece backward Euler e trapezoidal, selecionados por `SimulationEngine`. `TransientSession` conserva estado entre blocos limitados; a GUI usa um worker com canal limitado de resultados e descarta amostras obsoletas. Modelos analógicos genéricos ficam em `models.rs`; `mixed.rs` mantém estados e agenda de transições; `stepper.rs` sincroniza passos analógicos, bordas e limiares sem mutar estado nas tentativas de refinamento. A exportação fica em `spice.rs`. O motor Misto usa backward Euler e estágios de saída ligados às alimentações explícitas dos novos chips. Diodos são genéricos, com tolerâncias e limites documentados separadamente.

Decisão inicial: eframe/egui 0.33 com renderer OpenGL, evitando um renderer GPU próprio antes de medir necessidade. Foram escolhidos crates com licenças permissivas. O código-fonte original e a documentação do produto são licenciados sob MIT ou Apache-2.0, à escolha de quem recebe; os dados e assets de terceiros mantêm termos separados, descritos em [NOTICE.md](../NOTICE.md). Foi mantida uma versão de UI conhecida e compatível com a toolchain instalada, em vez de depender de mudanças recentes da API 0.37. O lockfile fixa o grafo efetivo.

Limites de escala: snapshots de edição não são uma solução final para projetos muito grandes; o esquema v1 representa uma única folha e componentes básicos. Conectividade e domínio precisam evoluir com testes de regressão antes de hierarquia e PCB avançada. `docs/roadmap.md` distingue o que existe dos módulos futuros.

O formato persistido v6 acrescenta furos NPTH; a v5 incorpora dispositivos/footprints/PCB em nanômetros; a v4 amplia os modelos e seus parâmetros; a v3 introduziu os parâmetros tipados; a v2 preservou a folha única e acrescentou metadados e pinos de importação. A conversão não usa proximidade nem arredondamento para ligar componentes: lê nós elétricos dos templates do DSN, aplica espelhos/rotações inteiras, preserva números do CDB e normaliza a grade por divisor comum. O renderer usa símbolos próprios. Parsers e conversor não executam modelos, firmware ou scripts. O estado parcial é persistido e bloqueia a simulação independentemente do caminho GUI/CLI.

A seleção do motor e as opções visuais ficam em configurações locais JSON gravadas atomicamente. Toda alteração de modelo passa por `Command::SetModel`, incluindo interações por duplo clique e automação CLI. O renderer usa a mesma geometria de pinos dos modelos, associa resultados à revisão atual e não calcula correntes de fio por aproximação.

A GUI mantém a PCB no mesmo projeto do esquemático; cada edição física usa Command e revisão. A fabricação roda em worker separado, com DRC nativo e processo KiCad CLI explicitamente configurado, limites de tempo e arquivos em pasta nova. O núcleo e eda-pcb não executam processos externos.

`eda-storage::fabrication::ManufacturingPreset` define a seleção comum à GUI/CLI. O preset JLCPCB aplica opções explícitas ao KiCad, confere os cabeçalhos e o contorno em escala real, e compacta somente Gerber/Excellon com a biblioteca ZIP já utilizada no workspace. A escolha do preset fica nas preferências do desktop; o projeto e suas regras de PCB não são alterados pela exportação.

O roteamento manual é separado da interface em `eda-pcb/routing.rs`: dobra definida pelo usuário, identificação de alvos na camada ativa e conferência da edição. O controlador desktop confirma lotes de segmentos/via pelo Engine, mantendo apenas a prévia e o histórico da rota em apresentação. O DRC de furação em `drilling.rs` inclui furos de footprints, vias e furos mecânicos independentes; as saídas KiCad/SVG/Excellon consomem os mesmos dados persistidos.

`eda-app/clipboard.rs` centraliza eventos nativos Copy/Cut/Paste e teclas de fallback, com deduplicação e respeito ao foco dos editores de texto. A saída CopyText e a solicitação RequestPaste passam pelo backend egui-winit para usar o clipboard do sistema. Payloads externos são limitados e validados; dados de recorte e sua identidade pertencem ao documento atual. Menus e atalhos usam os mesmos controladores e comandos do Engine.
