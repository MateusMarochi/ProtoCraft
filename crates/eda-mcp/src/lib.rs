//! Servidor MCP stdio do ProtoCraft. O processo mantém um único Engine headless.

use eda_automation::{Action, COMMANDS};
use eda_core::{Command, CoreError, Engine};
use serde_json::{Value, json};
use std::{
    error::Error,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

const MODERN_VERSION: &str = "2026-07-28";
const LEGACY_VERSION: &str = "2025-11-25";
const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_ACTIONS: usize = 1_000;

struct Server {
    engine: Engine,
    project_path: PathBuf,
    legacy_initialized: bool,
    legacy_awaiting_initialized: bool,
}

/// Inicia o protocolo MCP por stdin/stdout para um projeto explícito.
/// Mensagens de diagnóstico nunca são escritas em stdout, reservado ao protocolo.
pub fn serve(path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let project_path = path.as_ref().to_path_buf();
    let project = eda_storage::load(&project_path)?;
    let engine = Engine::new(project)?;
    let mut server = Server {
        engine,
        project_path,
        legacy_initialized: false,
        legacy_awaiting_initialized: false,
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    while let Some(frame) = read_frame(&mut reader)? {
        if frame.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let response = match serde_json::from_slice::<Value>(&frame) {
            Ok(request) => server.handle(request),
            Err(error) => Some(rpc_error(
                Value::Null,
                -32700,
                &eda_i18n::format(
                    "JSON-RPC inválido: {error}",
                    &[("error", error.to_string())],
                ),
                None,
            )),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut writer, &response)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
    }
    Ok(())
}

fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if frame.is_empty() {
                return Ok(None);
            }
            return Ok(Some(frame));
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let payload_len = newline.unwrap_or(available.len());
        if frame.len().saturating_add(payload_len) > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                eda_i18n::text("A mensagem MCP excede o limite de 1 MiB."),
            ));
        }
        frame.extend_from_slice(&available[..payload_len]);
        let consumed = payload_len + usize::from(newline.is_some());
        reader.consume(consumed);
        if newline.is_some() {
            return Ok(Some(frame));
        }
    }
}

impl Server {
    fn handle(&mut self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let is_notification = id.is_none();
        if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || !request.get("method").is_some_and(Value::is_string)
            || id.as_ref().is_some_and(|value| {
                !value.is_string() && !value.is_u64() && !value.is_i64() && !value.is_null()
            })
            || id.as_ref().is_some_and(Value::is_null)
        {
            return if is_notification {
                None
            } else {
                Some(rpc_error(
                    id.unwrap_or(Value::Null),
                    -32600,
                    &eda_i18n::text("Solicitação JSON-RPC inválida."),
                    None,
                ))
            };
        }

        let method = request["method"].as_str().unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        let malformed_protocol_version = params
            .get("_meta")
            .and_then(Value::as_object)
            .is_some_and(|meta| {
                meta.contains_key("io.modelcontextprotocol/protocolVersion")
                    && !meta
                        .get("io.modelcontextprotocol/protocolVersion")
                        .is_some_and(Value::is_string)
            });
        if malformed_protocol_version {
            return id.map(|request_id| {
                rpc_error(
                    request_id,
                    -32600,
                    &eda_i18n::text("Versão de protocolo em _meta deve ser texto."),
                    None,
                )
            });
        }
        if method == "initialize" {
            let request_id = id?;
            return Some(self.initialize(request_id, &params));
        }
        if method == "server/discover" {
            let request_id = id?;
            return Some(match protocol_version(&params) {
                Some(MODERN_VERSION) if has_client_capabilities(&params) => {
                    rpc_response(request_id, self.discover(), true)
                }
                Some(MODERN_VERSION) => rpc_error(
                    request_id,
                    -32600,
                    &eda_i18n::text("Solicitação MCP moderna sem clientCapabilities."),
                    None,
                ),
                Some(version) => rpc_error(
                    request_id,
                    -32022,
                    &eda_i18n::text("Versão de protocolo MCP não suportada."),
                    Some(json!({"supported":[MODERN_VERSION],"requested":version})),
                ),
                None => rpc_error(
                    request_id,
                    -32602,
                    &eda_i18n::text("server/discover requer a versão MCP 2026-07-28."),
                    None,
                ),
            });
        }

        let modern = match protocol_version(&params) {
            Some(MODERN_VERSION) if has_client_capabilities(&params) => true,
            Some(MODERN_VERSION) => {
                return id.map(|request_id| {
                    rpc_error(
                        request_id,
                        -32600,
                        &eda_i18n::text("Solicitação MCP moderna sem clientCapabilities."),
                        None,
                    )
                });
            }
            Some(version) => {
                return id.map(|request_id| {
                    rpc_error(
                        request_id,
                        -32022,
                        &eda_i18n::text("Versão de protocolo MCP não suportada."),
                        Some(json!({"supported":[MODERN_VERSION],"requested":version})),
                    )
                });
            }
            None if self.legacy_initialized
                || (method == "notifications/initialized" && self.legacy_awaiting_initialized) =>
            {
                false
            }
            None => {
                if is_notification {
                    return None;
                }
                return id.map(|request_id| {
                    rpc_error(
                        request_id,
                        -32601,
                        &eda_i18n::text(
                            "Use server/discover ou initialize antes de enviar solicitações MCP.",
                        ),
                        None,
                    )
                });
            }
        };

        if method == "notifications/initialized" {
            if !modern && self.legacy_awaiting_initialized {
                self.legacy_initialized = true;
                self.legacy_awaiting_initialized = false;
            }
            return None;
        }
        if method == "notifications/cancelled" || is_notification {
            return None;
        }
        let request_id = id.unwrap_or(Value::Null);

        let result = match method {
            "ping" => json!({}),
            "tools/list" => self.list_tools(),
            "tools/call" => match self.call_tool(&params) {
                Ok(result) => tool_success(result),
                Err((_, message, data)) => json!({
                    "content":[{"type":"text","text":message.as_str()}],
                    "isError":true,
                    "structuredContent":{"error":message,"details":data}
                }),
            },
            _ => {
                return Some(rpc_error(
                    request_id,
                    -32601,
                    &eda_i18n::text("Método MCP não encontrado."),
                    None,
                ));
            }
        };
        Some(rpc_response(request_id, result, modern))
    }

    fn initialize(&mut self, id: Value, params: &Value) -> Value {
        let Some(requested) = params.get("protocolVersion").and_then(Value::as_str) else {
            return rpc_error(
                id,
                -32602,
                &eda_i18n::text("initialize requer protocolVersion como texto."),
                None,
            );
        };
        if requested != LEGACY_VERSION {
            return rpc_error(
                id,
                -32602,
                &eda_i18n::text("Versão de protocolo MCP não suportada."),
                Some(json!({"supported":[LEGACY_VERSION],"requested":requested})),
            );
        }
        self.legacy_awaiting_initialized = true;
        self.legacy_initialized = false;
        json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{
                "protocolVersion":LEGACY_VERSION,
                "capabilities":{"tools":{"listChanged":false}},
                "serverInfo":{"name":"ProtoCraft","title":"ProtoCraft EDA","version":env!("CARGO_PKG_VERSION")},
                "instructions":eda_i18n::text("Servidor local do ProtoCraft. Consulte projeto e revisão antes de enviar comandos; execute uma edição por vez com expected_revision e salve explicitamente.")
            }
        })
    }

    fn discover(&self) -> Value {
        json!({
            "resultType":"complete",
            "supportedVersions":[MODERN_VERSION],
            "capabilities":{"tools":{}},
            "_meta":{"io.modelcontextprotocol/serverInfo":{"name":"ProtoCraft","title":"ProtoCraft EDA","version":env!("CARGO_PKG_VERSION")}},
            "instructions":eda_i18n::text("Servidor local do ProtoCraft. Consulte projeto e revisão antes de enviar comandos; execute uma edição por vez com expected_revision e salve explicitamente."),
            "ttlMs":0,
            "cacheScope":"private"
        })
    }

    fn list_tools(&self) -> Value {
        let action_schema = eda_automation::batch_action_schema();
        let tools = vec![
            tool(
                "protocraft_project_info",
                "Informações do projeto",
                "Retorna o nome do projeto, suas dimensões, revisão atual e estado de undo/redo.",
                json!({"type":"object","additionalProperties":false}),
                json!({"readOnlyHint":true,"idempotentHint":true}),
            ),
            tool(
                "protocraft_project_snapshot",
                "Snapshot do projeto",
                "Retorna o projeto ProtoCraft completo em JSON para inspeção.",
                json!({"type":"object","additionalProperties":false}),
                json!({"readOnlyHint":true,"idempotentHint":true}),
            ),
            tool(
                "protocraft_project_validate",
                "Validar projeto",
                "Valida as invariantes do projeto sem alterar o documento.",
                json!({"type":"object","additionalProperties":false}),
                json!({"readOnlyHint":true,"idempotentHint":true}),
            ),
            tool(
                "protocraft_netlist",
                "Netlist e ERC",
                "Recalcula a netlist e os diagnósticos ERC do projeto atual.",
                json!({"type":"object","additionalProperties":false}),
                json!({"readOnlyHint":true,"idempotentHint":true}),
            ),
            tool(
                "protocraft_command_list",
                "Catálogo de comandos",
                "Lista os comandos tipados de esquema e PCB aceitos pela CLI e pelo MCP.",
                json!({"type":"object","additionalProperties":false}),
                json!({"readOnlyHint":true,"idempotentHint":true}),
            ),
            tool(
                "protocraft_execute",
                "Executar comandos atômicos",
                "Executa até 1.000 comandos como uma transação indivisível. expected_revision deve ser a revisão informada pela leitura mais recente; em caso de conflito, releia o projeto.",
                json!({"type":"object","properties":{"expected_revision":{"type":"integer","minimum":0,"description":eda_i18n::text("Revisão do projeto lida pelo cliente antes da edição.")},"actions":{"type":"array","minItems":1,"maxItems":MAX_ACTIONS,"items":action_schema}},"required":["expected_revision","actions"],"additionalProperties":false}),
                json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}),
            ),
            tool(
                "protocraft_undo",
                "Desfazer",
                "Desfaz a edição mais recente se a revisão ainda for a esperada.",
                json!({"type":"object","properties":{"expected_revision":{"type":"integer","minimum":0}},"required":["expected_revision"],"additionalProperties":false}),
                json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}),
            ),
            tool(
                "protocraft_redo",
                "Refazer",
                "Refaz a edição desfeita mais recente se a revisão ainda for a esperada.",
                json!({"type":"object","properties":{"expected_revision":{"type":"integer","minimum":0}},"required":["expected_revision"],"additionalProperties":false}),
                json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}),
            ),
            tool(
                "protocraft_save",
                "Salvar projeto",
                "Salva o estado atual no arquivo .protocraft associado a este processo e preserva um backup .bak.",
                json!({"type":"object","additionalProperties":false}),
                json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true}),
            ),
            tool(
                "protocraft_device_search",
                "Buscar componentes físicos",
                "Busca o catálogo offline por termo, fabricante, família, encapsulamento, função e limites de pinos ou dimensões.",
                json!({"type":"object","properties":{"query":{"type":"string","maxLength":128,"description":eda_i18n::text("Termos em português ou inglês, número de peça, função ou encapsulamento.")},"manufacturer":{"type":"string","maxLength":100,"description":eda_i18n::text("Fabricante ou marca do componente.")},"family":{"type":"string","maxLength":100,"description":eda_i18n::text("Família funcional, por exemplo STM32 ou ESP32.")},"form":{"type":"string","maxLength":100,"description":eda_i18n::text("Encapsulamento físico, por exemplo QFN ou TQFP.")},"feature":{"type":"string","maxLength":100,"description":eda_i18n::text("Característica encontrada nos dados catalogados do componente.")},"min_pins":{"type":"integer","minimum":1,"description":eda_i18n::text("Quantidade mínima de pinos.")},"max_pins":{"type":"integer","minimum":1,"description":eda_i18n::text("Quantidade máxima de pinos.")},"max_body_dimension_nm":{"type":"integer","minimum":1,"description":eda_i18n::text("Dimensão máxima do corpo em nanômetros.")},"limit":{"type":"integer","minimum":1,"maximum":100,"description":eda_i18n::text("Número máximo de resultados retornados, de 1 a 100.")},"offset":{"type":"integer","minimum":0,"description":eda_i18n::text("Índice do primeiro resultado, começando em zero.")}},"additionalProperties":false}),
                json!({"readOnlyHint":true,"idempotentHint":true}),
            ),
            tool(
                "protocraft_pcb_check",
                "Verificar PCB",
                "Executa o DRC nativo de cobre, furos, bordas, posicionamento e conexões pendentes.",
                json!({"type":"object","additionalProperties":false}),
                json!({"readOnlyHint":true,"idempotentHint":true}),
            ),
            tool(
                "protocraft_autoroute",
                "Autorrotear conexões pendentes",
                "Calcula rotas nas duas camadas, valida o DRC e aplica o plano como uma única edição do Engine. Use expected_revision da leitura atual.",
                json!({"type":"object","properties":{"expected_revision":{"type":"integer","minimum":0,"description":eda_i18n::text("Revisão do projeto lida pelo cliente antes da edição.")},"grid_nm":{"type":"integer","minimum":50000,"maximum":10000000,"default":250000,"description":eda_i18n::text("Passo da malha em nanômetros; 250000 nm correspondem a 0,25 mm.")},"track_width_nm":{"type":"integer","minimum":1000,"maximum":10000000,"default":250000,"description":eda_i18n::text("Largura de trilha em nanômetros; 250000 nm correspondem a 0,25 mm.")}},"required":["expected_revision"],"additionalProperties":false}),
                json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}),
            ),
        ];
        json!({"resultType":"complete","tools":tools,"ttlMs":0,"cacheScope":"private"})
    }

    fn call_tool(&mut self, params: &Value) -> Result<Value, (i32, String, Option<Value>)> {
        let name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
            invalid_params(eda_i18n::text("tools/call requer o nome da ferramenta."))
        })?;
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !args.is_object() {
            return Err(invalid_params(eda_i18n::text(
                "Os argumentos da ferramenta devem ser um objeto JSON.",
            )));
        }
        let allowed_arguments: &[&str] = match name {
            "protocraft_project_info"
            | "protocraft_project_snapshot"
            | "protocraft_project_validate"
            | "protocraft_netlist"
            | "protocraft_command_list"
            | "protocraft_save"
            | "protocraft_pcb_check" => &[],
            "protocraft_execute" => &["expected_revision", "actions"],
            "protocraft_undo" | "protocraft_redo" => &["expected_revision"],
            "protocraft_device_search" => &[
                "query",
                "manufacturer",
                "family",
                "form",
                "feature",
                "min_pins",
                "max_pins",
                "max_body_dimension_nm",
                "limit",
                "offset",
            ],
            "protocraft_autoroute" => &["expected_revision", "grid_nm", "track_width_nm"],
            _ => &[],
        };
        validate_argument_keys(&args, allowed_arguments)?;
        let result = match name {
            "protocraft_project_info" => self.project_info(),
            "protocraft_project_snapshot" => {
                json!({"revision":self.engine.revision(),"project":self.engine.project()})
            }
            "protocraft_project_validate" => {
                self.engine.project().validate().map_err(domain_error)?;
                json!({"valid":true,"revision":self.engine.revision()})
            }
            "protocraft_netlist" => {
                let project = self.engine.project();
                let netlist = eda_netlist::build_netlist(project);
                let diagnostics = eda_netlist::erc(project, &netlist);
                let diagnostics: Vec<_> = diagnostics
                    .iter()
                    .map(|diagnostic| json!({"code":diagnostic.code,"severity":format!("{:?}",diagnostic.severity).to_lowercase(),"message":eda_i18n::text(&diagnostic.message),"objects":diagnostic.objects}))
                    .collect();
                json!({"revision":self.engine.revision(),"nets":netlist.nets,"diagnostics":diagnostics})
            }
            "protocraft_command_list" => {
                let commands: Vec<_> = COMMANDS
                    .iter()
                    .map(|command| json!({"id":command.id,"description":eda_i18n::text(command.description),"params":command.params,"input_schema":eda_automation::command_schema(command.id)}))
                    .collect();
                json!({"commands":commands})
            }
            "protocraft_device_search" => self.device_search(&args)?,
            "protocraft_pcb_check" => {
                let report = eda_pcb::check(self.engine.project()).map_err(domain_error)?;
                let errors = report.errors();
                let warnings = report.diagnostics.len().saturating_sub(errors);
                let diagnostics: Vec<_> = report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| json!({"severity":diagnostic.severity,"code":diagnostic.code,"message":eda_i18n::text(&diagnostic.message),"components":diagnostic.components,"position":diagnostic.position}))
                    .collect();
                json!({"revision":self.engine.revision(),"summary":{"errors":errors,"warnings":warnings,"airwires":report.airwires.len(),"placed":report.placed,"pads":report.pads,"routed_length_mm":report.routed_length_mm},"diagnostics":diagnostics,"airwires":report.airwires})
            }
            "protocraft_autoroute" => {
                let expected = required_u64(&args, "expected_revision")?;
                self.check_revision(expected).map_err(domain_error)?;
                let config = eda_pcb::AutorouteConfig {
                    grid_nm: optional_i64(&args, "grid_nm", 50_000, 10_000_000)?.unwrap_or(250_000),
                    track_width_nm: optional_i64(&args, "track_width_nm", 1_000, 10_000_000)?
                        .unwrap_or(250_000),
                    max_total_expansions: 600_000,
                    ..Default::default()
                };
                let plan =
                    eda_pcb::autoroute(self.engine.project(), &config).map_err(domain_error)?;
                let routed = plan.stats.routed_connections;
                let stats = serde_json::to_value(&plan.stats).map_err(|error| {
                    invalid_params(eda_i18n::format(
                        "Não foi possível serializar o resultado: {error}",
                        &[("error", error.to_string())],
                    ))
                })?;
                let failures: Vec<_> = plan
                    .failures
                    .iter()
                    .map(|failure| json!({"net":failure.net,"start":failure.start,"end":failure.end,"reason":eda_i18n::text(&failure.reason)}))
                    .collect();
                let command_count = plan.commands.len();
                if command_count > 0 {
                    self.engine
                        .execute_at_revision(expected, Command::Batch(plan.commands))
                        .map_err(domain_error)?;
                }
                json!({"ok":true,"applied":command_count>0,"commands":command_count,"routed_connections":routed,"revision":self.engine.revision(),"stats":stats,"failures":failures})
            }
            "protocraft_execute" => {
                let expected = required_u64(&args, "expected_revision")?;
                let actions = args
                    .get("actions")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid_params(eda_i18n::text("actions deve ser uma lista.")))?;
                if actions.is_empty() || actions.len() > MAX_ACTIONS {
                    return Err(invalid_params(eda_i18n::format(
                        "actions deve conter de 1 a {max} itens.",
                        &[("max", MAX_ACTIONS.to_string())],
                    )));
                }
                let commands = actions
                    .iter()
                    .cloned()
                    .map(serde_json::from_value::<Action>)
                    .map(|action| {
                        action.map_err(|error| {
                            invalid_params(eda_i18n::format(
                                "Ação inválida: {error}",
                                &[("error", error.to_string())],
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(Action::into_command)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| invalid_params(eda_i18n::text(&error)))?;
                self.engine
                    .execute_at_revision(expected, Command::Batch(commands))
                    .map_err(domain_error)?;
                json!({"ok":true,"actions":actions.len(),"revision":self.engine.revision()})
            }
            "protocraft_undo" => {
                let expected = required_u64(&args, "expected_revision")?;
                self.check_revision(expected).map_err(domain_error)?;
                let changed = self.engine.undo();
                json!({"changed":changed,"revision":self.engine.revision()})
            }
            "protocraft_redo" => {
                let expected = required_u64(&args, "expected_revision")?;
                self.check_revision(expected).map_err(domain_error)?;
                let changed = self.engine.redo();
                json!({"changed":changed,"revision":self.engine.revision()})
            }
            "protocraft_save" => {
                eda_storage::save(&self.project_path, self.engine.project())
                    .map_err(domain_error)?;
                json!({"saved":true,"path":self.project_path,"revision":self.engine.revision()})
            }
            _ => {
                return Err((
                    -32602,
                    eda_i18n::text("Ferramenta ProtoCraft desconhecida."),
                    None,
                ));
            }
        };
        Ok(result)
    }

    fn project_info(&self) -> Value {
        let project = self.engine.project();
        json!({
            "name":project.name,
            "format_version":project.format_version,
            "revision":self.engine.revision(),
            "components":project.components.len(),
            "wires":project.wires.len(),
            "junctions":project.junctions.len(),
            "labels":project.labels.len(),
            "has_board":project.board.is_some(),
            "can_undo":self.engine.can_undo(),
            "can_redo":self.engine.can_redo()
        })
    }

    fn device_search(&self, args: &Value) -> Result<Value, (i32, String, Option<Value>)> {
        let query = optional_string(args, "query", 128)?.unwrap_or_default();
        let filters = eda_library::DeviceSearchFilters {
            manufacturer: optional_string(args, "manufacturer", 100)?,
            family: optional_string(args, "family", 100)?,
            form: optional_string(args, "form", 100)?,
            feature: optional_string(args, "feature", 100)?,
            min_pins: optional_usize(args, "min_pins", 1, usize::MAX)?,
            max_pins: optional_usize(args, "max_pins", 1, usize::MAX)?,
            max_body_dimension_nm: optional_i64(args, "max_body_dimension_nm", 1, i64::MAX)?,
        };
        if filters
            .min_pins
            .zip(filters.max_pins)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            return Err(invalid_params(eda_i18n::text(
                "min_pins não pode ser maior que max_pins.",
            )));
        }
        let limit = optional_usize(args, "limit", 1, 100)?.unwrap_or(25);
        let offset = optional_usize(args, "offset", 0, 1_000_000)?.unwrap_or(0);
        let hits = eda_library::catalog()
            .map_err(domain_error)?
            .search_faceted(&query, &filters);
        let total = hits.len();
        let devices = hits
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|hit| {
                json!({
                    "device":hit.record,
                    "pin_count":hit.pin_count,
                    "width_nm":hit.width_nm,
                    "height_nm":hit.height_nm
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "query":query,
            "total":total,
            "offset":offset,
            "limit":limit,
            "devices":devices
        }))
    }

    fn check_revision(&self, expected: u64) -> Result<(), CoreError> {
        if self.engine.revision() == expected {
            Ok(())
        } else {
            Err(CoreError::RevisionConflict {
                expected,
                actual: self.engine.revision(),
            })
        }
    }
}

fn tool(
    name: &str,
    title: &str,
    description: &str,
    input_schema: Value,
    annotations: Value,
) -> Value {
    json!({
        "name":name,
        "title":eda_i18n::text(title),
        "description":eda_i18n::text(description),
        "inputSchema":input_schema,
        "annotations":annotations
    })
}

fn protocol_version(params: &Value) -> Option<&str> {
    params
        .get("_meta")?
        .get("io.modelcontextprotocol/protocolVersion")?
        .as_str()
}

fn has_client_capabilities(params: &Value) -> bool {
    params
        .get("_meta")
        .and_then(|meta| meta.get("io.modelcontextprotocol/clientCapabilities"))
        .is_some_and(Value::is_object)
}

fn validate_argument_keys(
    args: &Value,
    allowed: &[&str],
) -> Result<(), (i32, String, Option<Value>)> {
    let Some(args) = args.as_object() else {
        return Err(invalid_params(eda_i18n::text(
            "Os argumentos da ferramenta devem ser um objeto JSON.",
        )));
    };
    if let Some(key) = args.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid_params(eda_i18n::format(
            "Parâmetro desconhecido: {key}.",
            &[("key", key.to_owned())],
        )));
    }
    Ok(())
}

fn optional_string(
    args: &Value,
    key: &str,
    max_chars: usize,
) -> Result<Option<String>, (i32, String, Option<Value>)> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Err(invalid_params(eda_i18n::format(
            "{key} deve ser texto.",
            &[("key", key.to_owned())],
        )));
    };
    if value.chars().count() > max_chars {
        return Err(invalid_params(eda_i18n::format(
            "{key} excede o limite de {max} caracteres.",
            &[("key", key.to_owned()), ("max", max_chars.to_string())],
        )));
    }
    Ok(Some(value.to_owned()))
}

fn optional_i64(
    args: &Value,
    key: &str,
    minimum: i64,
    maximum: i64,
) -> Result<Option<i64>, (i32, String, Option<Value>)> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_i64() else {
        return Err(invalid_params(eda_i18n::format(
            "{key} deve ser um inteiro entre {min} e {max}.",
            &[
                ("key", key.to_owned()),
                ("min", minimum.to_string()),
                ("max", maximum.to_string()),
            ],
        )));
    };
    if !(minimum..=maximum).contains(&value) {
        return Err(invalid_params(eda_i18n::format(
            "{key} deve estar entre {min} e {max}.",
            &[
                ("key", key.to_owned()),
                ("min", minimum.to_string()),
                ("max", maximum.to_string()),
            ],
        )));
    }
    Ok(Some(value))
}

fn optional_usize(
    args: &Value,
    key: &str,
    minimum: usize,
    maximum: usize,
) -> Result<Option<usize>, (i32, String, Option<Value>)> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };
    let parsed = value.as_u64().and_then(|value| usize::try_from(value).ok());
    let Some(value) = parsed.filter(|value| (minimum..=maximum).contains(value)) else {
        return Err(invalid_params(eda_i18n::format(
            "{key} deve ser um inteiro entre {min} e {max}.",
            &[
                ("key", key.to_owned()),
                ("min", minimum.to_string()),
                ("max", maximum.to_string()),
            ],
        )));
    };
    Ok(Some(value))
}

fn required_u64(value: &Value, key: &str) -> Result<u64, (i32, String, Option<Value>)> {
    value.get(key).and_then(Value::as_u64).ok_or_else(|| {
        invalid_params(eda_i18n::format(
            "{key} deve ser um inteiro não negativo.",
            &[("key", key.to_owned())],
        ))
    })
}

fn invalid_params(message: String) -> (i32, String, Option<Value>) {
    (-32602, message, None)
}

fn tool_success(structured_content: Value) -> Value {
    let text = serde_json::to_string(&structured_content).unwrap_or_else(|_| "{}".to_owned());
    json!({
        "content":[{"type":"text","text":text}],
        "structuredContent":structured_content,
        "isError":false
    })
}

fn domain_error(error: impl std::fmt::Display) -> (i32, String, Option<Value>) {
    (-32001, eda_i18n::text(&error.to_string()), None)
}

fn rpc_response(id: Value, mut result: Value, modern: bool) -> Value {
    if modern {
        if result.get("resultType").is_none() {
            result["resultType"] = Value::String("complete".into());
        }
        if let Some(result) = result.as_object_mut() {
            let meta = result
                .entry("_meta")
                .or_insert_with(|| Value::Object(Default::default()));
            if let Some(meta) = meta.as_object_mut() {
                meta.entry("io.modelcontextprotocol/serverInfo")
                    .or_insert_with(|| {
                        json!({
                            "name":"ProtoCraft",
                            "title":"ProtoCraft EDA",
                            "version":env!("CARGO_PKG_VERSION")
                        })
                    });
            }
        }
    }
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

fn rpc_error(id: Value, code: i32, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({"code":code,"message":message});
    if let Some(data) = data {
        error["data"] = data;
    }
    json!({"jsonrpc":"2.0","id":id,"error":error})
}
