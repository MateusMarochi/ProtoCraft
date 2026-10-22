# Biblioteca física adaptada do ProtoCraft

Dados derivados de KiCad Libraries (KiCad Library Team e contribuidores) e Espressif KiCad Libraries (Espressif Systems e contribuidores). A conversão, normalização, deduplicação de pads idênticos e separação de variantes físicas foram realizadas no projeto ProtoCraft em 22/10/2026. Não há modelos de simulação ou código de terceiros incorporados nessa coleção.

**Licença da coleção de dados: Creative Commons Attribution-ShareAlike 4.0 International, com a exceção das bibliotecas KiCad.** Esta coleção adaptada é redistribuída sob as mesmas condições. A licença da coleção não substitui a licença do código Rust.

- `kicad-symbols-LICENSE.md` e `espressif-LICENSE.md` preservam os avisos originais e a exceção.
- Licença: https://creativecommons.org/licenses/by-sa/4.0/legalcode
- Explicação da exceção: https://www.kicad.org/libraries/license/
- Fonte KiCad: https://gitlab.com/kicad/libraries/kicad-symbols e https://gitlab.com/kicad/libraries/kicad-footprints
- Fonte Espressif: https://github.com/espressif/kicad-libraries
- Cada dispositivo e footprint conserva URL e revisão da origem. `coverage.json` registra commits, hash do catálogo, cobertura e rejeições.
- O conversor original está em `scripts/build-device-library.py` no repositório ProtoCraft.

`catalog.json` é a coleção normalizada, legível e editável. `schema_version` versiona seus dados; o aplicativo usa uma cópia incorporada na compilação. Veja `docs/mcu-pcb.md` para geometria, campos e limitações. Dados fornecidos sem garantia; a revisão dos datasheets e das regras de fabricação permanece necessária.
