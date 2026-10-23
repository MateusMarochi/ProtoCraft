# Proveniência e dependências

O código do ProtoCraft foi implementado originalmente neste projeto. Os símbolos são desenhados por primitivas geométricas próprias. Os exemplos de circuitos são originais e usam relações elétricas públicas. Não há código, binário, modelo de componente, imagem ou biblioteca proprietária do Proteus incluídos.

A imagem `assets/protocraft-logo.png` foi fornecida pelo responsável pelo projeto para uso como identidade visual. Autoria e licença original não foram informadas; este registro não atribui direitos adicionais nem substitui a confirmação desses dados antes de redistribuição pública.

Dependências diretas adotadas:

| Dependência | Uso | Licença declarada pelo projeto |
|---|---|---|
| serde / serde_json | Serialização JSON | MIT OU Apache-2.0 |
| thiserror | Erros tipados | MIT OU Apache-2.0 |
| tempfile | Gravação atômica | MIT OU Apache-2.0 |
| egui / eframe 0.33 | Interface desktop e renderização | MIT OU Apache-2.0 |
| rfd 0.15 | Diálogos nativos de arquivos | MIT |
| png 0.18 | Capturas de validação do renderer | MIT OU Apache-2.0 |
| zip 9.0.5 | Leitura limitada do contêiner em memória | MIT |
| quick-xml 0.41 | Metadados XML do projeto importado | MIT |

O importador usa a série zip 9, com correções dos avisos RustSec RUSTSEC-2026-0367/0368/0369. A aplicação não chama APIs de extração para disco. Os arquivos de exemplo Proteus de terceiros permanecem externos e não são fixtures distribuídas; testes usuais usam dados sintéticos autorais. Importações locais geradas ficam em `imports/`, ignorada no Git e no pacote.

As dependências transitivas e versões exatas são registradas em `Cargo.lock`; o arquivo `docs/dependencies.json` gerado durante a entrega contém os metadados de licença e proveniência resolvidos pelo Cargo. Os textos de licença encontrados nos pacotes estão reunidos em `docs/THIRD_PARTY_LICENSES.txt`, incluindo OFL e UFL das fontes padrão de egui. Algumas dependências opcionais/plataformas podem constar no lockfile e no conjunto de avisos sem serem vinculadas ao executável Windows.

Fontes primárias consultadas: https://rust-lang.org/tools/install/ ; https://docs.rs/eframe/0.33.3/eframe/ ; https://docs.rs/rfd/0.15.4/rfd/ ; https://docs.rs/tempfile/3.27.0/tempfile/ . O Cargo verifica os checksums dos pacotes publicados.

Os materiais originais do ProtoCraft — incluindo código-fonte, documentação e exemplos de circuitos distribuídos com o projeto — são licenciados sob **MIT ou Apache-2.0, à escolha de quem recebe o material**. Consulte `LICENSE-MIT` e `LICENSE-APACHE`; o titular pode escolher qualquer uma dessas licenças para usar, modificar e redistribuir esses materiais. Copyright © 2026 Mateus Marochi e contribuidores do ProtoCraft.

Essa concessão cobre os materiais originais do ProtoCraft descritos acima. Ela não altera os termos independentes dos dados em `assets/library/`, das dependências e outros materiais de terceiros, nem estabelece uma licença para a imagem de marca cuja autoria e licença original não foram informadas acima. Circuitos e arquivos de projeto criados pelos usuários permanecem sob os termos de seus autores. Este aviso acompanha a distribuição; os textos das licenças de terceiros estão em `docs/THIRD_PARTY_LICENSES.txt` e nos diretórios correspondentes.

O estudo funcional da versão 0.3 consultou as instruções públicas e a interface documentada do CircuitJS. Os modelos e métodos Rust foram implementados neste projeto com equações públicas e exemplos próprios; nenhum código, asset ou runtime desse simulador foi incorporado. Referências e escopo estão em `docs/simulation-engines.md`.

O estudo funcional da versão 0.4 usa datasheets públicos TI NE555/SN74HC74 e equações públicas MOS de canal longo. Os modelos são genéricos e originais em Rust, com pinagens próprias documentadas em `docs/mixed-simulation.md`. Não há modelos de fabricante incorporados nem código da Labcenter. Nenhuma dependência nova foi adicionada; a opção `float_roundtrip` do serde_json já adotado garante a preservação dos valores f64 na persistência.


## Dados de biblioteca física — versão 0.5

`assets/library/` contém uma coleção adaptada de dados KiCad Libraries (KiCad Library Team e contribuidores) e Espressif KiCad Libraries (Espressif Systems e contribuidores), licenciada sob CC-BY-SA 4.0 com exceção KiCad. A coleção adaptada é redistribuída sob as mesmas condições, com avisos originais, atribuição, JSON legível, revisão e URL de cada origem. O conversor faz normalização de unidades, deduplicação de pads idênticos, separação de variantes físicas e seleção por correspondência pino/pad. Consulte `assets/library/README.md`, os dois LICENSE.md originais e `coverage.json`. Esta licença de dados é distinta da licença do código Rust e da licença de circuitos criados pelo usuário.

Nenhuma dependência Rust externa nova foi adicionada nesta entrega. O KiCad CLI 10.0.7 oficial foi usado como ferramenta externa de validação e para o fluxo opcional de fabricação; ele e suas bibliotecas binárias não são incorporados nem distribuídos no ZIP do ProtoCraft. Não é necessário para edição, persistência ou exportação textual KiCad/SVG.
