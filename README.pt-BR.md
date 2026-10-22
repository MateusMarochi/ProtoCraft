# ProtoCraft

<p align="center"><img src="assets/protocraft-logo.png" alt="Formiga sobre um microchip, marca do ProtoCraft" width="240"></p>

<p align="center"><a href="README.pt-BR.md">Português</a> · <a href="README.en.md">English</a></p>

Editor de circuitos eletrônicos para desktop, feito em Rust. Reúne editor de esquemas, motores próprios de simulação, roteamento manual de PCB em duas camadas, dados físicos de dispositivos STM32/ESP32, CLI e importador experimental de projetos Proteus.

> **Estado do projeto:** em desenvolvimento ativo. Consulte o README no seu idioma para instruções, recursos e limitações. A licença principal de distribuição ainda está pendente; leia [NOTICE.md](NOTICE.md) antes de reutilizar ou redistribuir o projeto.

## Destaques

- Editor esquemático com fios, rótulos de rede, ERC, desfazer/refazer e copiar/recortar/colar componentes.
- Três motores próprios de simulação, análise transiente, gráficos e exportação CSV.
- Editor PCB de duas camadas com trilhas manuais, vias, furos não metalizados e DRC.
- Exportação Gerber/Excellon com preset JLCPCB e validação pelo KiCad CLI externo.
- Catálogo físico offline com 2.283 entradas STM32 e 48 Espressif.
- CLI para validação, netlists, simulação, exportação SPICE, ações de projeto e importação Proteus.
- Formato nativo de projeto `.protocraft`.

## Início rápido

No Windows, execute `Iniciar-ProtoCraft.cmd`. Para compilar o código-fonte, instale a toolchain Rust indicada em `rust-toolchain.toml` e as ferramentas de build C++ MSVC, depois execute:

```powershell
cargo run -p eda-app
```

Consulte o [README em português](README.pt-BR.md) para instruções completas ou o [README em inglês](README.en.md).
