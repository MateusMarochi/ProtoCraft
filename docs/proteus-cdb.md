# Perfil CDB experimental do importador

O parser `crates/eda-import/src/cdb.rs` é uma implementação original obtida por
comparação estrutural de seis arquivos de exemplo locais. Não inclui código,
símbolos, bibliotecas de modelos nem fixtures de terceiros. As amostras foram
lidas como dados, sem executar o Proteus. Esta descrição é um perfil observado,
não uma especificação oficial ou garantia de compatibilidade universal.

## Evidência e cobertura

Todos os seis `ROOT.CDB` foram decodificados até o fim do arquivo sem bytes
restantes e com os vínculos de peça íntegros:

| Exemplo externo | Bytes | Instâncias | Peças |
| --- | ---: | ---: | ---: |
| Circuito RC | 463 | 5 | 3 |
| Portas Lógicas | 2500 | 18 | 14 |
| Multiplexador | 2216 | 17 | 12 |
| FlipFlop RS | 1528 | 12 | 8 |
| Inversor CMOS | 819 | 9 | 3 |
| Somador Completo | 7084 | 48 | 42 |

As instâncias incluem objetos sem peça associada: fontes/interativos,
instrumentos e marcadores podem ter referência vazia, `M...` ou `P...`. O parser
preserva esses registros; ele não transforma automaticamente cada instância em
um componente simulável. Não se deve filtrar por prefixo de referência.

O CDB deste perfil contém associação instância/peça, referência, pinos e
metadados de modelo. **Não contém registros de fios nem redes elétricas
decodificadas por este parser.** A conectividade e a geometria precisam do DSN.
As três strings de pinos `D/G/S` dos MOS não são redes: seus pares `1/2/3` são
números físicos. No inversor CMOS, Q2 tem `D -> 2`, `G -> 1`, `S -> 3`; portanto,
assumir numeração pela ordem de enumeração altera o circuito.

## Codificação observada

- Todos os inteiros abaixo são `u32` little endian, sem alinhamento implícito.
- `str8` = tamanho `u8`, seguido por exatamente esse número de bytes, sem NUL.
- O perfil atual aceita texto ASCII; outros encodings são rejeitados para
  impedir conversões silenciosas. Strings vazias são válidas.
- Campos de semântica desconhecida mantêm nomes `field_N` na representação
  intermediária. O primeiro inteiro identifica registros; o vínculo `part_id`
  foi confirmado pelo cruzamento de referências nas seis amostras.

### Prefixo e tabelas

```text
u32[4] = [2, 1, 1, 0]
str8 root_name
u32 = 0
u8 = 0
u32[6] = [1, 1, 2, 1, 1, 1]
u8 = 0
u32[5] = [10, 0, 2, 2, 0]
str8 sheet_name
u32[2] = [10, 0]
u32 instance_count
Instance[instance_count]
u32[3] = [1, 1, 1]
u32 part_count
Part[part_count]
EOF
```

As amostras usam `root_name = ROOT` e `sheet_name = Master Sheet`; os nomes
podem variar no parser, mas os demais marcadores precisam corresponder ao
perfil conhecido de uma folha. Não há suporte confirmado a hierarquia,
múltiplas folhas ou PCB.

### Instância

```text
u32 id
u32 field_2
u32 field_3
str8 reference
u32 pin_count
(str8 name, str8 number)[pin_count]
u32 field_4
u32 part_id             # 0 significa ausência de peça
u32 field_6
```

Nas amostras, `field_2 = 1`, `field_3 = id`, `field_4 = 0`; `field_6` varia
entre `0` e `0xffffffff`. Não se atribui semântica elétrica a esses campos.
Os IDs não são necessariamente contíguos. A instância Q7 do exemplo de portas
lógicas tem `part_id = 5`, enquanto Q6 tem `part_id = 6`; procurar uma peça pela
posição na lista daria resultados errados.

### Peça

```text
u32 id
u32 field_2
u32 field_3
str8 reference
str8 value
str8 device
str8 package
u32 inclusive_length
u8[inclusive_length - 5] properties_text
u8 = 0
```

`inclusive_length` inclui os quatro bytes do próprio comprimento e o NUL
final. Por exemplo, o bloco RC tem tamanho declarado 26 e texto de 21 bytes,
seguido por NUL. Interpretar 26 como tamanho só do texto invade a próxima peça.

O valor `X` da bateria do RC é literal no CDB; não se inventa uma tensão.
Propriedades como `MODFILE`, `MODEL` e `PACKAGE` são preservadas como texto
inerte. O parser não resolve caminhos, lê modelos externos, executa firmware ou
ativa simulação automaticamente.

## Limites e erros

Limites atuais: 16 MiB por arquivo, 20.000 instâncias e peças por tabela, 1.024
pinos por instância e 1 MiB por bloco de propriedades. O parser exige consumo
exato do arquivo e rejeita truncamentos, cabeçalhos desconhecidos, comprimentos
impossíveis, IDs nulos/duplicados, referências de peça inexistentes, NUL
embutido, texto fora do perfil e dados extras. Cada erro inclui posição em bytes
e mensagem em português.

Não há recuperação por procura de strings soltas. Uma falha de interpretação
não pode gerar silenciosamente um esquema parcialmente conectado.

## Testes

Fixtures sintéticas autorais cobrem o mapeamento de pinos, todos os pontos de
truncamento da fixture, contagens excessivas, encoding, tamanho inclusivo,
duplicidade, referências pendentes e bytes adicionais.

O teste externo é opt-in e mantém os arquivos de terceiros fora do repositório:

```powershell
$env:PROTOCRAFT_PROTEUS_SAMPLES = 'C:\Users\Mateus\Downloads\RESULTADO_EXEMPLOS_PROTEUS\PROJETOS'
cargo test -p eda-import cdb::tests
```

Em 22/10/2026, o módulo foi compilado isoladamente por `rustc --test` e os seis
testes passaram, incluindo as seis amostras externas. A compilação isolada não
substitui a validação do workspace nem confirma a abertura visual dos projetos.
