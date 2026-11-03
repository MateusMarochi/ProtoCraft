# Plano de evolução — PCB primeiro

Este plano organiza o `BACKLOG.md` em entregas progressivas. O objetivo é ampliar o fluxo de projeto de PCB com dados e resultados verificáveis; não representa uma promessa de equivalência ao Proteus. Simulador de firmware e 3D ficam para o futuro.

## 1. Integridade e diagnóstico de PCB

- Garantir detecção de curtos e folgas em limites geométricos, incluindo folga zero, distintas camadas e redes; testar os mesmos projetos pela CLI e pelo DRC headless.
- Preservar projeto e gerações de backup em falhas de escrita; testar rotação e recuperação. A substituição de cada geração não torna a rotação inteira transacional.
- Reforçar o fluxo CLI: parâmetros inválidos ou duplicados devem falhar antes de gravar saída; testes de criação, edição, inspeção e DRC usam arquivos temporários.
- Aceite: testes de regressão positivos e negativos, round-trip de projeto e comando de inspeção reproduzível; `fmt`, `check`, Clippy e testes do workspace.

## 2. Construção básica de PCB

- Reconciliar alterações entre esquema e placa com relatório de impactos em pads, nets e cobre. Preservar posicionamentos e exigir decisões explícitas nos conflitos.
- Evoluir contorno e recortes, footprint/pads e posicionamento. Definir primeiro os contratos geométricos, formato e migração; não quebrar projetos existentes.
- Melhorar edição de trilhas conectadas e seleção em lote, mantendo DRC, histórico e cancelamento coerentes.
- Aceite: fixture autoral percorrendo esquema → PCB → edição → salvamento → DRC, pelos comandos do núcleo e pela CLI; testar interface real quando houver mudanças visuais.

## 3. Cobre e fabricação

- Desenvolver zonas e preenchimento determinístico, ilhas e térmicos; em seguida ampliar regras por camada, objeto e classe de rede.
- Ampliar verificações de furos, anéis, bordas, máscara, pasta e conectividade; cruzar os resultados com ferramenta independente.
- Validar Gerber/Excellon, BOM e posições de montagem em exemplos pequenos e depois em placas mais representativas; não declarar uma placa fabricável apenas pelo DRC.
- Aceite: saída vinculada à revisão do projeto, sem substituição indevida e conferida independentemente; falhas bloqueantes e limitações explícitas.

## 4. Roteamento e recursos avançados

- Aperfeiçoar roteamento manual e autorroteador com obstáculos, edição conectada, limites de execução, cancelamento e métricas reprodutíveis.
- Somente após regras e geometria confiáveis: multicamadas, pares diferenciais, ajuste de comprimento e perfis de fabricação. Qualquer alteração ampla no formato ou contrato exige decisão explícita.
- Aceite: fixtures positivas e negativas, integridade de nets, DRC, round-trip e testes de desempenho e cancelamento proporcionais.

## 5. Simulação e instrumentos

- Após consolidar o fluxo PCB, expandir validação DC/transiente com referência independente, modelos e diagnósticos; depois AC, varreduras, geradores e instrumentos.
- Definir unidades, tolerâncias e condições de contorno para cada modelo; testar cancelamento e resultados obsoletos. Backend externo depende de decisão de licença e isolamento.

## Processo de cada entrega

Registrar escopo e pré-condições, implementar pelo Engine e expor via CLI quando aplicável, acrescentar testes pequenos de regressão, verificar documentação e executar validações pertinentes. Em alterações visuais, abrir a interface apenas no monitor secundário e registrar o resultado; se não for possível, indicar a limitação. O backlog permanece a lista detalhada de itens e não deve ser marcado como concluído por uma correção parcial.
