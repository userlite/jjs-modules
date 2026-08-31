//! Node `sqlite3` compatibility surface backed by host-owned session databases.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, MODULE_API_VERSION,
    ModuleCallResult, ModuleContext, ModuleContinuation, ModuleError, ModuleFunctionKey,
    ModuleIdentity, ModuleManifest, ModuleObjectKind, ModuleValueKind, NativeModule, ValueHandle,
};

pub const SQLITE_REQUEST: &str = "jjs:sqlite/request";
pub const SQLITE_OPEN: &str = "jjs:sqlite/open";
pub const SQLITE_RUN: &str = "jjs:sqlite/run";
pub const SQLITE_GET: &str = "jjs:sqlite/get";
pub const SQLITE_ALL: &str = "jjs:sqlite/all";
pub const SQLITE_EXEC: &str = "jjs:sqlite/exec";
pub const SQLITE_PREPARE: &str = "jjs:sqlite/prepare";
pub const SQLITE_CLOSE: &str = "jjs:sqlite/close";
pub const SQLITE_STATEMENT_RUN: &str = "jjs:sqlite/statement/run";
pub const SQLITE_STATEMENT_GET: &str = "jjs:sqlite/statement/get";
pub const SQLITE_STATEMENT_ALL: &str = "jjs:sqlite/statement/all";
pub const SQLITE_STATEMENT_BIND: &str = "jjs:sqlite/statement/bind";
pub const SQLITE_STATEMENT_RESET: &str = "jjs:sqlite/statement/reset";
pub const SQLITE_STATEMENT_FINALIZE: &str = "jjs:sqlite/statement/finalize";

pub const OPEN_READONLY: u32 = 1;
pub const OPEN_READWRITE: u32 = 2;
pub const OPEN_CREATE: u32 = 4;

const DATABASE: ModuleFunctionKey = ModuleFunctionKey(1);
const VERBOSE: ModuleFunctionKey = ModuleFunctionKey(2);
const DB_RUN: ModuleFunctionKey = ModuleFunctionKey(10);
const DB_GET: ModuleFunctionKey = ModuleFunctionKey(11);
const DB_ALL: ModuleFunctionKey = ModuleFunctionKey(12);
const DB_EACH: ModuleFunctionKey = ModuleFunctionKey(13);
const DB_EXEC: ModuleFunctionKey = ModuleFunctionKey(14);
const DB_PREPARE: ModuleFunctionKey = ModuleFunctionKey(15);
const DB_CLOSE: ModuleFunctionKey = ModuleFunctionKey(16);
const DB_SERIALIZE: ModuleFunctionKey = ModuleFunctionKey(17);
const DB_PARALLELIZE: ModuleFunctionKey = ModuleFunctionKey(18);
const STMT_RUN: ModuleFunctionKey = ModuleFunctionKey(30);
const STMT_GET: ModuleFunctionKey = ModuleFunctionKey(31);
const STMT_ALL: ModuleFunctionKey = ModuleFunctionKey(32);
const STMT_BIND: ModuleFunctionKey = ModuleFunctionKey(33);
const STMT_RESET: ModuleFunctionKey = ModuleFunctionKey(34);
const STMT_FINALIZE: ModuleFunctionKey = ModuleFunctionKey(35);

const DATABASE_OBJECT: ModuleObjectKind = ModuleObjectKind(1);
const STATEMENT_OBJECT: ModuleObjectKind = ModuleObjectKind(2);
const HANDLE: u32 = 1;
const DATABASE_OWNER: u32 = 2;
const EXPORTS: u32 = 3;

const OPEN_COMPLETE: u32 = 1;
const RUN_COMPLETE: u32 = 2;
const GET_COMPLETE: u32 = 3;
const ALL_COMPLETE: u32 = 4;
const EACH_COMPLETE: u32 = 5;
const EXEC_COMPLETE: u32 = 6;
const PREPARE_COMPLETE: u32 = 7;
const CLOSE_COMPLETE: u32 = 8;
const STATEMENT_COMPLETE: u32 = 9;
const STATEMENT_GET_COMPLETE: u32 = 10;
const STATEMENT_ALL_COMPLETE: u32 = 11;
const STATEMENT_FINALIZE_COMPLETE: u32 = 12;

const DEFAULT_OPEN_MODE: f64 = (OPEN_READWRITE | OPEN_CREATE) as f64;

pub struct Sqlite3Module {
    manifest: ModuleManifest,
}

impl Default for Sqlite3Module {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.sqlite3".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-sqlite3-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["sqlite3".into()],
                capabilities: vec![HostCapabilityDescriptor {
                    id: SQLITE_REQUEST.into(),
                    contract_version: 1,
                    completion: CompletionMode::Yield,
                    schema: "jjs.sqlite.request.v1".into(),
                }],
                dependencies: vec![],
                function_keys: vec![
                    DATABASE.0,
                    VERBOSE.0,
                    DB_RUN.0,
                    DB_GET.0,
                    DB_ALL.0,
                    DB_EACH.0,
                    DB_EXEC.0,
                    DB_PREPARE.0,
                    DB_CLOSE.0,
                    DB_SERIALIZE.0,
                    DB_PARALLELIZE.0,
                    STMT_RUN.0,
                    STMT_GET.0,
                    STMT_ALL.0,
                    STMT_BIND.0,
                    STMT_RESET.0,
                    STMT_FINALIZE.0,
                ],
                object_kind_keys: vec![DATABASE_OBJECT.0, STATEMENT_OBJECT.0],
                deterministic_resources: vec![],
            },
        }
    }
}

fn thrown(message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: "TypeError".into(),
        message: message.into(),
    }
}

fn attach(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
    key: ModuleFunctionKey,
) -> Result<(), ModuleError> {
    let function = context.function(key)?;
    context.set_property(object, name, function)
}

fn null(context: &mut dyn ModuleContext) -> Result<ValueHandle, ModuleError> {
    let encoded = context.string("null")?;
    context.json_parse(encoded)
}

fn request(
    context: &mut dyn ModuleContext,
    operation: &str,
    mut arguments: Vec<ValueHandle>,
    continuation: u32,
    state: Vec<ValueHandle>,
) -> Result<ModuleCallResult, ModuleError> {
    arguments.insert(0, context.string(operation)?);
    context.request_host(
        HostRequestSpec {
            capability: SQLITE_REQUEST.into(),
            operation: operation.into(),
            arguments,
        },
        ModuleContinuation(continuation),
        state,
        false,
    )
}

fn callback_or_undefined(
    context: &mut dyn ModuleContext,
    args: &[ValueHandle],
) -> ValueHandle {
    args.last()
        .copied()
        .filter(|value| context.is_callable(*value))
        .unwrap_or_else(|| context.undefined())
}

fn args_without_callback<'a>(
    context: &dyn ModuleContext,
    args: &'a [ValueHandle],
) -> &'a [ValueHandle] {
    if args.last().is_some_and(|value| context.is_callable(*value)) {
        &args[..args.len() - 1]
    } else {
        args
    }
}

fn encode_params(
    context: &mut dyn ModuleContext,
    params: &[ValueHandle],
) -> Result<ValueHandle, ModuleError> {
    if params.len() == 1
        && matches!(
            context.value_kind(params[0])?,
            ModuleValueKind::Array | ModuleValueKind::Object
        )
    {
        return context.json_stringify(params[0]);
    }
    let values = context.array()?;
    for value in params {
        context.array_push(values, *value)?;
    }
    context.json_stringify(values)
}

fn database(context: &mut dyn ModuleContext) -> Result<ValueHandle, ModuleError> {
    let value = context.module_object(DATABASE_OBJECT)?;
    for (name, key) in [
        ("run", DB_RUN),
        ("get", DB_GET),
        ("all", DB_ALL),
        ("each", DB_EACH),
        ("exec", DB_EXEC),
        ("prepare", DB_PREPARE),
        ("close", DB_CLOSE),
        ("serialize", DB_SERIALIZE),
        ("parallelize", DB_PARALLELIZE),
    ] {
        attach(context, value, name, key)?;
    }
    Ok(value)
}

fn statement(
    context: &mut dyn ModuleContext,
    owner: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    let value = context.module_object(STATEMENT_OBJECT)?;
    context.set_private(value, DATABASE_OWNER, owner)?;
    for (name, key) in [
        ("run", STMT_RUN),
        ("get", STMT_GET),
        ("all", STMT_ALL),
        ("bind", STMT_BIND),
        ("reset", STMT_RESET),
        ("finalize", STMT_FINALIZE),
    ] {
        attach(context, value, name, key)?;
    }
    Ok(value)
}

fn invoke_callback(
    context: &mut dyn ModuleContext,
    callback: ValueHandle,
    receiver: ValueHandle,
    args: &[ValueHandle],
) -> Result<(), ModuleError> {
    if context.is_callable(callback) {
        context.call(callback, receiver, args)?;
    }
    Ok(())
}

fn error_value(
    context: &mut dyn ModuleContext,
    message: &str,
) -> Result<ValueHandle, ModuleError> {
    let error = context.object()?;
    let name = context.string("Error")?;
    let code = message
        .split_once(':')
        .map_or("SQLITE_ERROR", |(code, _)| code.trim());
    let code = context.string(code)?;
    let message = context.string(message)?;
    context.set_property(error, "name", name)?;
    context.set_property(error, "code", code)?;
    context.set_property(error, "message", message)?;
    Ok(error)
}

fn complete_error(
    context: &mut dyn ModuleContext,
    receiver: ValueHandle,
    callback: ValueHandle,
    message: String,
) -> Result<ModuleCallResult, ModuleError> {
    if context.is_callable(callback) {
        let error = error_value(context, &message)?;
        invoke_callback(context, callback, receiver, &[error])?;
        Ok(ModuleCallResult::Return(receiver))
    } else {
        Ok(ModuleCallResult::Throw {
            name: "SqliteError".into(),
            message,
        })
    }
}

fn decoded_completion(
    context: &mut dyn ModuleContext,
    completion: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    let encoded = context.as_string(completion)?;
    let encoded = context.string(&encoded)?;
    context.json_parse(encoded)
}

fn require_handle(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    kind: &str,
) -> Result<Result<ValueHandle, ModuleCallResult>, ModuleError> {
    let handle = context.get_private(object, HANDLE)?;
    if context.value_kind(handle)? != ModuleValueKind::Number {
        return Ok(Err(thrown(format!("SQLite {kind} is not open"))));
    }
    Ok(Ok(handle))
}

impl NativeModule for Sqlite3Module {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        let constructor = context.function(DATABASE)?;
        let verbose = context.function(VERBOSE)?;
        context.set_private(verbose, EXPORTS, exports)?;
        context.set_property(exports, "Database", constructor)?;
        context.set_property(exports, "verbose", verbose)?;
        for (name, value) in [
            ("OPEN_READONLY", OPEN_READONLY),
            ("OPEN_READWRITE", OPEN_READWRITE),
            ("OPEN_CREATE", OPEN_CREATE),
        ] {
            let value = context.number(f64::from(value))?;
            context.set_property(exports, name, value)?;
        }
        Ok(ModuleCallResult::Return(exports))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        match key {
            VERBOSE => {
                let exports = context.get_private(callee, EXPORTS)?;
                Ok(ModuleCallResult::Return(exports))
            }
            DATABASE => {
                if args.is_empty() || args.len() > 3 {
                    return Ok(thrown(
                        "sqlite3.Database requires a filename, optional mode, and optional callback",
                    ));
                }
                if context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(thrown("sqlite3.Database filename must be a string"));
                }
                let callback = callback_or_undefined(context, args);
                let mode = args
                    .get(1)
                    .copied()
                    .filter(|value| context.value_kind(*value).ok() == Some(ModuleValueKind::Number));
                let mode = match mode {
                    Some(mode) => mode,
                    None => context.number(DEFAULT_OPEN_MODE)?,
                };
                let db = database(context)?;
                request(
                    context,
                    SQLITE_OPEN,
                    vec![args[0], mode],
                    OPEN_COMPLETE,
                    vec![db, callback],
                )
            }
            DB_RUN | DB_GET | DB_ALL | DB_EACH => {
                if args.is_empty() || context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(thrown("SQLite query requires a SQL string"));
                }
                let handle = match require_handle(context, receiver, "database")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let callback = callback_or_undefined(context, args);
                let without_callback = args_without_callback(context, args);
                let params = encode_params(context, &without_callback[1..])?;
                if key == DB_EACH {
                    let callbacks = args
                        .iter()
                        .rev()
                        .copied()
                        .filter(|value| context.is_callable(*value))
                        .collect::<Vec<_>>();
                    if callbacks.is_empty() {
                        return Ok(thrown("SQLite each requires a row callback"));
                    }
                    let row_callback = *callbacks.last().expect("callbacks is non-empty");
                    let complete_callback = callbacks
                        .first()
                        .copied()
                        .filter(|_| callbacks.len() > 1)
                        .unwrap_or_else(|| context.undefined());
                    return request(
                        context,
                        SQLITE_ALL,
                        vec![handle, args[0], params],
                        EACH_COMPLETE,
                        vec![receiver, row_callback, complete_callback],
                    );
                }
                let (operation, continuation) = match key {
                    DB_RUN => (SQLITE_RUN, RUN_COMPLETE),
                    DB_GET => (SQLITE_GET, GET_COMPLETE),
                    DB_ALL => (SQLITE_ALL, ALL_COMPLETE),
                    _ => unreachable!(),
                };
                request(
                    context,
                    operation,
                    vec![handle, args[0], params],
                    continuation,
                    vec![receiver, callback],
                )
            }
            DB_EXEC => {
                if args.is_empty() || context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(thrown("SQLite exec requires a SQL string"));
                }
                let handle = match require_handle(context, receiver, "database")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let callback = callback_or_undefined(context, args);
                request(
                    context,
                    SQLITE_EXEC,
                    vec![handle, args[0]],
                    EXEC_COMPLETE,
                    vec![receiver, callback],
                )
            }
            DB_PREPARE => {
                if args.is_empty() || context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(thrown("SQLite prepare requires a SQL string"));
                }
                let handle = match require_handle(context, receiver, "database")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let callback = callback_or_undefined(context, args);
                let without_callback = args_without_callback(context, args);
                let params = encode_params(context, &without_callback[1..])?;
                let statement = statement(context, receiver)?;
                request(
                    context,
                    SQLITE_PREPARE,
                    vec![handle, args[0], params],
                    PREPARE_COMPLETE,
                    vec![statement, callback],
                )
            }
            DB_CLOSE => {
                if args.len() > 1 {
                    return Ok(thrown("SQLite close accepts only an optional callback"));
                }
                let handle = match require_handle(context, receiver, "database")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let callback = callback_or_undefined(context, args);
                request(
                    context,
                    SQLITE_CLOSE,
                    vec![handle],
                    CLOSE_COMPLETE,
                    vec![receiver, callback],
                )
            }
            DB_SERIALIZE | DB_PARALLELIZE => {
                if args.len() > 1 || args.first().is_some_and(|value| !context.is_callable(*value)) {
                    return Ok(thrown(
                        "SQLite serialize and parallelize accept only an optional callback",
                    ));
                }
                if let Some(callback) = args.first() {
                    let undefined = context.undefined();
                    context.call(*callback, receiver, &[])?;
                    let _ = undefined;
                }
                Ok(ModuleCallResult::Return(receiver))
            }
            STMT_RUN | STMT_GET | STMT_ALL | STMT_BIND => {
                let handle = match require_handle(context, receiver, "statement")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let callback = callback_or_undefined(context, args);
                let without_callback = args_without_callback(context, args);
                let params = encode_params(context, without_callback)?;
                let (operation, continuation) = match key {
                    STMT_RUN => (SQLITE_STATEMENT_RUN, STATEMENT_COMPLETE),
                    STMT_GET => (SQLITE_STATEMENT_GET, STATEMENT_GET_COMPLETE),
                    STMT_ALL => (SQLITE_STATEMENT_ALL, STATEMENT_ALL_COMPLETE),
                    STMT_BIND => (SQLITE_STATEMENT_BIND, STATEMENT_COMPLETE),
                    _ => unreachable!(),
                };
                request(
                    context,
                    operation,
                    vec![handle, params],
                    continuation,
                    vec![receiver, callback],
                )
            }
            STMT_RESET | STMT_FINALIZE => {
                if args.len() > 1 {
                    return Ok(thrown("SQLite statement method accepts only an optional callback"));
                }
                let handle = match require_handle(context, receiver, "statement")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let callback = callback_or_undefined(context, args);
                let (operation, continuation) = if key == STMT_RESET {
                    (SQLITE_STATEMENT_RESET, STATEMENT_COMPLETE)
                } else {
                    (SQLITE_STATEMENT_FINALIZE, STATEMENT_FINALIZE_COMPLETE)
                };
                request(
                    context,
                    operation,
                    vec![handle],
                    continuation,
                    vec![receiver, callback],
                )
            }
            _ => Err(ModuleError::ContractViolation(
                "unknown sqlite3 function key".into(),
            )),
        }
    }

    fn resume(
        &self,
        continuation: ModuleContinuation,
        state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let receiver = *state.first().ok_or_else(|| {
            ModuleError::ContractViolation("sqlite3 continuation receiver is missing".into())
        })?;
        let callback = state.get(1).copied().unwrap_or_else(|| context.undefined());
        let completion = match completion {
            Ok(completion) => completion,
            Err(message) => return complete_error(context, receiver, callback, message),
        };
        let decoded = decoded_completion(context, completion)?;
        match continuation.0 {
            OPEN_COMPLETE => {
                let handle = context.get_property(decoded, "connection")?;
                context.set_private(receiver, HANDLE, handle)?;
                let none = null(context)?;
                invoke_callback(context, callback, receiver, &[none])?;
            }
            RUN_COMPLETE | STATEMENT_COMPLETE => {
                let last_id = context.get_property(decoded, "lastID")?;
                let changes = context.get_property(decoded, "changes")?;
                context.set_property(receiver, "lastID", last_id)?;
                context.set_property(receiver, "changes", changes)?;
                let none = null(context)?;
                invoke_callback(context, callback, receiver, &[none])?;
            }
            GET_COMPLETE | STATEMENT_GET_COMPLETE => {
                let row = context.get_property(decoded, "row")?;
                let none = null(context)?;
                invoke_callback(context, callback, receiver, &[none, row])?;
            }
            ALL_COMPLETE | STATEMENT_ALL_COMPLETE => {
                let rows = context.get_property(decoded, "rows")?;
                let none = null(context)?;
                invoke_callback(context, callback, receiver, &[none, rows])?;
            }
            EACH_COMPLETE => {
                let rows = context.get_property(decoded, "rows")?;
                let row_callback = callback;
                let complete_callback = state
                    .get(2)
                    .copied()
                    .unwrap_or_else(|| context.undefined());
                let none = null(context)?;
                let count = context.array_len(rows)?;
                for index in 0..count {
                    let row = context.array_get(rows, index)?;
                    invoke_callback(context, row_callback, receiver, &[none, row])?;
                }
                if context.is_callable(complete_callback) {
                    let count = context.number(count as f64)?;
                    invoke_callback(context, complete_callback, receiver, &[none, count])?;
                }
            }
            EXEC_COMPLETE => {
                let none = null(context)?;
                invoke_callback(context, callback, receiver, &[none])?;
            }
            PREPARE_COMPLETE => {
                let handle = context.get_property(decoded, "statement")?;
                context.set_private(receiver, HANDLE, handle)?;
                let none = null(context)?;
                invoke_callback(context, callback, receiver, &[none])?;
            }
            CLOSE_COMPLETE | STATEMENT_FINALIZE_COMPLETE => {
                let undefined = context.undefined();
                context.set_private(receiver, HANDLE, undefined)?;
                let none = null(context)?;
                invoke_callback(context, callback, receiver, &[none])?;
            }
            _ => {
                return Err(ModuleError::ContractViolation(
                    "unknown sqlite3 continuation".into(),
                ));
            }
        }
        Ok(ModuleCallResult::Return(receiver))
    }

    fn event(
        &self,
        _event: u32,
        _target: ValueHandle,
        _payload: ValueHandle,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "sqlite3 has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_standard_import_and_host_contract() {
        let module = Sqlite3Module::default();
        assert_eq!(module.manifest().imports, ["sqlite3"]);
        assert_eq!(module.manifest().capabilities.len(), 1);
        assert_eq!(module.manifest().capabilities[0].id, SQLITE_REQUEST);
        assert_eq!(module.manifest().capabilities[0].completion, CompletionMode::Yield);
        assert!(module.manifest().function_keys.contains(&DATABASE.0));
        assert!(module.manifest().object_kind_keys.contains(&DATABASE_OBJECT.0));
    }
}
