# ProtoCraft

<p align="center"><img src="assets/protocraft-logo.png" alt="ProtoCraft ant on a microchip" width="240"></p>

<p align="center"><a href="README.pt-BR.md">Português</a> · <a href="README.en.md">English</a></p>

ProtoCraft is a Rust desktop application for electronic design. It brings together a schematic editor, original circuit simulation engines, manual and initial automatic two-layer PCB routing, physical STM32/ESP32 device data, a CLI, and an experimental Proteus project importer.

> **Project status:** under active development. Original ProtoCraft source code, documentation, and bundled circuit examples are licensed under MIT or Apache-2.0, at your option. Third-party data and assets have separate terms; see [NOTICE.md](NOTICE.md), [LICENSE-MIT](LICENSE-MIT), and [LICENSE-APACHE](LICENSE-APACHE).

## Features

- Schematic canvas with grid, zoom, pan, placement, rotation, wiring, junctions, net names, and component properties.
- Ctrl+C/Ctrl+X/Ctrl+V for schematic and PCB components, plus undo and redo.
- 36 component types, including passive parts, semiconductor devices, logic, comparators, Schmitt triggers, 555 timers, flip-flops, counters, and controlled sources.
- Three built-in engines: Stable (backward Euler), Dynamic (trapezoidal), and Mixed (analog with digital events). Supports DC and transient analysis, continuous runs, plots, and CSV/SPICE export.
- Two-layer PCB editor with manual 45-degree, 90-degree, and free-angle tracks, an initial A* autorouter, vias, non-plated holes, geometry editing, and design-rule checks.
- Manufacturing Gerber/Excellon generation with RS-274X, Protel extensions, millimeters, absolute drill coordinates, 1:1 scale, and ZIP packaging through the JLCPCB preset. KiCad CLI is required for this optional workflow.
- Offline physical catalog with 2,283 STM32 and 48 Espressif entries, including pins, packages, pads, and dimensions. This is physical design support; MCU firmware is not emulated.
- English and Brazilian Portuguese (pt-BR) desktop/CLI localization, selected from the operating-system locale with an English fallback; translators can replace JSON catalogs without rebuilding.
- Native JSON `.protocraft` format with migrations and recovery backups.
- Experimental Proteus ISIS 8 import for selected `.pdsprj` and `.dsn` project data, with import reports and preservation of unsupported model information.
- Headless CLI sharing the application’s project, connectivity, and simulation code.

## Windows quick start

Run `Iniciar-ProtoCraft.cmd`. It uses a local release executable when available, otherwise it builds and starts the application. Rust and the MSVC C++ build tools are required for compilation. Packaged Windows files are placed under `dist/ProtoCraft/` by the packaging script.

The initial example is a 10 V source and two 1 kΩ resistors. Run **DC** to see `V(VOUT) = 5 V`. The examples menu includes RC filters, diode limiters, digital circuits, and MCU footprint demonstrations. The UI includes dark and light themes.

## Build from source

The Rust toolchain is pinned in `rust-toolchain.toml` (Rust 1.99.1, edition 2024). On Windows, install the MSVC C++ build tools and Windows SDK.

```powershell
cargo run -p eda-app
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

`scripts/build.ps1` runs the build checks and compilation. `scripts/package.ps1` prepares the Windows package under `dist/ProtoCraft/`.

After packaging, `scripts/installer.ps1` builds the Windows x64 installer; Inno Setup 6 must be installed.

## CLI

Run CLI commands from the repository root with `cargo run -p eda-cli --`, or use the packaged `protocraft-cli.exe`:

```text
templates
new --output blink.protocraft --template 555-led-1hz --frequency-hz 1 --open
simulate blink.protocraft --engine mixed --transient 0.00025 3 --csv blink.csv
open blink.protocraft
engines
components
info examples/divisor.protocraft
validate examples/divisor.protocraft
netlist examples/divisor.protocraft
simulate examples/divisor.protocraft
simulate examples/rc.protocraft --engine dynamic --transient 0.00001 0.005 --csv rc.csv
simulate examples/555-astavel.protocraft --engine mixed --transient 0.00001 0.01 --csv timer.csv
spice examples/rc.protocraft --transient 0.00001 0.005 --output rc.cir
protocraft-cli import-proteus circuit.pdsprj --output circuit.protocraft --report import.json
```

For example, run `cargo run -p eda-cli -- simulate examples/rc.protocraft`. The `--transient` option takes a time step and duration in seconds. `run` applies typed project actions atomically in headless mode. See the [CLI guide](docs/cli.md) for details.

The desktop application also accepts a project path and analysis options, such as `protocraft.exe examples/rc.protocraft --analysis transient --engine dynamic`, or `protocraft.exe --settings` to open settings.

## STM32, ESP32, and PCB

Select **STM32 / ESP32** in the library and search for a device. The inspector displays package, physical pins, and pin-to-pad mapping. Use **View → PCB — real dimensions** to open the board editor, synchronize footprints, place components, and route manually. The catalog includes 2,331 entries and 162 footprints, for devices up to 448 pins. Sources and license terms are documented in `assets/library/` and `NOTICE.md`.

The application does not execute MCU firmware or simulate their on-chip peripherals. See the [MCU and PCB guide](docs/mcu-pcb.md).

## Importing Proteus projects

Use **File → Import Proteus…** and choose a `.pdsprj` or `.dsn`. The `.pdsprj` reader supports the `FILEVER=800` profile; a standalone DSN can use a same-named CDB file in its directory. The importer reconstructs selected components, pins, wires, junctions, labels, and ground, then reports unsupported data. Save the result as a separate `.protocraft` file; source files are preserved.

This import is partial and intended for editing and inspection. Unsupported components may be displayed as generic symbols. Imported projects with incomplete connectivity are blocked from simulation and SPICE export to avoid silently omitting connections. Compatibility with all Proteus 8/9 projects is not guaranteed. See the [import guide](docs/proteus-import.md), [DSN profile](docs/proteus-dsn.md), [CDB profile](docs/proteus-cdb.md), and [CLI guide](docs/proteus-cli.md).

## Current limits

The circuit models are generic and educational, without manufacturer characterization. Mixed simulation does not model setup/hold, metastability, or commercial device characterization. MCU firmware, memories, ADC/DAC, JFET, small-signal AC, parasitics, noise, advanced PCB features, rip-up routing, zones, differential pairs, and full compatibility with external simulators are outside the current implementation. PCB manufacturing output requires independent engineering review before fabrication.

Imported projects with partial connectivity remain blocked from simulation. Review the [simulation validation](docs/simulation-validation.md), [engine documentation](docs/simulation-engines.md), and [roadmap](docs/roadmap.md).

## Project documentation

- [Architecture](docs/architecture.md) and [contracts](docs/contracts.md)
- [File format and recovery](docs/file-format.md)
- [CLI actions and examples](docs/cli.md)
- [555 LED blinker starter template](docs/555-led-1hz.md)
- [Simulation engines](docs/simulation-engines.md), [mixed simulation](docs/mixed-simulation.md), and [validation](docs/simulation-validation.md)
- [Manual PCB routing](docs/manual-routing.md), [MCU/PCB catalog](docs/mcu-pcb.md), and [JLCPCB preset](docs/manual-routing.md#gerber-and-excellon-manufacturing-export)
- [Third-party notices and attribution](NOTICE.md)

The design goal is a usable EDA built from independently implemented code and documented public data. ProtoCraft does not include Proteus code or binaries. Original source code, documentation, and bundled circuit examples are available under MIT or Apache-2.0, at your option. See [NOTICE.md](NOTICE.md) for separate terms covering third-party data and assets.

## Search terms

Rust EDA, electronic design automation, circuit simulator, schematic capture, circuit design, PCB layout, PCB routing, PCB autorouter, Gerber RS-274X, Excellon drill files, SPICE netlist export, STM32, ESP32, Proteus project importer, `.protocraft`, desktop EDA, headless CLI.
