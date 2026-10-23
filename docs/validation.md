# Validação da entrega 0.1

Executada em 22/10/2026, Windows x64, Rust 1.99.1/MSVC, com ferramentas C++ do Visual Studio disponíveis na máquina. O Rust foi instalado pelo rustup oficial após conferir o checksum SHA-256 do instalador.

## Verificações automatizadas

Todos os comandos abaixo concluíram sem erro:

```text
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --workspace --locked
```

Foram executados **42 testes**, sem falha:

| Área | Quantidade | Cobertura |
|---|---:|---|
| Interface/geometria | 9 | Entrada SI, coordenadas/pinos, clique egui real por RawInput, giro/undo, alterações pendentes, worker e revisão obsoleta, escolha VOUT, proteção contra exportar sobre o projeto |
| CLI por subprocesso | 2 | Executável real: exemplos, validação, netlist, DC/transiente, CSV/SPICE e erros de entrada/argumentos |
| Domínio | 7 | Comando/lote inválido atômico, undo/redo, IDs, rotação, exclusão, referência duplicada e limites |
| Conectividade/ERC | 8 | Cruzamento sem junção, junção explícita, T, sobreposição, labels case-sensitive, terras, exemplos, mover/rotacionar e ausência de GND |
| Simulação | 12 | Divisor, Ohm, RC, RL, diodo, fontes de corrente, capacitor em DC, falhas de topologia, limites, exportação segura, determinismo, cancelamento e timeout |
| Armazenamento | 3 | Round-trip/backup, arquivo futuro/corrompido, valores inválidos e limite de tamanho |
| Integração armazenamento/simulação | 1 | Salvar, reabrir, simular divisor a 5 V e exportar CSV |

Os logs locais estão em `artifacts/final-tests.log` e `artifacts/final-package-validation.log`; essa pasta é ignorada pelo Git. Depois da otimização dos índices de netlist/ERC, os testes e Clippy foram executados novamente. Não houve comparação de desempenho com Proteus e não se afirma superioridade de velocidade ou consumo de memória.

## Aplicativo nativo

O executável release abriu a janela real e gerou capturas PNG pelo renderer em três cenários, encerrando com código 0: divisor DC no tema escuro, RC transiente no tema escuro e divisor DC no tema claro. As capturas foram inspecionadas e estão em `docs/screenshots/`. A revisão visual corrigiu glifos ausentes e contraste no tema claro. A captura RC usa o resultado numérico real e seleciona V(VOUT).

O teste de edição injeta eventos de ponteiro na mesma implementação egui e confirma posicionamento, giro e undo; ele não substitui uma sessão manual extensa de edição no Windows. Os diálogos nativos de abrir/salvar não foram operados por automação de mouse; os caminhos reais de arquivo e as regras de preservação foram exercitados pelos testes de armazenamento/CLI. Não houve validação em Linux ou macOS.

## Referência elétrica independente

O divisor, RC e diodo foram comparados com ngspice 47 oficial, instalado de modo portátil fora do produto após verificar SHA-256. O divisor coincidiu em 5 V/5 mA; o máximo desvio RC foi 18,340 mV (critério: 20 mV). O relatório [ngspice-validation.md](ngspice-validation.md) documenta a interpolação temporal, a diferença térmica do diodo e os logs. A referência analítica e as tolerâncias codificadas estão em [simulation-validation.md](simulation-validation.md).

O ngspice serviu de ferramenta externa de verificação e não é dependência de execução do aplicativo. Os testes cobrem circuitos pequenos definidos; não validam um simulador SPICE completo, modelos comerciais ou adequação física de um projeto.

## Entrega e limitações

`dist/ProtoCraft/` contém os executáveis portáteis `protocraft.exe` e `protocraft-cli.exe`, exemplos originais, documentação e avisos de terceiros. A fonte está no workspace da raiz. A aplicação suporta somente o formato `.protocraft` v1 e os componentes/análises documentados. PCB, firmware/MCU, lógica digital, AC e importação Proteus permanecem planejados. Naquela entrega 0.1, a licença principal ainda estava pendente; depois foi definida como MIT ou Apache-2.0, conforme `NOTICE.md`. Não houve publicação externa naquela validação.

# Validação da entrega 0.2

Executada em 22/10/2026, Windows x64, Rust 1.99.1/MSVC. Após integrar a identidade visual, o conversor experimental Proteus e os ajustes de ERC:

- `cargo fmt --all -- --check`, `cargo check --workspace --locked` e `cargo clippy --workspace --all-targets --locked -- -D warnings` passaram.
- `cargo test --workspace --locked` passou com 79 testes ativos, incluindo a importação CLI opt-in das seis amostras externas. O teste adicional `cargo test -p eda-import --test real_samples -- --ignored`, com `PROTOCRAFT_PROTEUS_SAMPLES` apontando para a pasta externa, passou e confirmou geometria, round-trip e numeração física dos pinos nos seis projetos. Total: 80 casos ativos aprovados; o único teste ignorado na suíte padrão é esta checagem explícita com as amostras externas.
- A nova regressão ERC confirma que uma conversão parcial mostra `INCOMPLETE_IMPORT` sem alegar ausência de terra, fonte ou circuito flutuante por causa de objetos possivelmente omitidos.
- O decoder da marca fornecida foi exercitado por teste e a imagem é incorporada ao executável. A janela nativa release foi capturada no exemplo RC importado e no somador completo reaberto em `.protocraft`; ambas as capturas foram inspecionadas. A marca aparece no cabeçalho. O somador inteiro cabe no canvas, e a aba Importação mostra os avisos persistidos. Capturas em `docs/screenshots/protocraft-proteus-rc.png` e `docs/screenshots/protocraft-proteus-somador.png`.
- A interface foi organizada em menus e comandos superiores, modos e biblioteca à esquerda, folha ao centro, Explorador à direita e painel de análise inferior. Os temas claro/escuro foram capturados e inspecionados em janela release, posicionada no monitor secundário (`DISPLAY1`, coordenadas negativas). As imagens atuais estão em `docs/screenshots/protocraft-interface-dark.png` e `docs/screenshots/protocraft-interface-light.png`.
- Ctrl+C/Ctrl+V copia e cola o componente selecionado; a cópia preserva valor, rotação e dados importados, recebe ID/referência novos, é selecionada após colar e participa do undo/redo. A colagem usa o cursor sobre o canvas ou desloca a cópia quando o cursor está fora dele.
- Após o ajuste dos rótulos da barra de modos, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` e `cargo build --release -p eda-app --locked` passaram. O teste externo opt-in das seis amostras Proteus foi executado e registrado acima; ele não foi necessário para esta alteração de interface.
- O ZIP `dist/ProtoCraft-0.2.0-windows-x64.zip` foi extraído em uma pasta de validação; os checksums dos dois executáveis conferiram, os arquivos da marca e da captura estavam presentes, `protocraft.exe --help` abriu e a CLI extraída importou o projeto RC original para um `.protocraft`, sem alterar o hash do `.pdsprj`.

A validação não implica compatibilidade geral com projetos Proteus nem autoriza simulação dos seis exemplos. Eles permanecem sinalizados como importações elétricas parciais; o bloqueio de simulação/SPICE é exercitado em testes.

# Validação da CLI headless

Executada em 22/10/2026, Windows x64. `cargo fmt --all -- --check`, `cargo check --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo test --workspace --locked` passaram. A suíte teve 82 testes ativos aprovados e um teste opt-in ignorado.

Os testes de subprocesso verificam `info` e `commands` em JSON, `run` tanto por arquivo de ações como por comandos diretos, persistência em um novo `.protocraft`, preservação da entrada e rejeição atômica de um lote inválido. `run` compartilha o `Engine`/`Command` do núcleo, limita arquivos JSON a 1 MiB e lotes a 1.000 ações. O CLI não abre janela para estes fluxos.

## Integração 0.3 — motores e modelos genéricos

Executada em 22/10/2026, Windows x64, Rust 1.99.1/MSVC. A suíte completa teve **100 testes aprovados** e um teste de amostras externas ignorado por depender de configuração opt-in. Formatação, `cargo check --workspace --locked`, Clippy de todos os alvos com avisos como erros e compilação release passaram.

Os testes novos abrangem backward Euler/trapezoidal contra RC e RLC analíticos, fontes periódicas, chave e potenciômetro, LED/Zener com KCL/potência, beta e polaridades NPN/PNP, operacional carregado e saturação, tabelas verdade, preservação de estado contínuo e limite de histórico. Os 11 exemplos foram validados por ERC e executados em DC/transiente nos dois motores. Há testes de seleção do motor pelo desktop/CLI, persistência e rejeição de configurações inválidas, `set_model` atômico/undo, migração v2→v3, duplo clique de chave no canvas e worker com pausa/avanço/revisão/cancelamento.

A GUI release foi aberta no monitor secundário, com posição negativa X e dimensões limitadas à sua área de trabalho. Foram capturados e inspecionados os temas claro e escuro, a janela de configurações, o NPN em DC e o RC senoidal em execução contínua. Os símbolos de onda usam primitivas vetoriais próprias, e os nomes de rede nos terminais são posicionados para fora do corpo dos componentes. Capturas: `protocraft-engines-dark.png`, `protocraft-models-light.png` e `protocraft-simulation-dark.png`, em `docs/screenshots/`.

A validação é numérica e funcional. Modelos genéricos não foram caracterizados contra dispositivos físicos nem contra todos os modelos comerciais. A comparação anterior com ngspice não estende automaticamente sua validade aos componentes novos. Fontes de ondas e lógica não implicam suporte a AC de pequenos sinais, MCU, parasitas ou propagação digital. As restrições completas e o estudo funcional estão em [simulation-engines.md](simulation-engines.md).

O pacote `dist/ProtoCraft-0.3.0-windows-x64.zip` foi extraído para validação. Os SHA-256 de `protocraft.exe` e `protocraft-cli.exe` coincidiram com `SHA256SUMS.txt`; a CLI extraída listou os dois motores, 26 tipos e 11 exemplos, validou o NPN e exportou CSV do RC usando o motor Dinâmico. Em t=1 ms, VOUT=6,321053213 V, contra 6,321205588 V da solução analítica (erro aproximado de 0,152 mV). O log da suíte final está em `artifacts/tests-0.3.0.log` e o CSV em `artifacts/rc-dynamic-0.3.csv`, ambos artefatos locais de validação.


## Integração 0.4 — motor Misto e componentes complexos

Executada em 22/10/2026 no Windows x64. `scripts/build.ps1` concluiu com código 0: formatação, check com lockfile, Clippy de todos os alvos sem avisos, testes e build release. A suíte teve **115 testes aprovados**, sem falhas, e um teste de amostras externas ignorado por exigir configuração opt-in. O log está em `artifacts/mixed-validation/build.log`.

A expansão entrega 36 tipos selecionáveis e 20 exemplos originais em `.protocraft` v4. Os testes verificam NMOS/PMOS em três regiões, fontes controladas, histerese, cruzamento Schmitt, divisão de clock, contadores módulo 16/10, atraso de propagação e filtragem inercial. O período do 555 astável e o pulso monoestável ficam a menos de 1,5% das equações RC nas condições documentadas. Foram conferidos reset prioritário, preset/clear assíncronos, saídas sem alimentação, correntes por terminal e retenção de eventos entre avanços. Estado inicial inválido e PREn/CLRn simultâneos são recusados.

O teste de armazenamento detectou perda de um ULP ao reabrir Is=1e-30 A com a configuração anterior do parser. Foi ativado `serde_json/float_roundtrip`, sem adicionar dependência. A regressão de migração v3→v4 e o round-trip completo do contador com LED passaram. A CLI real lista os três motores, simula o 555 em Misto e rejeita o motor incompatível. O teste egui confere a seleção automática de Misto ao abrir o contador, escolha do sinal Q0 e seleção de todos os 15 pinos nas quatro rotações.

A comparação externa com ngspice 47 passou em dez circuitos analógicos adicionais, com critérios de 10 nA para MOS e 1 nV para fontes controladas. As limitações da precisão impressa e do conjunto de casos estão em [ngspice-validation.md](ngspice-validation.md). Não foi executada comparação externa dos chips comportamentais nem caracterização física.

A aplicação release abriu no monitor secundário em posição X negativa. Capturas reais foram inspecionadas: `protocraft-mixed-555-dark.png` (555 oscilando em sessão contínua), `protocraft-mixed-counter-light.png` (contador em transiente) e `protocraft-mixed-settings.png` (configurações do motor). A revisão visual alinhou os nomes de rede com as linhas dos pinos e removeu valores numéricos sem uso dos símbolos dos chips. Não se trata de uma sessão manual extensa; os diálogos nativos não foram operados por mouse nesta entrega.

Os modelos são genéricos, originais e sem dependência de instalação do Proteus. Não há MCU/firmware, setup/hold, metastabilidade, parasitas nem equivalência integral com modelos comerciais. O [guia Misto](mixed-simulation.md) documenta pinagens, parâmetros, sincronização, exportação e limites.


O pacote portátil `dist/ProtoCraft-0.4.0-windows-x64.zip` foi extraído em uma pasta nova. Os checksums SHA-256 dos dois executáveis conferiram com `SHA256SUMS.txt`; a CLI extraída listou três motores, 36 componentes e 20 exemplos, simulou o 555 por 10 ms em Misto e inspecionou o contador com formato v4 e zero erros ERC. O relatório local é `artifacts/mixed-validation/portable-validation.json`. Os avisos de saídas sem carga do contador são preservados no ERC; não são tratados como erros.

Uma fixture adicional conectou diretamente a saída do 555 astável ao clock do contador binário, compartilhando alimentação e referência. Em 20 ms, com passo solicitado de 10 µs, foram observadas dez subidas do 555 e o contador terminou em dez, conforme esperado. Projeto, CSV e relatório estão em `artifacts/mixed-validation/555-counter.*`. Essa verificação local confirma a interação entre o oscilador RC analógico e o estado sequencial; não acrescenta um exemplo ao catálogo distribuído.


## Integração 0.5 — STM32/ESP32 e PCB

Executada em 22/10/2026 no Windows x64. O catálogo final tem **2.331 entradas** (2.283 STM32, 48 Espressif), 543 conjuntos de pinos e 162 footprints. A conversão mantém commits fixos e registra rejeições de mapeamento; módulos com antena integrada e conector externo receberam entradas físicas distintas quando a fonte agrupava os nomes. Os dados são fornecidos sob CC-BY-SA 4.0 com exceção KiCad, com avisos e atribuição em `assets/library/`.

A suíte completa passou com **132 testes**, sem falhas, e um caso externo opt-in ignorado. Formatação, check com lockfile e Clippy de todos os alvos com avisos como erros passaram. O log da compilação/verificação integrada está em `artifacts/pcb-validation/integration-0.5.log`. O teste do catálogo materializa e valida todas as entradas; os casos de referência conferem F103C8 LQFP-48 (pinos, corpo 7 × 7 mm, passo de 0,5 mm), ESP32-S3-WROOM-1 (41 terminais, corpo 18 × 25,5 mm e área de antena), variantes C6 MINI e BGA com 448 índices sem truncamento. Essas verificações são estruturais e amostrais; não representam conferência manual de todos os datasheets.

Os testes de PCB cobrem vínculo pino/pad/net, roteamento que resolve conexão pendente, curto/borda, footprint incompatível recusado atomicamente, duas faces, rotação, via entre camadas e exclusão de antena. Persistência preserva snapshots físicos e migra v4→v5. CLI real lista, cria projeto MCU, inspeciona mapas, edita pelo Engine, exporta e protege a entrada. Os testes egui arrastam um footprint por eventos reais de ponteiro, confirmam que o esquema não se move, exercitam undo e verificam sincronização preservando posições e cópia de pinagem/footprint.

### Referência externa KiCad

Foi obtido o instalador oficial KiCad 10.0.7 x64 de `github.com/KiCad/kicad-source-mirror/releases/download/10.0.7/kicad-10.0.7-x86_64.exe` (968.953.784 bytes). O SHA-256 coincidiu com o digest publicado pelo GitHub: `ce3881b6a9188eb34ac1ebaaec7651149e8a025206410addfb11a1a9656ecf95`. Authenticode retornou **Valid**, com assinante **KICAD SERVICES CORPORATION**. Apenas o CLI e seus recursos foram extraídos para uso portátil em `artifacts/kicad-tools/portable`; o instalador não foi executado. `kicad-cli --version` confirmou 10.0.7.

A placa autoral com STM32/ESP32-S3 foi exportada pelo ProtoCraft e carregada pelo KiCad CLI, que gerou SVG. O DRC dessa demonstração ainda sem roteamento informou 21 itens sem conexão e seis avisos (bibliotecas não configuradas e silk sobre máscara), preservados no relatório. Não foi tratada como placa concluída. Relatórios nativo e externo têm representações distintas de ilhas/itens e não se afirma igualdade de contagens.

Uma fixture autoral adicional de dois resistores 0805, três trechos de trilha e uma via foi construída por ações tipadas da CLI. O DRC nativo terminou sem erros/conexões pendentes; o fluxo `pcb-manufacture` passou pelo DRC externo de severidade error, que retornou zero violações e zero itens não conectados. Foram geradas nove camadas Gerber (extensões Protel), arquivo de furos Excellon com ferramenta de 0,3 mm, PCB intermediária, mapa de pinos, relatórios e logs. Evidências em `artifacts/pcb-validation/fabrication-final-0.5/`. O validador da saída foi corrigido para aceitar as extensões Protel padrão do KiCad, além de `.gbr`.

A geração exige pasta nova e DRC nativo aprovado; teste negativo confirmou preservação de arquivos existentes e bloqueio de uma PCB não roteada, antes de executar ferramenta externa. Nenhuma ferramenta ou arquivo KiCad externo é executado implicitamente ao abrir um projeto.


As janelas release de esquemático e PCB foram capturadas no monitor secundário, com posição X negativa. As imagens `protocraft-mcu-schematic-light.png`, `protocraft-mcu-pcb-dark.png` e `protocraft-mcu-pcb-light.png` foram inspecionadas. A revisão alinhou/simplificou rótulos de redes, evitou títulos dentro dos pinos, mostrou dimensões e distinguiu o projeto físico dos controles de simulação. Os testes de ponteiro complementam as capturas; não foi realizada uma sessão manual extensa com todos os modelos ou diálogos.

O DRC recebeu orçamento de geometria/conexões para rejeitar casos excessivos em vez de continuar uma análise sem limite. A regressão de sobreposição densa verifica esse erro. O projeto usa formato `.protocraft` v5 e mantém snapshots físicos; execução de firmware, PCB avançada e aprovação de fabricação permanecem fora do escopo implementado.

A revisão final incluiu o ESP32-WROOM-32E e variantes UE/R2: a notação agrupada de números do KiCad 10 é expandida em pinos físicos individuais. Um teste confere os quatro terminais GND 1/15/38/39 e 39 pinos totais; outro confirma que NC não cria rede fictícia, não aceita trilha e produz diagnóstico quando recebe conexão explícita. Os pinos de referência foram conferidos também no datasheet oficial Espressif WROOM-32E/32UE v2.1.

Os diagnósticos NC_CONNECTED e INCOMPLETE_IMPORT também são propagados ao DRC/fabricação, evitando produzir uma PCB a partir de ligação explícita a NC ou de importação elétrica parcial. As regressões exercitam os dois bloqueios.

O pacote portátil `dist/ProtoCraft-0.5.0-windows-x64.zip` foi extraído em uma pasta nova. Os SHA-256 dos dois executáveis e do catálogo conferiram; as licenças de dados estavam presentes. A CLI extraída confirmou 2.331 dispositivos (2.283 STM32 e 48 Espressif), gerou 21 exemplos, criou um projeto STM32F103C8T6 com 48 pinos e exportou sua PCB para KiCad. O relatório local está em `artifacts/pcb-validation/portable-validation.json`.

## Integração 0.5.1 — preset JLCPCB

Implementado em 22/10/2026. Passaram `cargo fmt --all -- --check`, `cargo check --workspace --offline`, `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` e `cargo build --release --workspace --locked --offline`. Não foram adicionados nem executados testes automatizados nesta alteração; a contagem de 132 testes acima pertence à entrega 0.5.

A CLI release gerou `artifacts/pcb-validation/jlcpcb-0.5.1/JLCPCB.zip` para a fixture roteada já existente (dois resistores, três trilhas e uma via). Os DRCs nativo e KiCad 10.0.7 retornaram zero erros/violações e zero conexões pendentes. O ZIP produzido contém 11 arquivos na raiz: nove Gerbers Protel e dois Excellon separados PTH/NPTH; a saída NPTH não contém furos nesta fixture. Não contém relatórios, projetos ou logs.

Os Gerbers usam RS-274X sem atributos X2, `%FSLAX46Y46*%` e `%MOMM*%`. O contorno exportado fecha o retângulo de 100 × 80 mm com coordenadas em escala 1:1. Os Excellon contêm `METRIC`, `G90` e declaração de formato absoluto/decimal. O manifesto `preset-jlcpcb.json`, fora do ZIP, registra as opções e arquivos. A exportação aplica verificação desses requisitos antes da compactação.

A geração foi local, sem upload ou pedido à JLCPCB. Não houve avaliação CAM pela fábrica ou fabricação física; o preset não define capacidades industriais nem substitui a revisão do projeto.

A GUI release foi aberta no monitor secundário. A captura `docs/screenshots/protocraft-jlcpcb-dark.png` foi inspecionada e mostra o preset e o botão de geração na seção Fabricação, visíveis no início do painel PCB. Os diálogos nativos de seleção da ferramenta/pasta não foram operados nesta conferência; a geração completa foi executada pela CLI com o mesmo serviço compartilhado.

## Integração 0.6 — trilhas manuais e furos

Implementação em 22/10/2026: ferramentas de trilha 45°/90°/livre, inversão de dobra, grade/largura configuráveis, encaixe em pads/vias/trilhas conforme a camada, via no cursor com troca de face, seleção/edição/arraste de geometria e furos mecânicos NPTH independentes. A conferência de uma edição usa o DRC e o Engine antes de confirmar; conexões pendentes permanecem visíveis. O domínio v6 acrescenta furos e comandos de atualização/exclusão com IDs estáveis. Não há autorroteamento ou movimento automático de trechos vizinhos.

Uma PCB de referência existente foi carregada de v5 e editada pela CLI com oito ações tipadas: dimensões 40 × 30 mm, largura de um trecho alterada para 0,3 mm, via alterada para cobre 0,8/furo 0,4 mm, quatro furos NPTH de 3,2 mm e reposicionamento de um deles. O projeto salvo em v6 está em `artifacts/manual-routing-0.6/placa.protocraft`; a fonte anterior foi preservada. O DRC nativo terminou sem diagnósticos/conexões pendentes; o DRC KiCad 10.0.7 retornou `violations: []` e `unconnected_items: []`.

O preset JLCPCB produziu nove Gerbers e Excellon PTH/NPTH em `artifacts/manual-routing-0.6/fabricacao/JLCPCB.zip`. O arquivo NPTH declara ferramenta `T1C3.200` e quatro posições: (4,26), (5,5), (36,4), (36,26) mm; o PTH declara `T1C0.400` para a via. Ambos usam mm e coordenadas absolutas; o sinal negativo de Y no Excellon corresponde à transformação compartilhada com os Gerbers. Não houve fabricação física ou envio à fábrica.

A edição revelou um problema no parser anterior de listas de ações: o buffer de um enum untagged perdia a conversão das chaves dos mapas de IDs de `set_board` preenchido. A leitura agora desserializa diretamente a lista ou o objeto `actions`, preservando validação e erros do JSON; o fluxo acima passou com posicionamentos/trilhas já existentes.

Não foram adicionados nem executados testes automatizados nesta alteração. A validação descrita corresponde à edição pela CLI e geração externa efetivamente observadas; os testes históricos da versão 0.5 não constituem uma nova execução da suíte para 0.6.

Uma segunda edição moveu o trecho central para B.Cu e acrescentou uma segunda via de 0,8/0,4 mm na outra extremidade, formando uma ligação real entre as duas faces e os pads SMD superiores. Os dois DRCs novamente terminaram sem erros/itens pendentes; o Excellon PTH passou a conter duas furações. Projeto e pacote estão em `artifacts/manual-routing-0.6/placa-duas-faces.protocraft` e `fabricacao-duas-faces/`. A demonstração foi distribuída como `docs/examples/pcb-trilhas-furos.protocraft`, com nome próprio e documentação de seu uso apenas como exemplo de PCB.

Passaram formatação, `cargo check --workspace --locked --offline`, Clippy de todos os alvos sem avisos e build release de todo o workspace. As janelas release foram abertas no monitor secundário e capturadas nos temas claro/escuro em `docs/screenshots/protocraft-trilhas-furos-*.png`. A inspeção visual confere controles, medidas, cores das trilhas e furos. Não foi realizada uma sessão manual extensa de traçado/arraste por mouse; as operações de domínio e fabricação descritas foram executadas pela CLI.

## Integração 0.6.1 — Ctrl+C / Ctrl+X / Ctrl+V

Executada em 22/10/2026. Corrigida a interpretação de eventos do clipboard: o backend egui-winit 0.33.3 produz `Event::Copy/Cut/Paste` e retorna antes de emitir a tecla C/X/V. O tratamento anterior escutava apenas teclas, não publicava o componente no clipboard do sistema e não implementava recorte. A PCB também recebia combinações com Ctrl como atalhos X/V de trilha/via. O controlador novo centraliza os eventos, remove duplicatas e deixa o editor de texto tratar seus próprios atalhos quando possui foco.

Foram acrescentados **14 testes de integração egui com RawInput**, passando por `shortcuts`, eventos nativos e saída `CopyText`: copiar/colar no esquema; recorte com ID/referência preservados; eventos nativos e teclas duplicados; clipboard inválido/comum/grande recusado sem colar dado antigo; componente MCU com snapshot de pinos/footprint; metadados e pinos importados; copiar/colar na PCB com símbolo isolado no esquema; recorte PCB com restauração do cobre; colisões de IDs de cobre; falha atômica de restauração; cursor em ambos os canvases; recorte esquemático conservando a posição física; troca de documento; foco real de TextEdit com copiar/recortar/colar texto. Os testes incluem undo/redo e seleção do resultado.

`scripts/build.ps1` concluiu com código zero: fmt/check, Clippy de todos os alvos sem avisos, **146 testes aprovados**, zero falhas, um teste externo opt-in ignorado, e build release de todo o workspace. O log está em `artifacts/clipboard-0.6.1/integration.log`. A suíte completa também reexecuta os testes de PCB, persistência, importação e simulação anteriores. O arquivo de projeto continua v6.

A versão release abriu a demonstração de PCB no monitor secundário e gerou `artifacts/clipboard-0.6.1/release-pcb.png`. A verificação dos atalhos é automatizada pelos eventos reais de egui e pela saída esperada para o backend; não foi feita injeção de teclas físicas em uma sessão manual do Windows. A documentação diferencia copiar de recortar e os efeitos sobre esquema/cobre em [clipboard.md](clipboard.md).

## CLI de criação e modelo 555 LED

Implementado em 23/10/2026. `cargo fmt --all`, `cargo check --workspace --locked`, Clippy do workspace com avisos como erros e `cargo build --release --workspace --locked` passaram. A suíte de testes não foi executada nesta alteração.

A CLI release listou o modelo `555-led-1hz`, criou `examples/555-led-1hz.protocraft` e a validação do esquema registrou nove componentes e zero erros/avisos ERC. A criação com `--frequency-hz 1` calculou C1=10,21 µF e reportou 1,00 Hz estimado. O modo Misto simulou três segundos, exportou CSV com 12.016 amostras e observou transições de subida separadas por 0,998358 s após o primeiro ciclo de partida. No ponto final, a corrente genérica do LED era 4,30 mA. Isso valida o modelo/simulador do ProtoCraft; não é caracterização de um NE555 ou LED comercial.

O fluxo também foi iniciado por `ProtoCraft-CLI.cmd --version`, que recompilou incrementalmente o workspace e respondeu `ProtoCraft 0.6.1`. O modo `new` agora pode criar folha vazia ou modelo sem substituir destinos existentes, `--open`/`open` inicia a GUI que acompanha a CLI, e `examples` interrompe antes de sobrescrever nomes em conflito. A GUI não foi aberta nesta validação; o teste de instalação visual permanece pendente.
