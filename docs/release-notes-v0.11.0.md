# ProtoCraft v0.11.0

## Novidades

- Importação Falstad/CircuitJS pelo desktop e pela CLI para texto legado, XML, atalhos `.url` e links HTTPS suportados. Fontes, passivos, chaves, diodos, BJTs, MOSFETs de três terminais e amplificadores operacionais reconhecidos podem ser convertidos para o modelo nativo.
- Modelos importados são aproximações do solver ProtoCraft. Instrumentos e anotações visuais são omitidos com aviso; dispositivos, pinagens e condições sem suporte são sinalizados e podem bloquear simulação.
- A resistência série de diodos agora participa do modelo não linear nativo e da exportação SPICE. Resistência interna positiva de fonte é preservada como resistor série.
- Novos fluxos CLI para relatórios DFM, variantes de montagem DNP, varredura DC de uma fonte/resistor e medições transientes.
- Verificação independente de folga entre furações e relatório DFM com limites configuráveis. Verificação geométrica não certifica fabricabilidade, segurança elétrica, térmica ou EMI.

## Limites conhecidos

As variantes DNP filtram relatórios de montagem; não removem componentes do esquemático ou PCB nem recalculam conectividade/simulação. Varredura DC aceita somente um resistor ou uma fonte DC. Não há equivalência numérica geral com CircuitJS ou Proteus. Zonas de cobre, hierarquia completa de folhas, roteamento profissional, co-simulação de firmware e 3D permanecem futuros.

O formato de projeto nativo permanece na versão 7, com migração das versões anteriores descritas em [file-format.md](file-format.md).
