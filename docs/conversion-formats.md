# Intercâmbio de projetos

O comando `protocraft-cli conversion-formats` imprime a matriz de leitura e gravação disponível na versão atual. Compatibilidade significa somente os formatos e subconjuntos descritos abaixo; nenhum conversor promete equivalência integral com Proteus ou KiCad.

## Formatos

| Formato | Leitura | Gravação | Limites principais |
| --- | --- | --- | --- |
| ProtoCraft (`.protocraft`) | Formato nativo, com versões suportadas pela persistência | Sim | Round-trip preserva o modelo nativo e aplica migrações documentadas. |
| Proteus ISIS (`.pdsprj`, `.pdsbak`, `.dsn`) | Experimental; contêiner FILEVER 800 e subconjunto binário DSN reconhecido | Não | A importação parcial de esquema não importa a PCB, não recupera todos os objetos, modelos, barramentos ou folhas e não permite reexportação fiel. |
| KiCad PCB (`.kicad_pcb`) | Subconjunto físico documentado no relatório | Sim | O conversor trabalha no domínio PCB e não preserva o esquema, configurações completas, regras avançadas ou toda a geometria e estrutura KiCad. |
| KiCad esquemático (`.kicad_sch`) | Subconjunto de fios, junções, rótulos e pinos de símbolos incorporados | Sim | Gráficos dos símbolos, hierarquia, tipos ERC e modelos de simulação não viram recursos nativos equivalentes. Símbolos de alimentação só são mapeados quando nome e pino conferem. |
| Configurações KiCad (`.kicad_pro`) | Não | Não | É configuração do projeto, não o esquema ou a placa. Converta os arquivos `.kicad_sch` e `.kicad_pcb` correspondentes. |
| DSN Specctra (`.dsn`) | Não nesta versão | Não | O sufixo `.dsn` também é usado por roteamento Specctra; a assinatura de texto é diferenciada do DSN binário do Proteus ISIS. |
| KiCad legado (`.sch`, `.brd`) | Não nesta versão | Não | Converta com uma versão compatível do KiCad para um formato moderno antes de importar. |

## Subconjuntos implementados

O importador `.kicad_pcb` lê footprints de topo e verso, pads numerados, algumas dimensões e margens de pad, furos, nomes de redes, segmentos retos em F.Cu/B.Cu, vias passantes e contornos retangulares. Ele cria um projeto PCB e um esquema sintético de pinos/rótulos para manter os nomes de redes da placa disponíveis no modelo. Esse esquema sintético não reconstrói o esquema elétrico do projeto KiCad.

Zonas de cobre e preenchimentos, camadas internas, arcos de trilha, formas de pad customizadas, contornos não retangulares, texto e regras avançadas geram perdas ou aproximações no relatório. Um contorno irregular pode virar sua caixa delimitadora; um footprint sem pinos elétricos reconhecíveis pode ser omitido. Pads com net 0 permanecem desconectados. Confira os objetos omitidos antes de editar ou fabricar.

O importador `.kicad_sch` lê fios de dois pontos, junções, labels e global labels em folhas sem hierarquia, além dos pinos encontrados nas definições de símbolo incorporadas. As instâncias mantêm referência, valor, propriedades textuais e posição dos pinos. Símbolos `power` só criam um rótulo de rede quando o valor e o nome do pino coincidem; casos ambíguos são perda elétrica. Tipos ERC e modelos de simulação não viram equivalentes executáveis.

Os exportadores escrevem um artefato de domínio: `.kicad_pcb` contém a PCB e não o projeto completo; `.kicad_sch` contém um esquema genérico e não a geometria da placa, desenhos originais dos símbolos ou modelos de simulação. O `kicad-cli` pode validar sintaxe e produzir netlists/arquivos de placa, mas isso não certifica equivalência semântica nem fabricação.

O KiCad documenta `.kicad_pcb` e `.kicad_sch` como formatos S-expression próprios; coordenadas, campos e regras variam por objeto. Consulte a [documentação oficial de placa](https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/), [documentação oficial de esquemático](https://dev-docs.kicad.org/en/file-formats/sexpr-schematic/) e [introdução ao formato](https://dev-docs.kicad.org/en/file-formats/sexpr-intro/) ao revisar intercâmbios externos.

## Política de perda

O comando `convert` recusa por padrão qualquer resultado que tenha perdas ou que reduza um projeto a apenas esquema ou PCB. O relatório inclui os formatos detectados, o escopo, perdas, avisos, a indicação de completude e se o arquivo de saída foi escrito.

`--allow-lossy` permite a gravação após revisar as limitações. Ele não converte automaticamente um esquema em uma PCB, não cria modelos elétricos ausentes e não transforma perdas em compatibilidade. A opção `--report` grava o relatório em JSON; sem ela, o relatório é impresso no terminal. Arquivos existentes nunca são substituídos.

Relatório e artefato são dois arquivos independentes. A publicação não é atômica como um par: se houver falha de acesso ao disco entre as gravações, o relatório pode não ser publicado após a saída. O campo `artifact_publication` do JSON registra essa condição.

## Exemplos

```powershell
protocraft-cli conversion-formats
protocraft-cli convert .\placa.kicad_pcb --to protocraft --output .\placa.protocraft --report .\placa-conversao.json
protocraft-cli convert .\projeto.protocraft --to kicad-pcb --output .\projeto.kicad_pcb --allow-lossy --report .\projeto-pcb.json
protocraft-cli convert .\esquema.kicad_sch --to protocraft --output .\esquema.protocraft --allow-lossy --report .\esquema-importacao.json
```

Um `.kicad_pro` não pode ser convertido sozinho porque não contém necessariamente a geometria do esquema e da placa. Um projeto Proteus não pode ser exportado de volta pelo ProtoCraft nesta versão.
