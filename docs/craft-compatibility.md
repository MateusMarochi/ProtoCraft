# Estudo PCBCraft e compatibilidade Craft

Revisão feita em 30/10/2026 dos repositórios públicos:

- PCBCraft: <https://github.com/tangency-labs/pcbcraft>, commit `31138579e5ce1c9d9ba4ee1f83f9253581c1c689` (12/10/2026).
- Craft libraries: <https://github.com/storytold/craft-libs>, commit `1aa81a773aa9198a0e4c0daeda954d622475af38`, fixado também em `Cargo.toml`/`Cargo.lock`.

PCBCraft separa geometria e documento, regras e I/O, motor de comandos, e superfícies MCP/UI. Seus contratos mais úteis são uma fronteira de mutação headless, lotes transacionais, revisão esperada, IDs estáveis, schemas descobríveis e uma UI fina que reutiliza `craft-ui`. O próprio README descreve o produto como uma fundação em desenvolvimento, com importação e recursos avançados ainda limitados.

## O que a revisão encontrou

- **Workbench:** uma janela com abas de esquema, PCB, bibliotecas e verificações; navegador e inspetor ancorados; tema escuro/claro, atalhos, zoom em torno do cursor e recuperação periódica.
- **Motor:** catálogo de comandos tipados, edição candidata validada, transações em lote, revisões esperadas, undo/redo limitado e IDs estáveis. A documentação deixa claro que erro não confirma estado parcial.
- **Automação:** 44 ferramentas MCP cobrem comandos, inspeção, bibliotecas, checagens, persistência, exportação e previews. Uma modalidade headless tem sessão própria; a modalidade desktop encaminha por ponte JSON-lines local.
- **Bibliotecas:** consulta a fontes instaladas do KiCad, preview geométrico, atribuição de footprint, unidades múltiplas de símbolo e editor de símbolos/footprints com biblioteca reutilizável. A própria PCBCraft ainda restringe vários formatos e primitivas.
- **PCB:** pads SMD/PTH com geometria, furos deslocados, contorno retangular/cutouts, trilhas ortogonais, conectividade por cobre, DRC inicial com IDs de objetos e exportação de um subconjunto KiCad. A documentação alerta que a rota inicial não evita obstáculos e que zonas, pares diferenciais e rip-up não estão prontos.
- **Distribuição:** CLI e aplicação desktop compartilham os crates; há um alvo Rust para navegador, ainda sem integração completa de arquivos do host.

Isso orienta a implementação para as partes transferíveis como contratos: sessão headless confiável, schemas navegáveis, revisão contra conflito, erros recuperáveis e uso das bibliotecas Craft permissivas. A geometria de PCB, simuladores, formatos, símbolos, imagens e regras de domínio continuam próprios do ProtoCraft.

## Licenças e proveniência

PCBCraft declara GNU GPL-3.0-or-later. O ProtoCraft usa MIT ou Apache-2.0. A licença da arquitetura não é uma dependência de código; por isso esta revisão adotou os contratos conceituais descritos publicamente e não copiou nem portou arquivos, símbolos, assets ou trechos da PCBCraft. Para UI compartilhada foi adotada a dependência independente `craft-ui`, de `craft-libs`, que declara MIT ou Apache-2.0. O commit exato e os avisos foram adicionados ao lockfile e aos arquivos de proveniência do ProtoCraft.

Isso mantém o código original do ProtoCraft sob seus termos atuais. Os dados físicos KiCad/Espressif continuam sob as licenças de dados existentes em `assets/library/` e `NOTICE.md`.

## Mapeamento implementado

| Contrato Craft observado | Implementação ProtoCraft |
|---|---|
| Motor headless como única fronteira de mutação | `eda-core::Engine` valida candidato e confirma comando/lote em uma operação |
| CLI e UI usam o mesmo motor | `eda-cli` e `eda-app` editam pelo `Engine`; `eda-automation` compartilha a tradução de JSON para comandos |
| Controle otimista de versão | `Engine::execute_at_revision` compara `expected_revision` antes de confirmar |
| Identidade de objeto estável | IDs continuam no documento, undo/redo os preserva e um limite monotônico evita reutilizá-los em novas ramificações |
| Catálogo de comandos | `eda-automation::COMMANDS` lista os comandos usados pela CLI e pelo MCP |
| Schema descobrível | `command_schema` gera schemas dos parâmetros e `batch_action_schema` publica as variantes no MCP |
| MCP headless por stdio | `eda-mcp` implementa discovery moderno, compatibilidade de handshake legado e inspeção/edição de projeto |
| Consulta ao catálogo e validações físicas | ferramentas MCP de busca facetada, DRC e autorroteamento chamam `eda-library`/`eda-pcb` |
| Biblioteca egui compartilhada | `craft-ui` fornece o fitting e interação das abas de análise e a paleta de estados de controles; tema e localização continuam no ProtoCraft |
| Workspace Craft atual | Rust edition 2024, Cargo resolver 3 e `eframe`/`egui` 0.36 compatíveis com o commit fixado de `craft-ui` |

## Diferenças que permanecem

Compatibilidade aqui significa encaixe nos limites e dependências da arquitetura Craft, sem substituir os módulos eletrônicos específicos do ProtoCraft. Ainda não existem:

- uma ponte MCP loopback para controlar o documento já aberto na interface; o servidor atual possui uma sessão headless independente;
- frontend WebAssembly ou hospedagem no navegador;
- edição de bibliotecas gráficas autorais de símbolos e footprints com salvamento em coleções reutilizáveis;
- a organização completa de workspace em abas de esquemático, PCB, biblioteca e verificações.

O formato de projeto segue sendo `.protocraft`. Não há promessa de ler ou gravar `.pcbcraft`, nem dependência de tipos ou crates internos da PCBCraft.

## Fontes

- PCBCraft README: <https://github.com/tangency-labs/pcbcraft>
- Arquitetura PCBCraft: <https://github.com/tangency-labs/pcbcraft/blob/main/docs/architecture.md>
- Roadmap PCBCraft: <https://github.com/tangency-labs/pcbcraft/blob/main/ROADMAP.md>
- Craft libraries e licenças: <https://github.com/storytold/craft-libs>
- `craft-ui` README: <https://github.com/storytold/craft-libs/blob/main/crates/ui/README.md>
