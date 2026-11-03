# Arquitetura do ProtoCraft

O ProtoCraft usa camadas Rust com um motor headless como autoridade sobre o documento. Interface desktop, CLI e servidor MCP enviam comandos ao mesmo `Engine`; eles não mantêm cópias independentes das regras de edição. O projeto segue os limites arquiteturais do ecossistema Craft e usa componentes compartilhados de `craft-ui` sem incorporar código da PCBCraft.

## Camadas e crates

| Camada | Crates | Responsabilidade |
|---|---|---|
| L0 — modelo e geometria | `eda-core` | IDs estáveis, unidades, documentos, comandos tipados, validação, transações, revisões e histórico |
| L1 — definições e entrada | `eda-library`, `eda-import` | Catálogo físico offline; leitura e conversão limitada de formatos externos |
| L2 — regras e cálculos | `eda-netlist`, `eda-pcb`, `eda-simulation` | Conectividade/ERC, PCB/DRC/roteamento e simulação; sem acesso à janela |
| L3 — persistência e integração | `eda-storage` | Formato `.protocraft`, backup, CSV e fluxo externo de fabricação supervisionado |
| L4 — superfícies | `eda-automation`, `eda-mcp`, `eda-cli`, `eda-app` | Mapeamento de comandos, protocolo MCP, terminal e interface desktop |

O formato nativo versionado é o contrato compartilhado entre o editor e a CLI. A biblioteca física e as regras geométricas de PCB usam nanômetros; o esquemático usa coordenadas inteiras da grade. A conversão para pixels existe somente na camada de apresentação.

## Motor, comandos e histórico

O `Engine` é o único ponto que confirma mutações. Ele clona o documento candidato, aplica um comando ou lote, valida as invariantes e só então atualiza o snapshot e a revisão. Um erro descarta o candidato sem alterar o projeto ou o histórico. CLI e MCP reutilizam `eda-automation::Action` para converter JSON em comandos tipados; a interface chama o mesmo `Engine`.

As edições aceitam uma revisão esperada para recusar mutações feitas sobre dados desatualizados. Undo e redo também avançam a revisão. O alocador mantém um limite superior monotônico de IDs, de modo que ramificações depois de undo não reutilizem IDs já emitidos. O histórico guarda até 20 snapshots; o limite ainda não é configurável.

`eda-automation` publica o catálogo de comandos, valida objetos JSON sem campos desconhecidos e fornece JSON Schema por comando, além do esquema discriminado para lotes. A validação final do documento continua no `Engine`; o schema ajuda clientes a formar chamadas válidas sem substituir as regras de domínio.

## CLI e MCP

`eda-cli` oferece fluxos headless para criação, inspeção, edição, validação, simulação, exportação e importação. O subcomando `mcp --project arquivo.protocraft` inicia `eda-mcp` por stdio. O servidor mantém um único projeto headless no processo e disponibiliza inspeção, validação, netlist/ERC, busca facetada do catálogo físico, DRC, autorroteamento, comandos em lote, undo/redo e save.

O MCP moderno anuncia versão e capacidades com `server/discover`; cada chamada de protocolo moderno valida os metadados exigidos. A camada legada também aceita o handshake MCP `initialize` de 2025-11-25. Edições usam `expected_revision`, lotes são atômicos e salvar é explícito. O servidor trabalha com uma sessão própria do arquivo indicado; ainda não há uma ponte para controlar a instância desktop em execução.

## Interface e Craft UI

A interface desktop é uma camada egui/eframe fina que apresenta o documento e delega edições ao motor. O workspace usa Rust edition 2024 e Cargo resolver 3. `eframe`/`egui` foram atualizados para a série 0.36 exigida pela revisão de `craft-ui` fixada no `Cargo.lock`.

O ProtoCraft integra `craft-ui` para ajuste de abas e estilos de estados de controles. A aplicação continua dona das cores completas, ícones, textos, localização e execução de comandos; esses dados não são trazidos da biblioteca compartilhada. A localização de interface e automação usa `assets/i18n/en.json` e `assets/i18n/pt-BR.json`.

O editor segue em uma superfície desktop; separar esquemático e PCB como abas do workspace e consolidar bibliotecas/diagnósticos são etapas da interface. A arquitetura por crates não exige que o motor seja carregado pela UI e deixa CLI/MCP operarem sem renderer.

## Limites atuais

- O MCP é headless, sem ponte para o projeto aberto no desktop.
- Não há frontend WebAssembly.
- A importação Proteus é parcial e bloqueia simulações com conectividade incompleta.
- Hierarquia de esquemas, zonas, pares diferenciais, modelos MCU e simuladores externos completos seguem fora do escopo implementado.
- Snapshots inteiros simplificam transações; projetos muito grandes podem exigir histórico incremental.

## Referência arquitetural e licenças

A arquitetura pública da PCBCraft ajudou a identificar contratos úteis: motor headless como fronteira de mutação, IDs estáveis, lotes atômicos, revisão esperada e UI compartilhando uma biblioteca permissiva. O código e assets GPL da PCBCraft não foram incorporados. O código original do ProtoCraft mantém MIT ou Apache-2.0; `craft-ui` é uma dependência independente MIT ou Apache-2.0, fixada por commit e registrada em [NOTICE.md](../NOTICE.md) e [THIRD_PARTY_LICENSES.txt](THIRD_PARTY_LICENSES.txt).
