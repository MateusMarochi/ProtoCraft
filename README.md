# ProtoCraft

<p align="center"><img src="assets/protocraft-logo.png" alt="ProtoCraft ant on a microchip" width="240"></p>

<p align="center"><strong>Rust EDA for schematic capture, circuit simulation, PCB layout, and manufacturing-file export.</strong></p>

<p align="center"><a href="README.pt-BR.md">Português</a> · <a href="README.en.md">English</a></p>

ProtoCraft is a desktop electronics design application built in Rust. It combines a schematic editor, built-in circuit simulation engines, manual and initial automatic two-layer PCB routing, physical STM32/ESP32 device data, a headless CLI, and an experimental Proteus project importer.

> **Project status:** active development. Original ProtoCraft source code, documentation, and bundled circuit examples are licensed under MIT or Apache-2.0, at your option. Third-party data and assets have separate terms; see [NOTICE.md](NOTICE.md), [LICENSE-MIT](LICENSE-MIT), and [LICENSE-APACHE](LICENSE-APACHE).

## Highlights

- Schematic editor with wiring, net labels, ERC, undo/redo, and component copy/cut/paste.
- Three built-in simulation engines, transient analysis, plots, and CSV export.
- Two-layer PCB editor with manual tracks, an initial A* autorouter, vias, non-plated holes, and design-rule checks.
- Gerber/Excellon manufacturing export with a JLCPCB preset, validated through the external KiCad CLI.
- Offline physical catalog with 2,283 STM32 and 48 Espressif device entries.
- CLI for validation, netlists, simulation, SPICE export, project actions, and Proteus import.
- Native `.protocraft` project format.

## Quick start

On Windows, run `Iniciar-ProtoCraft.cmd`. To build from source, install the Rust toolchain pinned in `rust-toolchain.toml` and the MSVC C++ build tools, then run:

```powershell
cargo run -p eda-app
```

See [README.en.md](README.en.md) for complete build and CLI instructions, or [README.pt-BR.md](README.pt-BR.md) for the Portuguese documentation.

## Documentation

- [Build, features, and CLI (English)](README.en.md)
- [Construção, funcionalidades e CLI (Português)](README.pt-BR.md)
- [Modelo 555 com LED e CLI](docs/555-led-1hz.md)
- [Architecture](docs/architecture.md) · [PCB routing](docs/manual-routing.md) · [Simulation](docs/simulation-engines.md)
- [Proteus import](docs/proteus-import.md) · [File format](docs/file-format.md) · [Third-party notices](NOTICE.md)
