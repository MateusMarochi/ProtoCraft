# Tradução da interface

Os catálogos JSON desta pasta contêm os textos visíveis da interface desktop e as mensagens humanas da CLI. Para criar ou revisar uma tradução, copie `en.json` ou `pt-BR.json` e edite **somente os valores**. As chaves são frases-fonte estáveis usadas pelo código; não as traduza nem remova. Placeholders como `{count}`, `{name}`, `{error}` e `{0}` devem permanecer com as chaves entre chaves, embora possam mudar de posição na frase traduzida.

O inglês (`en.json`) é o idioma de fallback. O ProtoCraft escolhe português do Brasil quando o locale do sistema é português; outros locales usam inglês. A preferência também pode ser definida nas configurações.

## Substituir o catálogo sem recompilar

O aplicativo incorpora os catálogos deste repositório e aceita arquivos JSON externos com o mesmo nome. Coloque `en.json` ou `pt-BR.json` em uma destas pastas; as entradas externas substituem apenas as chaves correspondentes e as demais continuam usando o catálogo incorporado:

- pasta `i18n` ao lado do executável;
- `%APPDATA%\ProtoCraft\i18n` no Windows;
- `$XDG_CONFIG_HOME/ProtoCraft/i18n` no Linux; se não definido, `~/.config/ProtoCraft/i18n`;
- pasta apontada pela variável de ambiente `PROTOCRAFT_I18N_DIR` (tem prioridade).

Reinicie o ProtoCraft após alterar um catálogo. Use UTF-8 e JSON válido. Um catálogo externo pode conter apenas os textos revisados; não precisa duplicar o arquivo inteiro.

## Glossário técnico

| English | Português (Brasil) |
| --- | --- |
| schematic | esquema elétrico |
| wire / net | fio / rede elétrica |
| junction | junção elétrica |
| footprint | footprint (padrão de pads do componente na PCB) |
| track / via | trilha / via |
| board outline | contorno da placa |
| copper clearance | afastamento mínimo entre objetos de cobre |
| electrical rule check (ERC) | verificação das regras elétricas (ERC) |
| courtyard | área de montagem (courtyard) |
| annular ring | anel de cobre ao redor de um furo metalizado |
| netlist | netlist |
| DC operating point | ponto de operação em corrente contínua (CC) |
| transient analysis | análise transiente |
| conventional current | corrente convencional |
| peak amplitude | amplitude de pico |
| duty cycle | ciclo de trabalho |
| low-pass / high-pass | passa-baixas / passa-altas |
| through-hole / surface-mount | montagem through-hole (PTH) / montagem em superfície (SMD) |
| drill file | arquivo de furação |
| plated / non-plated hole | furo metalizado (PTH) / furo não metalizado (NPTH) |
| design-rule check (DRC) | verificação das regras de projeto (DRC) |
| pad | pad (ilha de conexão) |
| pin | pino |

Prefira termos estabelecidos em engenharia elétrica e fabricação de PCB. Preserve siglas normalizadas, nomes próprios, unidades SI, referências de componentes e nomes de arquivos.
