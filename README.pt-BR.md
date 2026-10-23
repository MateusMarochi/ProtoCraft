# ProtoCraft

<p align="center"><img src="assets/protocraft-logo.png" alt="Formiga sobre um microchip, marca do ProtoCraft" width="240"></p>

<p align="center"><a href="README.pt-BR.md">Português</a> · <a href="README.en.md">English</a></p>

Editor de circuitos eletrônicos para desktop, feito em Rust. Reúne editor de esquemas, motores próprios de simulação, roteamento manual e autorroteamento inicial de PCB em duas camadas, dados físicos de dispositivos STM32/ESP32, CLI e importador experimental de projetos Proteus.

> **Estado do projeto:** em desenvolvimento ativo. O código-fonte, a documentação e os exemplos de circuitos originais distribuídos com o ProtoCraft são licenciados sob MIT ou Apache-2.0, à sua escolha. Dados e assets de terceiros têm termos próprios; consulte [NOTICE.md](NOTICE.md), [LICENSE-MIT](LICENSE-MIT) e [LICENSE-APACHE](LICENSE-APACHE).

## Destaques

- Editor esquemático com fios, rótulos de rede, ERC, desfazer/refazer e copiar/recortar/colar componentes.
- Três motores próprios de simulação, análise transiente, gráficos e exportação CSV.
- Editor PCB de duas camadas com trilhas manuais, autorroteador A* inicial, vias, furos não metalizados e DRC.
- Exportação Gerber/Excellon com preset JLCPCB e validação pelo KiCad CLI externo.
- Catálogo físico offline com 2.283 entradas STM32 e 48 Espressif.
- CLI para validação, netlists, simulação, exportação SPICE, autorroteamento, ações de projeto e importação Proteus.
- Formato nativo de projeto `.protocraft`.

## Início rápido

No Windows, execute `Iniciar-ProtoCraft.cmd`. Para compilar o código-fonte, instale a toolchain Rust indicada em `rust-toolchain.toml` e as ferramentas de build C++ MSVC, depois execute:

```powershell
cargo run -p eda-app
```

Consulte o [README em português](README.pt-BR.md) para instruções completas ou o [README em inglês](README.en.md).

## Criar e simular o exemplo 555 de 1 Hz

Na raiz do repositório, o atalho `ProtoCraft-CLI.cmd` compila a CLI e permite começar um circuito a partir de um modelo:

```powershell
ProtoCraft-CLI.cmd templates
ProtoCraft-CLI.cmd new --template 555-led-1hz --frequency-hz 1 --output pisca.protocraft --open
ProtoCraft-CLI.cmd simulate pisca.protocraft --engine mixed --transient 0.00025 3 --csv pisca.csv
```

O projeto inicial usa RA=47 kΩ, RB=47 kΩ, LED com resistor limitador de 680 Ω e capacitor de bypass de 10 nF. O comando calcula C1 para a frequência informada, grava o `.protocraft`, abre o editor e permite salvar os sinais da simulação em CSV. Use `--c-farads` no lugar de `--frequency-hz` para definir diretamente o capacitor. Consulte o [guia da CLI](docs/cli.md) para criar projetos vazios, listar os outros modelos e ver os limites.
