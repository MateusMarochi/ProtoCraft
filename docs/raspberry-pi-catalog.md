# Catálogo Raspberry Pi

O catálogo físico inclui 27 definições de pinagem para placas Raspberry Pi que expõem headers GPIO documentados. Cada definição pode ser consultada por `device <modelo>` e usada por `device-project <modelo> --output projeto.protocraft`. Os footprints de conectores são geometria original do ProtoCraft baseada nas dimensões e atribuições elétricas publicadas pela Raspberry Pi.

## Computadores com header GPIO clássico de 40 pinos

Estão disponíveis definições para Raspberry Pi 1 Model A+/B+, Pi 2 Model B, Pi 3 Model A+/B/B+, Pi 4 Model B, Raspberry Pi 5, Zero, Zero W, Zero WH, Zero 2 W e Zero 2 W with headers. As variantes Zero com e sem header soldado compartilham o mapa elétrico; o catálogo representa o conector de expansão, não a presença física do header em uma unidade comprada.

Pi 400, Pi 500 e Pi 500+ também estão incluídos. Seus headers ficam na parte traseira do teclado e têm orientação física invertida em 180° em relação às placas clássicas; por isso usam footprints próprios, com os números dos pads preservados e a geometria invertida.

As definições Pi 5/500 usam nomes de GPIO e alimentação no conector. Funções alternativas multiplexadas dependem da geração do SoC/controlador e não são apresentadas como se fossem idênticas em todos os modelos. Nenhum GPIO clássico é tolerante a 5 V; os níveis são de 3,3 V.

## Modelos antigos com header de 26 pinos

O catálogo diferencia:

- Raspberry Pi 1 Model B revisão 1.0;
- Raspberry Pi 1 Model B revisão 2.0;
- Raspberry Pi 1 Model A com a pinagem da revisão 2.0.

As revisões B 1.0 e 2.0 não são intercambiáveis nos pinos físicos 3, 5 e 13. Na revisão 1.0, eles são GPIO0/SDA0, GPIO1/SCL0 e GPIO21; na revisão 2.0, são GPIO2/SDA1, GPIO3/SCL1 e GPIO27. O conector auxiliar P5 da revisão 2.0 não está incluído nesta definição P1 de 26 pinos.

## Raspberry Pi Pico e Pico 2

As oito variantes estão disponíveis: Pico, Pico H, Pico W, Pico WH, Pico 2, Pico 2 with headers, Pico 2 W e Pico 2 W with headers. Elas usam o mesmo mapa de 40 posições do conector principal por geração correspondente, mas seu layout físico é diferente do header Raspberry Pi 2×20: são duas fileiras separadas por 17,78 mm, com numeração que sobe por uma fileira e retorna pela outra. O footprint Pico tem essa ordem física explícita.

O pino 33 é GND/AGND; o pino 30 é RUN; os pinos 31, 32 e 34 são GP26/ADC0, GP27/ADC1 e GP28/ADC2; alimentação inclui ADC_VREF, 3V3(OUT), 3V3_EN, VSYS e VBUS. Nas versões wireless, alguns GPIOs do RP2040/RP2350 são usados internamente pelo rádio e não devem ser tratados como recursos livres. A pinagem não representa a placa completa, antena, debug SWD ou simulação de firmware.

## Base mecânica HAT via CLI

```powershell
ProtoCraft-CLI.cmd raspberry-pi-hat --output projetos\hat-base.protocraft
# Alternativa:
ProtoCraft-CLI.cmd new --template raspberry-pi-hat --output projetos\hat-base.protocraft
```

O template cria uma placa de 65 × 56 mm para conector through-hole, raio de canto de 3 mm, quatro furos não conectados de 2,75 mm com centros espaçados em 58 × 49 mm e um header GPIO de 40 posições. O footprint do header coloca o pad 1 em (8,37 mm; 5 mm) no sistema de coordenadas da placa.

Isso é uma base mecânica de projeto, não uma declaração de que a placa é HAT ou HAT+ compatível. Não há EEPROM de identificação, mapa de GPIO/device tree, keepout PoE nem recortes de acesso a flex CSI/DSI. O template também não substitui uma verificação de altura de componentes, espaçadores, gabinete e conectores. O projeto anterior `raspberry-pi-4-shield` continua como exemplo customizado de 85 × 56 mm com GPIO17 e LEDs; essa dimensão não é o formato HAT padrão.

## O que ainda não está no catálogo

Compute Module 1/3/3+/4/4S/5 não usa um header GPIO comum na placa módulo. As famílias usam conectores SODIMM ou conectores high-density com atribuições específicas e precisam ser montadas em carrier boards. O catálogo não finge que o header GPIO de uma placa Pi completa representa esses conectores. A próxima inclusão deve trazer mapa oficial completo, footprint do conector correto e variantes próprias por geração, além de testes de correspondência pad a pad.

O catálogo também não cria um registro separado para cada tamanho de RAM, armazenamento ou revisão de produção quando essas opções não mudam o mapa do header. Isso evita duplicar definições idênticas e mantém a busca centrada no modelo de placa e pinagem.

## Referências oficiais

- [Documentação de hardware Raspberry Pi e GPIO](https://www.raspberrypi.com/documentation/computers/raspberry-pi.html)
- [Série de produtos Raspberry Pi](https://www.raspberrypi.com/news/raspberry-pi-product-series-explained/)
- [Documentação Raspberry Pi Zero](https://www.raspberrypi.com/documentation/computers/zero.html)
- [Documentação Raspberry Pi Pico](https://www.raspberrypi.com/documentation/microcontrollers/pico-series.html)
- [Diagrama oficial de pinagem Pico/Pico 2](https://datasheets.raspberrypi.com/pico/Pico-2-Pinout.pdf)
- [Mudanças entre as revisões iniciais do Pi 1](https://www.raspberrypi.com/news/upcoming-board-revision/)
- [Especificação mecânica Raspberry Pi HAT](https://github.com/raspberrypi/hats/blob/master/hat-board-mechanical.pdf)
- [Especificação Raspberry Pi HAT+](https://datasheets.raspberrypi.com/hat/hat-plus-specification.pdf)
