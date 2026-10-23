# 555 com LED perto de 1 Hz

O modelo `555-led-1hz` cria um esquemático inicial que pisca um LED com um temporizador 555 em configuração astável. Ele é voltado a abrir, editar e simular dentro do ProtoCraft; não inclui uma PCB já roteada.

## Componentes do preset

| Referência | Valor | Função |
|---|---:|---|
| J1 | 5 V | Alimentação |
| U1 | 555 genérico | Oscilador astável |
| RA | 47 kΩ | Resistência de temporização para VCC |
| RB | 47 kΩ | Resistência de temporização para descarga |
| C1 | 10 µF | Capacitor de temporização |
| C2 | 10 nF | Bypass do pino CTRL |
| D1 | LED genérico | Indicador ligado à saída |
| RLED | 680 Ω | Limitação de corrente do LED |

A equação aproximada clássica é `f ≈ 1.44 / ((RA + 2 RB) C1)`. Com os valores iniciais, isso resulta em aproximadamente 1,02 Hz. O cálculo é uma estimativa; o motor Misto usa modelos genéricos próprios e o resultado simulado pode variar.

## Criar, personalizar e simular

```powershell
ProtoCraft-CLI.cmd templates
ProtoCraft-CLI.cmd new --template 555-led-1hz --output pisca.protocraft --open
ProtoCraft-CLI.cmd simulate pisca.protocraft --engine mixed --transient 0.00025 3 --csv pisca.csv
```

Para fazer o CLI calcular C1 para 1 Hz:

```powershell
ProtoCraft-CLI.cmd new --template 555-led-1hz --frequency-hz 1 --output pisca-1hz.protocraft --open
```

RA, RB e o resistor do LED podem ser ajustados com `--ra-ohms`, `--rb-ohms` e `--led-resistor-ohms`. Para escolher diretamente o capacitor, passe `--c-farads` e não use `--frequency-hz` na mesma execução. Os argumentos numéricos usam unidades SI: 47000 representa 47 kΩ e `0.00001` representa 10 µF.

O arquivo salvo é um projeto normal `.protocraft` v6; os valores também podem ser alterados no editor ou por `run --cmd set_component`. A geração de PCB, o posicionamento dos footprints, o roteamento manual, o DRC e a exportação Gerber são etapas separadas.
