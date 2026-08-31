//! `better-sqlite3` compatibility surface backed by host-owned session databases.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleObjectKind, ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};

pub const SQLITE_REQUEST: &str = "jjs:sqlite/request";
pub const SQLITE_OPEN: &str = "jjs:sqlite/open";
pub const SQLITE_EXEC: &str = "jjs:sqlite/exec";
pub const SQLITE_PREPARE: &str = "jjs:sqlite/prepare";
pub const SQLITE_CLOSE: &str = "jjs:sqlite/close";
pub const BETTER_SQLITE3_CLOSE: &str = "jjs:sqlite/better/close";
pub const SQLITE_ALL: &str = "jjs:sqlite/all";
pub const SQLITE_STATEMENT_RUN: &str = "jjs:sqlite/statement/run";
pub const SQLITE_STATEMENT_GET: &str = "jjs:sqlite/statement/get";
pub const SQLITE_STATEMENT_ALL: &str = "jjs:sqlite/statement/all";
pub const SQLITE_STATEMENT_BIND: &str = "jjs:sqlite/statement/bind";

const OPEN_READONLY: f64 = 1.0;
const OPEN_READWRITE: f64 = 2.0;
const OPEN_CREATE: f64 = 4.0;

const DATABASE: ModuleFunctionKey = ModuleFunctionKey(1);
const DB_EXEC: ModuleFunctionKey = ModuleFunctionKey(10);
const DB_PREPARE: ModuleFunctionKey = ModuleFunctionKey(11);
const DB_PRAGMA: ModuleFunctionKey = ModuleFunctionKey(12);
const DB_CLOSE: ModuleFunctionKey = ModuleFunctionKey(13);
const DB_TRANSACTION: ModuleFunctionKey = ModuleFunctionKey(14);
const DB_UNSUPPORTED: ModuleFunctionKey = ModuleFunctionKey(15);
const STMT_RUN: ModuleFunctionKey = ModuleFunctionKey(30);
const STMT_GET: ModuleFunctionKey = ModuleFunctionKey(31);
const STMT_ALL: ModuleFunctionKey = ModuleFunctionKey(32);
const STMT_BIND: ModuleFunctionKey = ModuleFunctionKey(33);
const STMT_PLUCK: ModuleFunctionKey = ModuleFunctionKey(34);
const STMT_UNSUPPORTED: ModuleFunctionKey = ModuleFunctionKey(35);
const TRANSACTION_CALL: ModuleFunctionKey = ModuleFunctionKey(50);

const DATABASE_OBJECT: ModuleObjectKind = ModuleObjectKind(1);
const STATEMENT_OBJECT: ModuleObjectKind = ModuleObjectKind(2);

const HANDLE: u32 = 1;
const OWNER: u32 = 2;
const CALLBACK: u32 = 3;
const TX_MODE: u32 = 4;
const PLUCK: u32 = 5;
const UNSUPPORTED_FEATURE: u32 = 6;

const OPEN_COMPLETE: u32 = 1;
const EXEC_COMPLETE: u32 = 2;
const PREPARE_COMPLETE: u32 = 3;
const CLOSE_COMPLETE: u32 = 4;
const PRAGMA_COMPLETE: u32 = 5;
const RUN_COMPLETE: u32 = 6;
const GET_COMPLETE: u32 = 7;
const ALL_COMPLETE: u32 = 8;
const BIND_COMPLETE: u32 = 9;

pub struct BetterSqlite3Module {
    manifest: ModuleManifest,
}

impl Default for BetterSqlite3Module {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.better-sqlite3".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-better-sqlite3-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["better-sqlite3".into()],
                capabilities: vec![HostCapabilityDescriptor {
                    id: SQLITE_REQUEST.into(),
                    contract_version: 1,
                    completion: CompletionMode::Sync,
                    schema: "jjs.sqlite.request.v1".into(),
                }],
                dependencies: vec![],
                function_keys: vec![
                    DATABASE.0,
                    DB_EXEC.0,
                    DB_PREPARE.0,
                    DB_PRAGMA.0,
                    DB_CLOSE.0,
                    DB_TRANSACTION.0,
                    DB_UNSUPPORTED.0,
                    STMT_RUN.0,
                    STMT_GET.0,
                    STMT_ALL.0,
                    STMT_BIND.0,
                    STMT_PLUCK.0,
                    STMT_UNSUPPORTED.0,
                    TRANSACTION_CALL.0,
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

fn unsupported_message(feature: &str) -> String {
    format!(
        "JJS_UNSUPPORTED_FEATURE: {feature} is not supported by this JJS runtime. Retrying will not succeed."
    )
}

fn unsupported(
    context: &mut dyn ModuleContext,
    feature: &str,
) -> Result<ModuleCallResult, ModuleError> {
    let error = context.object()?;
    let name = context.string("JjsUnsupportedFeatureError")?;
    let code = context.string("JJS_UNSUPPORTED_FEATURE")?;
    let retryable = context.bool(false)?;
    let feature_value = context.string(feature)?;
    let message = context.string(&unsupported_message(feature))?;
    context.set_property(error, "name", name)?;
    context.set_property(error, "code", code)?;
    context.set_property(error, "retryable", retryable)?;
    context.set_property(error, "feature", feature_value)?;
    context.set_property(error, "message", message)?;
    Ok(ModuleCallResult::ThrowValue(error))
}

fn attach(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
    key: ModuleFunctionKey,
) -> Result<ValueHandle, ModuleError> {
    let function = context.function(key)?;
    if matches!(key, DB_UNSUPPORTED | STMT_UNSUPPORTED) {
        let feature = context.string(name)?;
        context.set_private(function, UNSUPPORTED_FEATURE, feature)?;
    }
    context.set_property(object, name, function)?;
    Ok(function)
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

fn decoded_completion(
    context: &mut dyn ModuleContext,
    completion: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    let encoded = context.as_string(completion)?;
    let encoded = context.string(&encoded)?;
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
    let saved = state.clone();
    let result = context.request_host(
        HostRequestSpec {
            capability: SQLITE_REQUEST.into(),
            operation: operation.into(),
            arguments,
        },
        ModuleContinuation(continuation),
        state,
        false,
    )?;
    match result {
        ModuleCallResult::Return(completion) => BetterSqlite3Module::default().resume(
            ModuleContinuation(continuation),
            &saved,
            Ok(completion),
            context,
        ),
        other => Ok(other),
    }
}

fn require_handle(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    kind: &str,
) -> Result<Result<ValueHandle, ModuleCallResult>, ModuleError> {
    let handle = context.get_private(object, HANDLE)?;
    if context.value_kind(handle)? != ModuleValueKind::Number {
        return Ok(Err(thrown(format!("The {kind} is not open"))));
    }
    Ok(Ok(handle))
}

fn set_bool_property(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
    value: bool,
) -> Result<(), ModuleError> {
    let value = context.bool(value)?;
    context.set_property(object, name, value)
}

fn option_bool(
    context: &mut dyn ModuleContext,
    options: ValueHandle,
    name: &str,
) -> Result<bool, ModuleError> {
    let value = context.get_property(options, name)?;
    if context.value_kind(value)? == ModuleValueKind::Undefined {
        Ok(false)
    } else if context.value_kind(value)? == ModuleValueKind::Bool {
        context.as_bool(value)
    } else {
        Err(ModuleError::ContractViolation(format!(
            "better-sqlite3 option {name} must be a boolean"
        )))
    }
}

fn database(context: &mut dyn ModuleContext) -> Result<ValueHandle, ModuleError> {
    let value = context.module_object(DATABASE_OBJECT)?;
    for (name, key) in [
        ("exec", DB_EXEC),
        ("prepare", DB_PREPARE),
        ("pragma", DB_PRAGMA),
        ("close", DB_CLOSE),
        ("transaction", DB_TRANSACTION),
        ("backup", DB_UNSUPPORTED),
        ("serialize", DB_UNSUPPORTED),
        ("function", DB_UNSUPPORTED),
        ("aggregate", DB_UNSUPPORTED),
        ("table", DB_UNSUPPORTED),
        ("loadExtension", DB_UNSUPPORTED),
        ("defaultSafeIntegers", DB_UNSUPPORTED),
    ] {
        let _ = attach(context, value, name, key)?;
    }
    Ok(value)
}

fn statement(
    context: &mut dyn ModuleContext,
    owner: ValueHandle,
    source: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    let value = context.module_object(STATEMENT_OBJECT)?;
    context.set_private(value, OWNER, owner)?;
    let disabled = context.bool(false)?;
    context.set_private(value, PLUCK, disabled)?;
    context.set_property(value, "source", source)?;
    for (name, key) in [
        ("run", STMT_RUN),
        ("get", STMT_GET),
        ("all", STMT_ALL),
        ("bind", STMT_BIND),
        ("pluck", STMT_PLUCK),
        ("iterate", STMT_UNSUPPORTED),
        ("raw", STMT_UNSUPPORTED),
        ("expand", STMT_UNSUPPORTED),
        ("columns", STMT_UNSUPPORTED),
        ("safeIntegers", STMT_UNSUPPORTED),
    ] {
        let _ = attach(context, value, name, key)?;
    }
    Ok(value)
}

fn first_property(
    context: &mut dyn ModuleContext,
    row: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    if context.value_kind(row)? == ModuleValueKind::Undefined {
        return Ok(row);
    }
    let names = context.own_property_names(row)?;
    match names.first() {
        Some(name) => context.get_property(row, name),
        None => Ok(context.undefined()),
    }
}

fn transaction_function(
    context: &mut dyn ModuleContext,
    database: ValueHandle,
    callback: ValueHandle,
    mode: &str,
) -> Result<ValueHandle, ModuleError> {
    let function = context.function(TRANSACTION_CALL)?;
    context.set_private(function, OWNER, database)?;
    context.set_private(function, CALLBACK, callback)?;
    let mode_value = context.string(mode)?;
    context.set_private(function, TX_MODE, mode_value)?;
    Ok(function)
}

fn sync_exec(
    context: &mut dyn ModuleContext,
    database: ValueHandle,
    sql: &str,
) -> Result<Option<ModuleCallResult>, ModuleError> {
    let handle = match require_handle(context, database, "database connection")? {
        Ok(handle) => handle,
        Err(error) => return Ok(Some(error)),
    };
    let sql = context.string(sql)?;
    let result = request(
        context,
        SQLITE_EXEC,
        vec![handle, sql],
        EXEC_COMPLETE,
        vec![database],
    )?;
    match result {
        ModuleCallResult::Return(_) => Ok(None),
        other => Ok(Some(other)),
    }
}

impl NativeModule for BetterSqlite3Module {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let constructor = context.function(DATABASE)?;
        let sqlite_error = context.string("SqliteError")?;
        context.set_property(constructor, "SqliteError", sqlite_error)?;
        Ok(ModuleCallResult::Return(constructor))
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
            DATABASE => {
                if args.is_empty() || args.len() > 2 {
                    return Ok(thrown(
                        "better-sqlite3 Database requires a filename and optional options object",
                    ));
                }
                if context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(thrown("better-sqlite3 filename must be a string"));
                }
                let mut readonly = false;
                let mut file_must_exist = false;
                if let Some(options) = args.get(1).copied() {
                    if context.value_kind(options)? != ModuleValueKind::Object {
                        return Ok(thrown("better-sqlite3 options must be an object"));
                    }
                    readonly = option_bool(context, options, "readonly")?;
                    file_must_exist = option_bool(context, options, "fileMustExist")?;
                    let verbose = context.get_property(options, "verbose")?;
                    if context.value_kind(verbose)? != ModuleValueKind::Undefined {
                        return unsupported(context, "better-sqlite3.Database option verbose");
                    }
                }
                let mode = if readonly {
                    OPEN_READONLY
                } else if file_must_exist {
                    OPEN_READWRITE
                } else {
                    OPEN_READWRITE + OPEN_CREATE
                };
                let db = database(context)?;
                context.set_property(db, "name", args[0])?;
                set_bool_property(context, db, "open", true)?;
                set_bool_property(context, db, "readonly", readonly)?;
                let filename = context.as_string(args[0])?;
                set_bool_property(context, db, "memory", filename == ":memory:")?;
                set_bool_property(context, db, "inTransaction", false)?;
                let mode = context.number(mode)?;
                request(
                    context,
                    SQLITE_OPEN,
                    vec![args[0], mode],
                    OPEN_COMPLETE,
                    vec![db],
                )
            }
            DB_EXEC => {
                if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(thrown("better-sqlite3 exec requires one SQL string"));
                }
                let handle = match require_handle(context, receiver, "database connection")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                request(
                    context,
                    SQLITE_EXEC,
                    vec![handle, args[0]],
                    EXEC_COMPLETE,
                    vec![receiver],
                )
            }
            DB_PREPARE => {
                if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::String {
                    return Ok(thrown("better-sqlite3 prepare requires one SQL string"));
                }
                let handle = match require_handle(context, receiver, "database connection")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let statement = statement(context, receiver, args[0])?;
                let params = encode_params(context, &[])?;
                request(
                    context,
                    SQLITE_PREPARE,
                    vec![handle, args[0], params],
                    PREPARE_COMPLETE,
                    vec![statement],
                )
            }
            DB_PRAGMA => {
                if args.is_empty()
                    || args.len() > 2
                    || context.value_kind(args[0])? != ModuleValueKind::String
                {
                    return Ok(thrown("better-sqlite3 pragma requires a pragma string"));
                }
                let handle = match require_handle(context, receiver, "database connection")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let simple = if let Some(options) = args.get(1).copied() {
                    if context.value_kind(options)? != ModuleValueKind::Object {
                        return Ok(thrown("better-sqlite3 pragma options must be an object"));
                    }
                    option_bool(context, options, "simple")?
                } else {
                    false
                };
                let sql = format!("PRAGMA {}", context.as_string(args[0])?);
                let sql = context.string(&sql)?;
                let params = encode_params(context, &[])?;
                let simple = context.bool(simple)?;
                request(
                    context,
                    SQLITE_ALL,
                    vec![handle, sql, params],
                    PRAGMA_COMPLETE,
                    vec![receiver, simple],
                )
            }
            DB_CLOSE => {
                if !args.is_empty() {
                    return Ok(thrown("better-sqlite3 close does not accept arguments"));
                }
                let handle = match require_handle(context, receiver, "database connection")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                request(
                    context,
                    BETTER_SQLITE3_CLOSE,
                    vec![handle],
                    CLOSE_COMPLETE,
                    vec![receiver],
                )
            }
            DB_TRANSACTION => {
                if args.len() != 1 || !context.is_callable(args[0]) {
                    return Ok(thrown("better-sqlite3 transaction requires one function"));
                }
                let function = transaction_function(context, receiver, args[0], "")?;
                for (name, mode) in [
                    ("deferred", "DEFERRED"),
                    ("immediate", "IMMEDIATE"),
                    ("exclusive", "EXCLUSIVE"),
                ] {
                    let variant = transaction_function(context, receiver, args[0], mode)?;
                    context.set_property(function, name, variant)?;
                }
                Ok(ModuleCallResult::Return(function))
            }
            TRANSACTION_CALL => {
                let db = context.get_private(callee, OWNER)?;
                let callback = context.get_private(callee, CALLBACK)?;
                let mode_value = context.get_private(callee, TX_MODE)?;
                let mode = context.as_string(mode_value)?;
                let begin = if mode.is_empty() {
                    "BEGIN".to_string()
                } else {
                    format!("BEGIN {mode}")
                };
                if let Some(result) = sync_exec(context, db, &begin)? {
                    return Ok(result);
                }
                set_bool_property(context, db, "inTransaction", true)?;
                match context.try_call(callback, receiver, args)? {
                    Ok(value) => {
                        if let Some(result) = sync_exec(context, db, "COMMIT")? {
                            return Ok(result);
                        }
                        set_bool_property(context, db, "inTransaction", false)?;
                        Ok(ModuleCallResult::Return(value))
                    }
                    Err(error) => {
                        let rollback = sync_exec(context, db, "ROLLBACK")?;
                        set_bool_property(context, db, "inTransaction", false)?;
                        if let Some(result) = rollback {
                            return Ok(result);
                        }
                        Ok(ModuleCallResult::ThrowValue(error))
                    }
                }
            }
            STMT_RUN | STMT_GET | STMT_ALL | STMT_BIND => {
                let handle = match require_handle(context, receiver, "statement")? {
                    Ok(handle) => handle,
                    Err(error) => return Ok(error),
                };
                let params = encode_params(context, args)?;
                let (operation, continuation) = match key {
                    STMT_RUN => (SQLITE_STATEMENT_RUN, RUN_COMPLETE),
                    STMT_GET => (SQLITE_STATEMENT_GET, GET_COMPLETE),
                    STMT_ALL => (SQLITE_STATEMENT_ALL, ALL_COMPLETE),
                    STMT_BIND => (SQLITE_STATEMENT_BIND, BIND_COMPLETE),
                    _ => unreachable!(),
                };
                request(
                    context,
                    operation,
                    vec![handle, params],
                    continuation,
                    vec![receiver],
                )
            }
            STMT_PLUCK => {
                if args.len() > 1 {
                    return Ok(thrown("statement.pluck accepts at most one boolean"));
                }
                let enabled = match args.first().copied() {
                    None => true,
                    Some(value) if context.value_kind(value)? == ModuleValueKind::Bool => {
                        context.as_bool(value)?
                    }
                    Some(_) => return Ok(thrown("statement.pluck requires a boolean")),
                };
                let enabled = context.bool(enabled)?;
                context.set_private(receiver, PLUCK, enabled)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            DB_UNSUPPORTED | STMT_UNSUPPORTED => {
                let feature = context.get_private(callee, UNSUPPORTED_FEATURE)?;
                let feature = context.as_string(feature)?;
                unsupported(context, &format!("better-sqlite3.{feature}"))
            }
            _ => Err(ModuleError::ContractViolation(
                "unknown better-sqlite3 function key".into(),
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
            ModuleError::ContractViolation("better-sqlite3 continuation receiver is missing".into())
        })?;
        let completion = match completion {
            Ok(completion) => completion,
            Err(message) => {
                return Ok(ModuleCallResult::Throw {
                    name: "SqliteError".into(),
                    message,
                });
            }
        };
        let decoded = decoded_completion(context, completion)?;
        match continuation.0 {
            OPEN_COMPLETE => {
                let handle = context.get_property(decoded, "connection")?;
                context.set_private(receiver, HANDLE, handle)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            EXEC_COMPLETE => Ok(ModuleCallResult::Return(receiver)),
            PREPARE_COMPLETE => {
                let handle = context.get_property(decoded, "statement")?;
                context.set_private(receiver, HANDLE, handle)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            CLOSE_COMPLETE => {
                let undefined = context.undefined();
                context.set_private(receiver, HANDLE, undefined)?;
                set_bool_property(context, receiver, "open", false)?;
                Ok(ModuleCallResult::Return(receiver))
            }
            RUN_COMPLETE => {
                let result = context.object()?;
                let changes = context.get_property(decoded, "changes")?;
                let last_id = context.get_property(decoded, "lastID")?;
                context.set_property(result, "changes", changes)?;
                context.set_property(result, "lastInsertRowid", last_id)?;
                Ok(ModuleCallResult::Return(result))
            }
            GET_COMPLETE => {
                let row = context.get_property(decoded, "row")?;
                let pluck_value = context.get_private(receiver, PLUCK)?;
                let pluck = context.as_bool(pluck_value)?;
                if pluck {
                    Ok(ModuleCallResult::Return(first_property(context, row)?))
                } else {
                    Ok(ModuleCallResult::Return(row))
                }
            }
            ALL_COMPLETE => {
                let rows = context.get_property(decoded, "rows")?;
                let pluck_value = context.get_private(receiver, PLUCK)?;
                let pluck = context.as_bool(pluck_value)?;
                if !pluck {
                    return Ok(ModuleCallResult::Return(rows));
                }
                let values = context.array()?;
                for index in 0..context.array_len(rows)? {
                    let row = context.array_get(rows, index)?;
                    let value = first_property(context, row)?;
                    context.array_push(values, value)?;
                }
                Ok(ModuleCallResult::Return(values))
            }
            PRAGMA_COMPLETE => {
                let rows = context.get_property(decoded, "rows")?;
                let simple = state
                    .get(1)
                    .copied()
                    .map(|value| context.as_bool(value))
                    .transpose()?
                    .unwrap_or(false);
                if simple {
                    if context.array_len(rows)? == 0 {
                        return Ok(ModuleCallResult::Return(context.undefined()));
                    }
                    let row = context.array_get(rows, 0)?;
                    Ok(ModuleCallResult::Return(first_property(context, row)?))
                } else {
                    Ok(ModuleCallResult::Return(rows))
                }
            }
            BIND_COMPLETE => Ok(ModuleCallResult::Return(receiver)),
            _ => Err(ModuleError::ContractViolation(
                "unknown better-sqlite3 continuation".into(),
            )),
        }
    }

    fn event(
        &self,
        _event: u32,
        _target: ValueHandle,
        _payload: ValueHandle,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "better-sqlite3 has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_distinct_import_and_sync_sqlite_contract() {
        let module = BetterSqlite3Module::default();
        assert_eq!(module.manifest().imports, ["better-sqlite3"]);
        assert_eq!(module.manifest().identity.id, "org.jjs.better-sqlite3");
        assert_eq!(module.manifest().capabilities[0].id, SQLITE_REQUEST);
        assert_eq!(
            module.manifest().capabilities[0].completion,
            CompletionMode::Sync
        );
    }

    #[test]
    fn unsupported_features_are_explicit() {
        assert_eq!(
            unsupported_message("better-sqlite3.loadExtension"),
            "JJS_UNSUPPORTED_FEATURE: better-sqlite3.loadExtension is not supported by this JJS runtime. Retrying will not succeed."
        );
    }
}
