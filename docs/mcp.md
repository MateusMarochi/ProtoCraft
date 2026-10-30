# Automação MCP

O ProtoCraft oferece um servidor MCP local por stdio. Ele abre um projeto `.protocraft` em uma sessão headless e expõe inspeção e edição sem iniciar a interface gráfica.

## Iniciar

Na raiz do repositório:

```powershell
cargo run -p eda-cli -- mcp --project .\examples\555-astavel.protocraft
```

Com uma distribuição Windows já compilada:

```powershell
.\protocraft-cli.exe mcp --project C:\projetos\placa.protocraft
```

O cliente MCP deve iniciar esse comando como subprocesso e conectar `stdin`/`stdout`. O transporte usa mensagens JSON-RPC delimitadas por nova linha, limita cada mensagem a 1 MiB e reserva `stdout` para o protocolo. O processo encerra ao receber EOF. Use um processo por projeto.

Um exemplo genérico de configuração de cliente:

```json
{
  "mcpServers": {
    "ProtoCraft": {
      "command": "C:\\apps\\ProtoCraft\\protocraft-cli.exe",
      "args": ["mcp", "--project", "C:\\projetos\\placa.protocraft"]
    }
  }
}
```

O processo tem uma sessão headless própria. Alterações no editor desktop não aparecem nessa sessão e as edições MCP não atualizam uma janela já aberta. Use um caminho de projeto conhecido e feche sessões concorrentes se várias delas puderem salvar o mesmo arquivo.

## Protocolo

O servidor anuncia MCP `2026-07-28` por `server/discover`. As solicitações dessa versão carregam `protocolVersion` e `clientCapabilities` em `_meta`; as respostas incluem o tipo de resultado e a identidade do servidor. Clientes antigos podem usar `initialize` com `2025-11-25`, seguido da notificação `notifications/initialized`. Versões desconhecidas recebem erro de negociação com as versões aceitas.

## Ferramentas

| Ferramenta | Ação |
|---|---|
| `protocraft_project_info` | Nome, versão do formato, revisão, contagens e estado de undo/redo |
| `protocraft_project_snapshot` | Snapshot completo do projeto em JSON |
| `protocraft_project_validate` | Validação das invariantes do documento |
| `protocraft_netlist` | Netlist e diagnósticos ERC |
| `protocraft_command_list` | Catálogo de comandos, descrições e schema de parâmetros |
| `protocraft_device_search` | Busca em português/inglês com filtros por fabricante, família, encapsulamento, função, pinos e dimensão |
| `protocraft_pcb_check` | DRC, conexões pendentes e resumo de cobre |
| `protocraft_autoroute` | Planeja e aplica rotas aprovadas pelo DRC em uma edição atômica |
| `protocraft_execute` | Executa de 1 a 1.000 comandos tipados como uma edição indivisível |
| `protocraft_undo` / `protocraft_redo` | Desfaz/refaz se a revisão esperada ainda for atual |
| `protocraft_save` | Grava no arquivo da sessão e preserva backup `.bak` |

Chamadas de ferramenta anunciam seus limites por JSON Schema. `protocraft_execute` usa uma união discriminada com schema para cada ação. O Engine ainda aplica validação semântica do documento depois de validar a forma JSON.

## Alteração atômica e controle de revisão

Sempre consulte a revisão atual com `protocraft_project_info` ou `protocraft_project_snapshot`. Envie essa revisão em cada operação de edição. Exemplo de argumentos de `protocraft_execute`:

```json
{
  "expected_revision": 0,
  "actions": [
    {
      "command": "place_component",
      "params": {
        "kind": "Resistor",
        "position": { "x": 5, "y": 5 }
      }
    }
  ]
}
```

Se outra chamada alterar o projeto, a edição com uma revisão antiga falha com conflito. Releia o estado e gere uma nova chamada. O lote inteiro é rejeitado se uma ação for inválida; use `protocraft_save` para persistir após editar. Undo/redo também contam como uma nova revisão, e IDs atribuídos não são reutilizados ao criar uma ramificação depois de desfazer.

O autorroteador aplica seu plano apenas depois das verificações do motor. A ferramenta retorna estatísticas e conexões que não conseguiu rotear; execute `protocraft_pcb_check` novamente antes de fabricar a placa.

## Exemplo de descoberta

Pesquise por um microcontrolador físico da família ESP32 com encapsulamento QFN:

```json
{
  "name": "protocraft_device_search",
  "arguments": {
    "query": "ESP32",
    "form": "QFN",
    "limit": 20
  }
}
```

Os resultados vêm dos dados offline do ProtoCraft e incluem proveniência, pinos e dimensões. A busca não infere especificações que não estejam nos dados catalogados.
