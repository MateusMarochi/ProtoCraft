# Shield de teste para Raspberry Pi 4 Model B

O repositório agora contém um projeto `.protocraft` gerado pela CLI: [`examples/raspberry-pi-4-shield.protocraft`](../examples/raspberry-pi-4-shield.protocraft). Ele é um protótipo de shield para estudo, com PCB roteada e validada pelo DRC nativo. Não é uma aprovação de engenharia nem um pacote de fabricação verificado externamente.

## Criar e conferir pela CLI

Na raiz do repositório:

```powershell
cargo run -p eda-cli -- devices --filter RaspberryPi4B_GPIO
cargo run -p eda-cli -- device RaspberryPi4B_GPIO
cargo run -p eda-cli -- raspberry-pi-4-shield --output shield-cli.protocraft
cargo run -p eda-cli -- new --template raspberry-pi-4-shield --output shield-template.protocraft
cargo run -p eda-cli -- validate shield-cli.protocraft
cargo run -p eda-cli -- pcb-check shield-cli.protocraft
cargo run -p eda-cli -- pcb-export shield-cli.protocraft --format kicad --output shield-cli.kicad_pcb
cargo run -p eda-cli -- pcb-export shield-cli.protocraft --format svg --output shield-cli.svg
```

`raspberry-pi-4-shield` e `new --template raspberry-pi-4-shield` criam o mesmo circuito/PCB. Os comandos recusam sobrescrever o arquivo de destino. O modelo `RaspberryPi4B_GPIO` está no catálogo físico compartilhado pelo desktop e pela CLI; `devices` e `device` mostram seus 40 números físicos, nomes de sinal, offsets de pino e mapeamento pad a pad. Isso é um modelo físico; não executa o sistema operacional nem emula firmware do Raspberry Pi.

## O que está no exemplo

- Board de **85 × 56 mm**, cantos com raio **3 mm**, espessura de 1,6 mm.
- Quatro furos NPTH para montagem, padrão de centros **58 × 49 mm**, propostos a 3,5 mm das bordas; furação de 2,75 mm.
- Header PTH GPIO **2 × 20**, passo de 2,54 mm, com pinagem física 1–40 correspondente ao Pi 4 Model B. O pad 1 fica em `(8,37; 5,00) mm`; o centro das 20 posições fica a 32,50 mm da borda longa.
- Ponto de teste PTH identificado como `TP_GPIO17`, ligado ao GPIO17 no pino físico 11.
- LED vermelho para 5 V com resistor série **1 kΩ** e LED verde para 3V3 com resistor série **680 Ω**; ambos retornam ao GND do pino físico 6.
- Trilhas geradas pelo autorroteador, com início e fim da saída GPIO17 nas pads correspondentes.

Os resistores limitam a corrente dos LEDs; nunca ligue LED diretamente entre alimentação e terra. A corrente real depende da queda direta dos LEDs instalados. Os outros pinos continuam no header e no modelo do dispositivo, mas não recebem furos de teste individuais neste exemplo.

## Pinagem do header

Os números da tabela são físicos; não confundir com os identificadores BCM. Os GPIO trabalham em 3,3 V e **não toleram 5 V**. Os pinos ID_SD/ID_SC (27/28) são reservados ao ID EEPROM em HATs.

| Físico | Sinal | Físico | Sinal |
|---:|---|---:|---|
| 1 | 3V3 | 2 | 5V |
| 3 | GPIO2 / SDA1 | 4 | 5V |
| 5 | GPIO3 / SCL1 | 6 | GND |
| 7 | GPIO4 / GPCLK0 | 8 | GPIO14 / TXD0 |
| 9 | GND | 10 | GPIO15 / RXD0 |
| 11 | GPIO17 | 12 | GPIO18 / PWM0 |
| 13 | GPIO27 | 14 | GND |
| 15 | GPIO22 | 16 | GPIO23 |
| 17 | 3V3 | 18 | GPIO24 |
| 19 | GPIO10 / SPI0_MOSI | 20 | GND |
| 21 | GPIO9 / SPI0_MISO | 22 | GPIO25 |
| 23 | GPIO11 / SPI0_SCLK | 24 | GPIO8 / SPI0_CE0 |
| 25 | GND | 26 | GPIO7 / SPI0_CE1 |
| 27 | ID_SD | 28 | ID_SC |
| 29 | GPIO5 | 30 | GND |
| 31 | GPIO6 | 32 | GPIO12 / PWM0 |
| 33 | GPIO13 / PWM1 | 34 | GND |
| 35 | GPIO19 / PCM_FS | 36 | GPIO16 |
| 37 | GPIO26 | 38 | GPIO20 / PCM_DIN |
| 39 | GND | 40 | GPIO21 / PCM_DOUT |

## Origem e limites mecânicos

A pinagem e o passo 2,54 mm seguem a documentação oficial de GPIO da Raspberry Pi. A dimensão geral de 85 × 56 mm e o padrão de furos vêm do desenho mecânico oficial do Pi 4 Model B. O raio de 3 mm e os furos de 2,75 mm seguem a especificação mecânica de HAT da Raspberry Pi. A geometria eletrônica do footprint é original do ProtoCraft; revise alinhamento, tipo/altura do conector e desenho mecânico da revisão física usada antes de fabricar.

A dimensão de 85 × 56 mm foi escolhida a pedido para acompanhar o formato da placa do Pi 4. Ela é maior que o contorno tradicional de HAT (65 × 56,5 mm), então pode interferir com USB, HDMI, Ethernet, conectores de câmera ou gabinetes. Considere este projeto um exemplo de pinagem e posição dos furos, ainda sem validação de montagem sobre uma Pi real. A exportação KiCad preserva o contorno arredondado como linhas e arcos; Gerber/Excellon ainda precisa ser gerado com KiCad CLI, verificado por DRC independente e revisado no visualizador CAM do fabricante. O DRC nativo sem erros não garante montagem, compatibilidade mecânica, segurança elétrica ou fabricabilidade.

Referências primárias: [documentação oficial do GPIO](https://www.raspberrypi.com/documentation/computers/raspberry-pi.html), [desenho mecânico do Pi 4 Model B](https://pip.raspberrypi.com/categories/559-mechanical) e [especificação mecânica HAT+](https://datasheets.raspberrypi.com/hat/hat-plus-specification.pdf).
