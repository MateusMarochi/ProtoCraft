# AGENTS.md — Projeto EDA em Rust (nome a definir)

> **Estado:** diretrizes iniciais propostas para um projeto novo. Antes de implementar, inspecione o repositório e adapte caminhos, comandos e decisões ainda não consolidadas à realidade do código. Nunca afirme que um módulo, teste ou recurso já existe sem verificá-lo.
>
> **Objetivo:** desenvolver um aplicativo de automação de projetos eletrônicos (EDA) inspirado nos fluxos de trabalho do Proteus: edição de esquemas, conectividade elétrica, simulação, instrumentos virtuais, projeto de PCB e, futuramente, co-simulação de firmware de microcontroladores. A implementação deve ser original e legalmente independente de código e recursos proprietários do Proteus.

## 1. Regra principal: comunicação em português do Brasil

**OBRIGATÓRIO para todos os agentes e subagentes.**

1. **Toda comunicação com o responsável pelo projeto será em português do Brasil (pt-BR)**: respostas, explicações, planejamento, relatórios, sugestões, mensagens de erro apresentadas ao usuário e resumos de tarefas.
2. Escreva documentação voltada à equipe e ao usuário em pt-BR, salvo exigência explícita de integração externa.
3. Textos visíveis na interface devem ser em pt-BR desde o início; prepare o sistema para internacionalização futura, evitando textos espalhados pelo código.
4. **Identificadores Rust** (nomes de crates, módulos, estruturas, funções, variáveis, traits e APIs) devem usar inglês técnico idiomático, seguindo as convenções de Rust. Ex.: `Schematic`, `NetId`, `build_netlist`, `SimulationBackend`.
5. Termos técnicos consagrados, como `netlist`, `SPICE`, `ERC`, `DRC`, `PCB`, `footprint`, `commit` e `crate`, podem continuar em inglês, com explicação em português quando necessário.
6. Comentários de código devem explicar o *porquê*, não repetir o *como*; prefira pt-BR em comentários explicativos, preservando nomenclatura de APIs e protocolos.
7. Nunca responda ao responsável pelo projeto em inglês, mesmo quando a fonte, a ferramenta ou outro agente usar inglês, salvo solicitação explícita.
8. Quando criar um subagente, inclua expressamente esta regra de comunicação nas instruções delegadas.

## 2. Visão do produto e limites

### 2.1 Objetivo de longo prazo

Um único aplicativo desktop de engenharia eletrônica que permita, progressivamente:

- Criar, editar e validar esquemas eletrônicos.
- Associar componentes, pinos, valores, símbolos, modelos e footprints.
- Extrair a conectividade elétrica em uma netlist confiável.
- Executar análises analógicas e visualizar resultados.
- Projetar placas PCB, verificar restrições e exportar arquivos de fabricação.
- Simular sinais digitais e, em fase posterior, executar firmware em dispositivos emulados compatíveis.
- Automatizar tarefas via comandos estruturados, CLI e, se aprovado, MCP.

### 2.2 Escopo do MVP (primeira entrega utilizável)

O MVP é **editor de esquemas + netlist + ERC básico + persistência + simulação inicial**. Deve incluir:

1. Projeto local com abrir, salvar, criar e recuperar erros de carregamento.
2. Canvas 2D com grade, pan, zoom, seleção e posicionamento.
3. Símbolos básicos: resistor, capacitor, indutor, diodo, fonte DC, GND e conectores simples.
4. Fios ortogonais, junções explícitas, nomes de redes e edição de propriedades.
5. Undo/redo, ações canceláveis e histórico coerente.
6. Extração determinística de netlist, com testes para cruzamentos e junções.
7. ERC inicial, com diagnósticos associados aos objetos envolvidos.
8. Análise DC básica usando um backend SPICE aprovado e exemplos verificáveis; AC/transiente entram na sequência conforme a capacidade real do backend.
9. Interface e mensagens em pt-BR.

**Fora do MVP:** autorroteamento profissional, 3D de PCB, simulação completa de microcontroladores, compatibilidade total com projetos Proteus, suporte irrestrito a modelos SPICE, colaboração online e recursos que dependam de contas ou serviços em nuvem.

**Nunca alegar equivalência integral ao Proteus.** Exibir limitações de recursos experimentais.

## 3. Princípios arquiteturais inegociáveis

1. **Engine-first:** o núcleo de domínio funciona sem interface gráfica, GPU, janelas ou ambiente desktop.
2. **Fonte única de verdade:** projeto, objetos, conexões e regras pertencem ao núcleo, nunca aos widgets da UI.
3. **Uma única derivação de conectividade:** editor, ERC, simulação e PCB utilizam a mesma netlist derivada pelo módulo responsável.
4. **Toda alteração é um comando tipado:** `Command -> validate -> apply -> result`. UI, testes, CLI e automação devem usar o mesmo fluxo.
5. **Atomicidade:** comando inválido não pode deixar o projeto parcialmente alterado. Lotes de comandos devem ser integralmente aceitos ou rejeitados.
6. **Undo/redo determinístico:** comandos de edição precisam poder ser revertidos e refeitos sem quebrar referências ou IDs.
7. **Separação entre dados e apresentação:** geometria elétrica, posicionamento visual e desenho/renderização não devem ser confundidos.
8. **Componentização proporcional:** adicionar crates apenas quando uma fronteira técnica justificar. Evitar tanto um único crate gigantesco quanto microcrates sem necessidade.
9. **Testabilidade sem GUI:** conectividade, serialização, verificadores e simuladores devem executar em testes headless.
10. **Precisão e rastreabilidade:** nenhum resultado numérico deve ser apresentado como correto sem critérios de tolerância, unidades e testes.
11. **Offline-first:** projetos locais e operações principais não devem exigir login nem internet.
12. **Compatibilidade evolutiva:** todo arquivo persistido tem versão e política explícita de migração.
13. **Sem promessas falsas:** um recurso planejado não é um recurso implementado; um teste não executado não está aprovado.

## 4. Estrutura inicial proposta do repositório

A estrutura abaixo é **alvo inicial**, não descrição de arquivos já existentes. Adaptar a nomes já estabelecidos no repositório antes de criar ou renomear qualquer coisa.

```text
/
├── AGENTS.md                  # Regras dos agentes
├── README.md                  # Visão geral, instalação e status real
├── Cargo.toml                 # Workspace
├── Cargo.lock                 # Versionado para a aplicação
├── rust-toolchain.toml        # Versão aprovada da toolchain
├── crates/
│   ├── eda-core/              # Projeto, símbolos, instâncias, IDs, comandos, undo/redo
│   ├── eda-netlist/           # Conectividade, geração de netlist e ERC inicial
│   ├── eda-simulation/        # Interface dos backends e adaptador SPICE
│   ├── eda-storage/           # Arquivos de projeto, serialização e migrações
│   └── eda-app/               # Desktop: UI, interação e apresentação
├── examples/                  # Circuitos de referência e projetos demonstrativos
├── tests/                     # Casos de integração e fixtures compartilhadas
├── docs/
│   ├── architecture.md        # Decisões e dependências efetivamente implementadas
│   ├── roadmap.md             # Fases, critérios e estado real
│   ├── decisions/             # ADRs para decisões arquiteturais relevantes
│   └── simulation-validation.md # Casos numéricos e tolerâncias, quando aplicável
└── .github/workflows/         # CI, quando o repositório usar GitHub
```

Crates **futuros**, criados somente quando necessários:

- `eda-pcb`: footprints, placa, trilhas, vias e DRC.
- `eda-mcu`: integração de emulação de CPU, firmware e periféricos.
- `eda-cli`: automação textual sem UI.
- `eda-renderer`: renderização especializada, caso a UI de fato precise dessa separação.
- `eda-library`: catálogo avançado, versionamento e importação de bibliotecas.

### 4.1 Direção permitida das dependências

- `eda-core` não depende de `eda-app`, GPU, `egui`, `eframe`, `wgpu` nem processos externos.
- `eda-netlist` depende apenas dos contratos necessários de `eda-core`.
- `eda-simulation` consome snapshots/netlists validados; não importa a interface desktop.
- `eda-storage` serializa dados públicos do modelo sem assumir detalhes de widgets.
- `eda-app` orquestra comandos e serviços. É uma camada externa, não o lugar das regras elétricas.
- `eda-pcb`, `eda-mcu` e `eda-cli` devem depender dos contratos do núcleo, nunca da UI.
- Evitar dependências circulares. Extrair contratos para um módulo compartilhado quando necessário; não inverter dependências para resolver rapidamente um erro de compilação.

### 4.2 Tecnologias candidatas, não decisões irreversíveis

- Linguagem principal: Rust estável, versão fixada em `rust-toolchain.toml` após aprovação.
- Interface desktop inicial: **egui + eframe**, por velocidade de iteração.
- GPU dedicada: **wgpu** apenas para necessidades medidas de renderização.
- Serialização: `serde`, com formato legível, estável e versionado.
- Erros: `thiserror` nas bibliotecas; tratamento contextual apropriado nas bordas.
- Diagnósticos: `tracing` com informações suficientes para depuração e sem dados sensíveis.
- Motor inicial de simulação: adaptador para **ngspice**, condicionado à licença, empacotamento, disponibilidade e segurança do processo.

**Política de dependências:** pesquisar manutenção, licença, compatibilidade, segurança e tamanho da dependência antes de adicioná-la; explicar ao responsável as trocas quando houver impacto relevante. Não introduzir versões, APIs ou recursos sem verificá-los.

## 5. Contratos essenciais de dados

### 5.1 Identidade e relacionamentos

- IDs estáveis e tipados para projetos, folhas, componentes, pinos, fios, junções, nets e, futuramente, footprints e pads.
- Não usar o nome exibido (`R1`, `U2`, etc.) como identificador interno definitivo.
- Alteração ou exclusão nunca deixa referências pendentes.
- Separar **definição de símbolo**, **instância no esquema**, **componente lógico**, **modelo de simulação** e **footprint físico**.
- Numerar e mapear pinos explicitamente. Nunca inferir a ligação entre pinos SPICE e pads de PCB somente pela ordem de renderização.
- Tipos de pino e metadados elétricos devem ser extensíveis para ERC e futuras regras de PCB.

### 5.2 Coordenadas e unidades

- Esquemático: posições em grade com representação inteira para conectividade exata; renderização pode utilizar `f32`/`f64` apenas na borda visual.
- PCB futura: usar unidades geométricas inteiras com precisão documentada (por exemplo, nanômetros em `i64`), sem comparar igualdade de posição por epsilon de ponto flutuante.
- Simulação: unidades SI coerentes; valores numéricos de componentes em `f64` finito e validado, sem aceitar `NaN`, infinito ou valores fisicamente inválidos.
- Não misturar coordenadas do mundo, pixels de tela, unidades de placa e grandezas elétricas no mesmo tipo sem conversões explícitas.

### 5.3 Conectividade: regras normativas

- Apenas o módulo de netlist determina quais pinos pertencem à mesma rede.
- Um cruzamento visual de dois fios **não** estabelece contato por si só.
- Uma junção explícita cria conexão; encontros em T ou extremidades coincidentes seguem regra documentada e coberta por testes.
- Nomes de nets conectam redes conforme política de escopo e normalização explicitamente testada; não assumir silenciosamente sensibilidade a maiúsculas/minúsculas.
- GND de simulação mapeia para o nó de referência elétrico apropriado do backend; não confundir qualquer símbolo decorativo de terra com conexão SPICE válida.
- Componentes sem aterramento necessário, pinos órfãos, referências duplicadas e nets inválidas produzem diagnósticos claros.
- Exportação, ERC, resultados de simulação e sincronização PCB devem consumir a mesma conectividade validada.
- Adicionar testes para arrastar/rotacionar/excluir componentes sem romper indevidamente ou preservar incorretamente conexões.

### 5.4 Motor de comandos e revisões

- Definir operações explícitas, como `PlaceComponent`, `MoveComponent`, `RotateComponent`, `ConnectPins`, `PlaceJunction`, `SetProperty`, `DeleteSelection`.
- Validar pré-condições, integridade referencial, limites e permissões antes de confirmar a mutação.
- Retornar erros tipados úteis, não valores silenciosos ou falhas genéricas.
- Manter número de revisão/snapshot para identificar resultados obsoletos de tarefas assíncronas.
- Edição, undo/redo e salvamento devem preservar invariantes do modelo.
- A UI nunca modifica os campos internos do projeto diretamente para contornar validação.

## 6. Simulação elétrica e numérica

1. O editor exporta uma **netlist validada** ao adaptador de simulação.
2. O backend é substituível por contrato/trait, sem afetar UI ou modelo elétrico.
3. Se usar `ngspice`, executar por processo supervisionado ou integração isolada e aprovada; verificar a instalação e informar quando o motor não estiver disponível.
4. Definir limites de tempo, tamanho de entrada, uso de recursos, mensagens de erro e cancelamento de simulações sempre que o backend permitir.
5. Não executar implicitamente scripts fornecidos por arquivos externos ou diretivas potencialmente perigosas. Restringir recursos como execução de comandos e inclusão arbitrária de arquivos; documentar política para `.include` e arquivos de modelos.
6. Não bloquear a thread da interface durante simulações; resultados devem estar vinculados à revisão do projeto que os originou.
7. Diferenciar **DC**, **AC** e **transiente**; não apresentar uma análise como suporte às outras.
8. Registrar unidades, tolerâncias numéricas, avisos de convergência e condições de contorno.
9. Tratar circuito singular, rede flutuante, ausência de referência, valores inválidos e não convergência como condições diagnosticáveis.
10. Nunca inventar modelos exatos de componentes comerciais nem sugerir que modelos genéricos representam necessariamente o dispositivo físico real.
11. A eventual implementação de solver próprio deverá começar por testes analíticos de MNA e só avançar para não linear/transiente após critérios verificáveis; não implementar solver SPICE completo como atalho para o MVP.

### Testes mínimos da simulação

- Divisor resistivo: fonte de 10 V, resistores de 1 kΩ + 1 kΩ -> nó intermediário de 5 V em DC, dentro da tolerância definida.
- Lei de Ohm para resistor isolado com fonte e referência corretamente conectadas.
- RC em degrau: `R = 1 kΩ` e `C = 1 µF`, constante de tempo teórica `τ = 1 ms`; ao medir transiente, comparar curva com solução analítica e tolerância documentada.
- Erro esperado para circuito sem referência exigida pelo backend e para malha singular.
- Cancelamento, timeout e resultado obsoleto, quando houver execução assíncrona.

## 7. Futuro editor de PCB

**Não implementar antecipadamente sem concluir as bases do esquemático e da netlist.** Ao iniciar o módulo:

- Manter vínculo explícito entre símbolo, componente, footprint, pad e net.
- A sincronização esquemático -> PCB preserva posicionamentos existentes sempre que possível.
- DRC deve identificar objetos, nets e regras violadas com precisão.
- Distinguir visualização de geometria real de fabricação (cobre, máscara, pasta, silk, furos, contorno, vias).
- Só exportar Gerber/Excellon com testes de geometria e verificação de arquivos por ferramenta independente.
- Nenhum autorroteador básico deve ser anunciado como roteamento profissional ou garantia de placa fabricável.
- A aplicação deve alertar que revisão elétrica, mecânica e de fabricação por profissional continua necessária.

## 8. Futura simulação de microcontroladores

A co-simulação de firmware **não faz parte do MVP**. Antes de implementá-la:

1. Escolher explicitamente a primeira arquitetura de CPU (por exemplo, AVR ou uma família ARM Cortex-M específica), considerando disponibilidade de emulador e licença.
2. Construir contrato entre estado do microcontrolador, GPIO, eventos digitais, tempo virtual e periféricos simulados.
3. Definir a ponte analógico/digital (limiares, níveis, amostragem, sincronização temporal).
4. Isolar carregamento e execução de firmware, com limites de recursos e sem conceder acesso não autorizado ao sistema operacional.
5. Testar ao menos GPIO, temporizadores, interrupções e cenários determinísticos pertinentes à arquitetura escolhida.
6. Identificar com precisão quais modelos e periféricos estão suportados; não anunciar suporte universal a Arduino, PIC, STM32 ou ESP32.

## 9. Interface e experiência do usuário

- Priorizar uma janela principal com áreas claras: esquema, biblioteca, propriedades, console/erros e gráficos de simulação.
- Manter estado do projeto separado do estado transitório da ferramenta/cursor.
- Implementar zoom centrado no cursor, pan previsível, snap configurável, seleção consistente e atalhos estáveis.
- Ações destrutivas devem ser reversíveis ou pedir confirmação quando não houver reversão.
- Suportar tema claro/escuro sem codificar cores como único indicador de erro.
- Apresentar resultados da simulação com unidades, legendas e nomes reais das nets.
- Planejar acessibilidade, comportamento em DPI alto e layouts redimensionáveis.
- Não sacrificar consistência elétrica para tornar uma interação visual aparentemente conveniente.
- Antes de declarar uma alteração visual concluída, testar manualmente ou por automação a interação real; registrar quando não for possível executar UI gráfica.

## 10. Persistência e bibliotecas

- Formato de projeto próprio com número de versão, identificadores estáveis e migrações controladas.
- Salvar de maneira atômica quando o sistema de arquivos permitir (arquivo temporário e substituição segura), com recuperação/backup conforme a plataforma.
- Nunca sobrescrever silenciosamente projetos inválidos, mais novos do que a versão suportada ou arquivos externos ao projeto.
- Round-trip obrigatório: salvar -> abrir -> preservar significado elétrico, IDs e relações.
- Separar biblioteca editável de definições incorporadas ao projeto para reprodutibilidade.
- Importar formatos externos apenas mediante escopo documentado, fixtures e análise de licença.
- Arquivos externos são entradas não confiáveis: impor limites de tamanho, profundidade, contagens e referências para evitar travamento ou consumo excessivo.

## 11. Padrões de Rust e qualidade do código

- Adotar a edição e toolchain definidas no repositório; não alterá-las sem justificar.
- Seguir `rustfmt`, Clippy e convenções idiomáticas.
- Evitar `unsafe`; qualquer exceção exige justificativa, escopo pequeno e testes próprios.
- Não usar `unwrap()`, `expect()`, `panic!()`, `todo!()`, `unimplemented!()` ou `unreachable!()` em fluxos de produção que possam receber entradas externas ou falhar em operação normal. Exceções só com prova do invariante e revisão.
- Preferir `Result` e erros tipados, preservando o contexto da falha nas bordas.
- Não esconder falhas com `let _ = ...`, defaults silenciosos ou conversões que percam dados.
- Evitar estado global mutável; preferir dependências explícitas e dados imutáveis quando possível.
- Evitar `clone()` indiscriminado de projetos inteiros em eventos de mouse; medir antes de otimizar e otimizar gargalos reais.
- Separar lógica pura de I/O, threads, GPU, sistema de arquivos e execução de programas externos.
- Não adicionar dependência apenas por conveniência se a biblioteca padrão resolver com clareza.
- APIs públicas precisam de tipos e contratos estáveis; usar documentação curta para invariantes não óbvios.
- Proibir segredos, tokens, credenciais, dados privados de usuários e binários proprietários no repositório.

## 12. Segurança e licenciamento

- Código, ícones, símbolos, footprints e conteúdos devem ter origem e licença verificáveis.
- Não copiar código-fonte, telas, marcas, bibliotecas de componentes ou arquivos proprietários do Proteus/Altium e apresentá-los como implementação própria.
- Não assumir que reescrever um algoritmo em Rust elimina obrigações de atribuição ou copyleft da fonte original.
- Registrar atribuições e licenças de recursos de terceiros conforme necessário em `NOTICE`/`ATTRIBUTION.md`.
- Antes de integrar código GPL, bibliotecas com licença não permissiva, modelos de fabricantes ou emuladores, comunicar o impacto e obter decisão explícita sobre a licença do produto.
- Tratar scripts, netlists, modelos SPICE, firmware e arquivos importados como não confiáveis.
- Nenhuma execução de código externo, acesso à rede, telemetria ou upload de projeto deve ocorrer sem propósito claro, autorização apropriada e opção visível quando aplicável.
- Não executar comandos destrutivos de Git/shell, apagar projetos, publicar releases ou enviar dados a terceiros sem autorização explícita.

## 13. Testes e comandos de validação

Quando o workspace Rust estiver criado, executar a partir da raiz, conforme a abrangência da mudança:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Para rodar a aplicação **depois de criado o pacote correspondente**:

```bash
cargo run -p eda-app
```

**Atenção:** nomes e comandos acima são metas para o scaffold proposto, não afirmações de que o repositório já funciona. Se o repositório usar outros nomes, descubra-os por `Cargo.toml` antes de executar.

Regras de testes:

1. Toda alteração de regra elétrica, comando, persistência ou simulação precisa de teste de regressão.
2. Testes unitários cobrem invariantes locais; testes de integração cobrem `comando -> netlist -> ERC -> simulação -> serialização` quando aplicável.
3. Usar fixtures pequenas e legíveis, incluindo casos negativos.
4. Comparar resultados numéricos com valores analíticos ou ferramentas independentes; fixar tolerâncias e unidades.
5. Testar serialize/deserialize e migração de versões.
6. Testar repetição de undo/redo, comandos inválidos, referências órfãs e lote parcialmente inválido.
7. Testes que exigem ngspice instalado devem ser distinguidos dos testes headless puros e documentar como habilitá-los.
8. Quando houver CI, incluir pelo menos Linux e Windows; macOS conforme suporte definido e infraestrutura disponível.
9. Não remover/desativar testes somente para tornar a pipeline verde.
10. Se ferramentas não estiverem disponíveis, dizer quais validações ficaram pendentes. **Nunca afirmar “testes passaram” sem execução efetiva.**

## 14. Fluxo de trabalho obrigatório para agentes

### 14.1 Antes de alterar

1. Ler este `AGENTS.md`, `README.md`, `Cargo.toml` e os documentos pertinentes **que existirem**.
2. Inspecionar árvore, APIs relacionadas, testes e estado atual do Git.
3. Identificar o módulo responsável e as invariantes que a mudança afeta.
4. Para tarefa não trivial, apresentar plano curto em pt-BR com objetivo, arquivos previstos e critério de conclusão; se o usuário pedir execução direta, não interromper por perguntas desnecessárias.
5. Não presumir que caminhos, comandos, versões ou funcionalidades planejadas já foram implementados.

### 14.2 Durante a implementação

1. Resolver uma entrega verificável por vez; preferir mudanças pequenas e revisáveis.
2. Implementar contratos e testes de núcleo antes de acoplar widgets gráficos.
3. Respeitar limites de cada crate; não duplicar regras de conectividade em UI, PCB ou simulação.
4. Se descobrir problema arquitetural, documentar a escolha e impacto antes de ampliar muito o escopo.
5. Não fazer refatorações não relacionadas que dificultem revisão.
6. Não substituir código funcional por mock silencioso nem entregar botões sem comportamento anunciado.
7. Atualizar documentação somente para o comportamento efetivamente existente.
8. Não instalar ferramenta, alterar versão de Rust ou adicionar dependência com impacto relevante sem justificar a necessidade.

### 14.3 Antes de concluir

1. Revisar o diff e verificar ausência de arquivos indesejados, credenciais e alterações fora do escopo.
2. Executar formatação, compilação, Clippy e testes cabíveis.
3. Validar critérios de aceitação da tarefa e relatar limites.
4. Em alterações de UI, testar a interface real quando o ambiente permitir.
5. Em alterações elétricas, demonstrar ao menos uma fixture de circuito relevante.
6. Responder em pt-BR com resumo, arquivos alterados, verificações realizadas, limitações e próximo passo sugerido.

**Modelo de resposta dos agentes:**

```text
Resumo: o que foi implementado e por quê.
Arquivos alterados: arquivos/crates efetivamente modificados.
Validação: comandos executados e resultados observados.
Limitações: pendências, riscos e testes não executados.
Próximo passo: uma recomendação objetiva.
```

## 15. Coordenação de múltiplos agentes

- Cada agente recebe tarefa com escopo, crate/arquivos de responsabilidade, entradas, saídas e testes de aceitação.
- Priorizar paralelismo por módulos independentes; não editar simultaneamente o mesmo arquivo sem coordenação explícita.
- Alterações de contratos compartilhados (`eda-core`, modelo, comando ou netlist) requerem acordo antes de agentes dependentes integrarem suas mudanças.
- Usar branches ou worktrees separados quando possível; integrar por etapas e com testes.
- Não sobrescrever mudanças locais do responsável pelo projeto nem de outro agente.
- Ao concluir, comunicar mudanças de API, dependências e invariantes afetados.
- Um agente integrador deve revisar conflitos e executar a suíte relevante; não presumir que testes isolados garantem integração.
- Ao delegar, incluir a regra de **comunicação obrigatória em português do Brasil**.
- Não criar mecanismos MCP, servidores de agente ou infraestrutura multiagente complexa até haver caso de uso concreto.

## 16. Prioridades e planejamento de entregas

**P0 — Fundação**

- Criar workspace Rust mínimo e aplicação que abre corretamente.
- Modelo de projeto com IDs estáveis, comandos tipados, salvar/abrir e testes de round-trip.
- Definir contrato de símbolos, pinos e coordenadas.
- **Pronto quando:** projeto de exemplo abre, salva e reabre sem perda; comandos inválidos não corrompem estado.

**P1 — Editor esquemático**

- Canvas, pan/zoom, grade, símbolos básicos, fios, junções, propriedades e undo/redo.
- Extrator único de conectividade + ERC básico.
- **Pronto quando:** circuito de exemplo pode ser desenhado, modificado, salvo e convertido em netlist consistente, incluindo testes de cruzamentos e junções.

**P2 — Simulação e instrumentos iniciais**

- Backend SPICE isolado, análise DC, resultados por nó, gráficos e diagnósticos.
- Estender para análises transientes e AC conforme backend aprovado.
- **Pronto quando:** divisor de tensão e RC reproduzem resultados analíticos dentro das tolerâncias e falhas são explicadas.

**P3 — PCB inicial**

- Vínculo símbolo/footprint/pads, placa básica, posicionamento e rotas manuais, DRC inicial.
- Exportação de fabricação somente após verificação independente dos arquivos.
- **Pronto quando:** um circuito simples percorre esquema -> placa -> verificações -> exportação validada, com limitações documentadas.

**P4 — Simulação digital e MCU**

- Escolher uma arquitetura concreta; integrar emulador e eventos digitais; sincronizar com o motor elétrico.
- **Pronto quando:** firmware de referência altera GPIO de forma determinística e os sinais são observáveis no esquema.

**P5 — Recursos avançados**

- Bibliotecas extensas, hierarquia, melhores instrumentos, roteamento mais sofisticado, 3D, interoperabilidade, plugins e automação MCP, sempre por demanda e com validação.

**Prioridade absoluta:** robustez da conectividade, correção dos dados e resultados verificáveis vêm antes de animações, efeitos visuais e quantidade de componentes.

## 17. Definição de pronto (Definition of Done)

Uma tarefa só pode ser marcada como concluída quando:

- [ ] O comportamento solicitado está implementado, não apenas representado visualmente.
- [ ] As invariantes do domínio continuam válidas.
- [ ] Há testes para comportamento novo e regressões relevantes.
- [ ] Verificações automatizadas cabíveis passaram, ou as impossibilidades foram declaradas.
- [ ] Erros ao usuário são claros e em pt-BR.
- [ ] Nenhuma outra camada passou a duplicar dados elétricos autoritativos.
- [ ] Não há dependências ou ativos novos sem análise de licença.
- [ ] Documentação e roadmap refletem somente o estado real.
- [ ] O relatório de entrega informa o que foi feito e o que não foi validado.

## 18. Limites de autonomia

### Pode fazer sem solicitar aprovação adicional

- Implementar correções e funcionalidades **dentro do escopo atribuído**.
- Criar testes, exemplos pequenos e documentação relacionada à tarefa.
- Executar validações locais não destrutivas.
- Fazer pequenas refatorações diretamente necessárias à mudança.

### Deve consultar o responsável antes de decidir

- Licença principal do produto e adoção de dependências com impacto de licença.
- Mudança significativa de arquitetura, formato persistido ou contratos compartilhados.
- Troca de framework de UI, motor de simulação ou toolchain com impacto amplo.
- Redefinição do MVP ou da ordem de grandes módulos.
- Inclusão de serviços online, telemetria, login ou execução arbitrária de código externo.

### Nunca fazer sem autorização explícita

- `git push`, `git push --force`, `git reset --hard`, limpeza destrutiva ou exclusão de trabalho alheio.
- Publicar versões, enviar dados do usuário ou conectar serviços externos em nome do responsável.
- Incorporar código ou ativos proprietários sem direitos de uso verificados.
- Declarar aprovação de engenharia, segurança elétrica ou fabricação sem evidência.
- Ocultar falhas de compilação, testes, convergência ou compatibilidade.

## 19. Referências técnicas para estudar (não copiar indiscriminadamente)

- Signex — https://github.com/alplabai/signex — organização por crates, comandos e netlist autoritativa.
- PCBCraft — https://github.com/tangency-labs/pcbcraft — comandos validados, GUI/CLI/MCP e instruções de agentes.
- CircuitCraft — https://github.com/Berserk-hub150/CircuitCraft — fluxo integrado de esquema, simulação via ngspice e PCB inicial.
- repath — https://github.com/Agustin-Delgado/repath — simulação analógica/digital e núcleo Rust independente.
- Spice21 — https://github.com/HW21/Spice21 — estudo de núcleo SPICE em Rust.
- RSpice — https://github.com/JaimeHW/RSpice — simulação e testes de conformidade; **verificar a licença de uso**.
- Padrão AGENTS.md — https://agents.md/ — convenção para instruções de agentes.

**As referências servem para estudar arquitetura e comportamento. Antes de reutilizar qualquer implementação, examinar sua licença e proveniência.**

---

**Regra de encerramento:** nenhum agente deve otimizar pela quantidade de código produzido. Priorize **software utilizável, modular, verificável, seguro e explicado em português do Brasil**.
