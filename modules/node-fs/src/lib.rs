//! Node `fs` compatibility surface backed exclusively by the host session VFS.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, MODULE_API_VERSION,
    ModuleCallResult, ModuleContext, ModuleContinuation, ModuleError, ModuleFunctionKey,
    ModuleIdentity, ModuleManifest, ModuleObjectKind, ModuleValueKind, NativeModule, ValueHandle,
};

pub const FS_READ: &str = "jjs:fs/readFile";
pub const FS_WRITE: &str = "jjs:fs/writeFile";
pub const FS_LIST: &str = "jjs:fs/list";
pub const FS_STAT: &str = "jjs:fs/stat";
pub const FS_MKDIR: &str = "jjs:fs/mkdir";

const READ_SYNC: ModuleFunctionKey = ModuleFunctionKey(1);
const WRITE_SYNC: ModuleFunctionKey = ModuleFunctionKey(2);
const READDIR_SYNC: ModuleFunctionKey = ModuleFunctionKey(3);
const STAT_SYNC: ModuleFunctionKey = ModuleFunctionKey(4);
const MKDIR_SYNC: ModuleFunctionKey = ModuleFunctionKey(5);
const EXISTS_SYNC: ModuleFunctionKey = ModuleFunctionKey(6);
const ACCESS_SYNC: ModuleFunctionKey = ModuleFunctionKey(7);

const READ_CALLBACK: ModuleFunctionKey = ModuleFunctionKey(11);
const WRITE_CALLBACK: ModuleFunctionKey = ModuleFunctionKey(12);
const READDIR_CALLBACK: ModuleFunctionKey = ModuleFunctionKey(13);
const STAT_CALLBACK: ModuleFunctionKey = ModuleFunctionKey(14);
const MKDIR_CALLBACK: ModuleFunctionKey = ModuleFunctionKey(15);
const EXISTS_CALLBACK: ModuleFunctionKey = ModuleFunctionKey(16);
const ACCESS_CALLBACK: ModuleFunctionKey = ModuleFunctionKey(17);

const READ_PROMISE: ModuleFunctionKey = ModuleFunctionKey(21);
const WRITE_PROMISE: ModuleFunctionKey = ModuleFunctionKey(22);
const READDIR_PROMISE: ModuleFunctionKey = ModuleFunctionKey(23);
const STAT_PROMISE: ModuleFunctionKey = ModuleFunctionKey(24);
const MKDIR_PROMISE: ModuleFunctionKey = ModuleFunctionKey(25);
const ACCESS_PROMISE: ModuleFunctionKey = ModuleFunctionKey(27);

const UNSUPPORTED: ModuleFunctionKey = ModuleFunctionKey(30);
const STAT_IS_FILE: ModuleFunctionKey = ModuleFunctionKey(40);
const STAT_IS_DIRECTORY: ModuleFunctionKey = ModuleFunctionKey(41);
const STATS_OBJECT: ModuleObjectKind = ModuleObjectKind(1);
const STAT_TYPE: u32 = 1;
const UNSUPPORTED_FEATURE: u32 = 2;

const COMPLETE_READ_SYNC: u32 = 1;
const COMPLETE_WRITE_SYNC: u32 = 2;
const COMPLETE_READDIR_SYNC: u32 = 3;
const COMPLETE_STAT_SYNC: u32 = 4;
const COMPLETE_MKDIR_SYNC: u32 = 5;
const COMPLETE_EXISTS_SYNC: u32 = 6;
const COMPLETE_ACCESS_SYNC: u32 = 7;
const COMPLETE_READ_CALLBACK: u32 = 11;
const COMPLETE_WRITE_CALLBACK: u32 = 12;
const COMPLETE_READDIR_CALLBACK: u32 = 13;
const COMPLETE_STAT_CALLBACK: u32 = 14;
const COMPLETE_MKDIR_CALLBACK: u32 = 15;
const COMPLETE_EXISTS_CALLBACK: u32 = 16;
const COMPLETE_ACCESS_CALLBACK: u32 = 17;
const COMPLETE_READ_PROMISE: u32 = 21;
const COMPLETE_WRITE_PROMISE: u32 = 22;
const COMPLETE_READDIR_PROMISE: u32 = 23;
const COMPLETE_STAT_PROMISE: u32 = 24;
const COMPLETE_MKDIR_PROMISE: u32 = 25;
const COMPLETE_ACCESS_PROMISE: u32 = 27;

fn capabilities() -> Vec<HostCapabilityDescriptor> {
    [FS_READ, FS_WRITE, FS_LIST, FS_STAT, FS_MKDIR]
        .into_iter()
        .map(|id| HostCapabilityDescriptor {
            id: id.into(),
            contract_version: 1,
            completion: CompletionMode::Yield,
            schema: "jjs.fs.request.v1".into(),
        })
        .collect()
}

fn fs_function_keys() -> Vec<u32> {
    vec![
        READ_SYNC.0,
        WRITE_SYNC.0,
        READDIR_SYNC.0,
        STAT_SYNC.0,
        MKDIR_SYNC.0,
        EXISTS_SYNC.0,
        ACCESS_SYNC.0,
        READ_CALLBACK.0,
        WRITE_CALLBACK.0,
        READDIR_CALLBACK.0,
        STAT_CALLBACK.0,
        MKDIR_CALLBACK.0,
        EXISTS_CALLBACK.0,
        ACCESS_CALLBACK.0,
        READ_PROMISE.0,
        WRITE_PROMISE.0,
        READDIR_PROMISE.0,
        STAT_PROMISE.0,
        MKDIR_PROMISE.0,
        ACCESS_PROMISE.0,
        UNSUPPORTED.0,
        STAT_IS_FILE.0,
        STAT_IS_DIRECTORY.0,
    ]
}

fn promises_function_keys() -> Vec<u32> {
    vec![
        READ_PROMISE.0,
        WRITE_PROMISE.0,
        READDIR_PROMISE.0,
        STAT_PROMISE.0,
        MKDIR_PROMISE.0,
        ACCESS_PROMISE.0,
        UNSUPPORTED.0,
        STAT_IS_FILE.0,
        STAT_IS_DIRECTORY.0,
    ]
}

pub struct NodeFsModule {
    manifest: ModuleManifest,
}

impl Default for NodeFsModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.node-fs".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-node-fs-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["fs".into(), "node:fs".into()],
                capabilities: capabilities(),
                dependencies: vec![],
                function_keys: fs_function_keys(),
                object_kind_keys: vec![STATS_OBJECT.0],
                deterministic_resources: vec![],
            },
        }
    }
}

pub struct NodeFsPromisesModule {
    manifest: ModuleManifest,
}

impl Default for NodeFsPromisesModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.node-fs-promises".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-node-fs-promises-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["fs/promises".into(), "node:fs/promises".into()],
                capabilities: capabilities(),
                dependencies: vec![],
                function_keys: promises_function_keys(),
                object_kind_keys: vec![STATS_OBJECT.0],
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

fn null(context: &mut dyn ModuleContext) -> Result<ValueHandle, ModuleError> {
    let value = context.string("null")?;
    context.json_parse(value)
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

fn attach_unsupported(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
) -> Result<(), ModuleError> {
    let function = context.function(UNSUPPORTED)?;
    let feature = context.string(name)?;
    context.set_private(function, UNSUPPORTED_FEATURE, feature)?;
    context.set_property(object, name, function)
}

fn promise_exports(context: &mut dyn ModuleContext) -> Result<ValueHandle, ModuleError> {
    let exports = context.object()?;
    for (name, key) in [
        ("readFile", READ_PROMISE),
        ("writeFile", WRITE_PROMISE),
        ("readdir", READDIR_PROMISE),
        ("stat", STAT_PROMISE),
        ("mkdir", MKDIR_PROMISE),
        ("access", ACCESS_PROMISE),
    ] {
        attach(context, exports, name, key)?;
    }
    for name in [
        "appendFile",
        "copyFile",
        "cp",
        "open",
        "opendir",
        "rename",
        "rm",
        "rmdir",
        "truncate",
        "unlink",
        "watch",
    ] {
        attach_unsupported(context, exports, name)?;
    }
    Ok(exports)
}

fn fs_exports(context: &mut dyn ModuleContext) -> Result<ValueHandle, ModuleError> {
    let exports = context.object()?;
    for (name, key) in [
        ("readFileSync", READ_SYNC),
        ("writeFileSync", WRITE_SYNC),
        ("readdirSync", READDIR_SYNC),
        ("statSync", STAT_SYNC),
        ("mkdirSync", MKDIR_SYNC),
        ("existsSync", EXISTS_SYNC),
        ("accessSync", ACCESS_SYNC),
        ("readFile", READ_CALLBACK),
        ("writeFile", WRITE_CALLBACK),
        ("readdir", READDIR_CALLBACK),
        ("stat", STAT_CALLBACK),
        ("mkdir", MKDIR_CALLBACK),
        ("exists", EXISTS_CALLBACK),
        ("access", ACCESS_CALLBACK),
    ] {
        attach(context, exports, name, key)?;
    }
    for name in [
        "appendFile",
        "appendFileSync",
        "copyFile",
        "copyFileSync",
        "cp",
        "cpSync",
        "createReadStream",
        "createWriteStream",
        "open",
        "openSync",
        "opendir",
        "opendirSync",
        "rename",
        "renameSync",
        "rm",
        "rmSync",
        "rmdir",
        "rmdirSync",
        "truncate",
        "truncateSync",
        "unlink",
        "unlinkSync",
        "watch",
        "watchFile",
    ] {
        attach_unsupported(context, exports, name)?;
    }
    let promises = promise_exports(context)?;
    context.set_property(exports, "promises", promises)?;
    let constants = context.object()?;
    let f_ok = context.number(0.0)?;
    context.set_property(constants, "F_OK", f_ok)?;
    context.set_property(exports, "constants", constants)?;
    Ok(exports)
}

fn unsupported(
    context: &mut dyn ModuleContext,
    callee: ValueHandle,
) -> Result<ModuleCallResult, ModuleError> {
    let feature = context.get_private(callee, UNSUPPORTED_FEATURE)?;
    let feature = context.as_string(feature)?;
    unsupported_feature(context, &format!("fs.{feature}"))
}

fn unsupported_feature(
    context: &mut dyn ModuleContext,
    qualified: &str,
) -> Result<ModuleCallResult, ModuleError> {
    let error = context.object()?;
    let name = context.string("JjsUnsupportedFeatureError")?;
    let code = context.string("JJS_UNSUPPORTED_FEATURE")?;
    let retryable = context.bool(false)?;
    let feature_value = context.string(&qualified)?;
    let message = context.string(&format!(
        "JJS_UNSUPPORTED_FEATURE: {qualified} is not supported by this JJS runtime. Retrying will not succeed."
    ))?;
    context.set_property(error, "name", name)?;
    context.set_property(error, "code", code)?;
    context.set_property(error, "retryable", retryable)?;
    context.set_property(error, "feature", feature_value)?;
    context.set_property(error, "message", message)?;
    Ok(ModuleCallResult::ThrowValue(error))
}

fn path_argument(
    context: &mut dyn ModuleContext,
    args: &[ValueHandle],
) -> Result<Result<ValueHandle, ModuleCallResult>, ModuleError> {
    let Some(path) = args.first().copied() else {
        return Ok(Err(thrown("fs operation requires a path string")));
    };
    if context.value_kind(path)? != ModuleValueKind::String {
        return Ok(Err(thrown("fs path must be a string")));
    }
    Ok(Ok(path))
}

fn utf8_encoding(
    context: &mut dyn ModuleContext,
    value: Option<ValueHandle>,
) -> Result<bool, ModuleError> {
    let Some(mut value) = value else {
        return Ok(false);
    };
    if context.value_kind(value)? == ModuleValueKind::Object {
        value = context.get_property(value, "encoding")?;
    }
    if context.value_kind(value)? != ModuleValueKind::String {
        return Ok(false);
    }
    Ok(matches!(
        context.as_string(value)?.to_ascii_lowercase().as_str(),
        "utf8" | "utf-8"
    ))
}

fn binary_unsupported(context: &mut dyn ModuleContext) -> Result<ModuleCallResult, ModuleError> {
    unsupported_feature(context, "fs binary Buffer results")
}

fn request(
    context: &mut dyn ModuleContext,
    operation: &str,
    arguments: Vec<ValueHandle>,
    continuation: u32,
    state: Vec<ValueHandle>,
    promise: bool,
) -> Result<ModuleCallResult, ModuleError> {
    context.request_host(
        HostRequestSpec {
            capability: operation.into(),
            operation: operation.into(),
            arguments,
        },
        ModuleContinuation(continuation),
        state,
        promise,
    )
}

fn callback_argument(context: &dyn ModuleContext, args: &[ValueHandle]) -> Option<ValueHandle> {
    args.last()
        .copied()
        .filter(|value| context.is_callable(*value))
}

fn call_api(
    key: ModuleFunctionKey,
    callee: ValueHandle,
    receiver: ValueHandle,
    args: &[ValueHandle],
    context: &mut dyn ModuleContext,
) -> Result<ModuleCallResult, ModuleError> {
    if key == UNSUPPORTED {
        return unsupported(context, callee);
    }
    if matches!(key, STAT_IS_FILE | STAT_IS_DIRECTORY) {
        let kind = context.get_private(receiver, STAT_TYPE)?;
        let kind = context.as_string(kind)?;
        return Ok(ModuleCallResult::Return(context.bool(
            (key == STAT_IS_FILE && kind == "file") || (key == STAT_IS_DIRECTORY && kind == "dir"),
        )?));
    }

    let path = match path_argument(context, args)? {
        Ok(path) => path,
        Err(error) => return Ok(error),
    };
    let path_state = vec![path];

    match key {
        READ_SYNC | READ_PROMISE => {
            if !utf8_encoding(context, args.get(1).copied())? {
                return binary_unsupported(context);
            }
            let (continuation, promise) = if key == READ_PROMISE {
                (COMPLETE_READ_PROMISE, true)
            } else {
                (COMPLETE_READ_SYNC, false)
            };
            request(
                context,
                FS_READ,
                vec![path],
                continuation,
                path_state,
                promise,
            )
        }
        READ_CALLBACK => {
            let Some(callback) = callback_argument(context, args) else {
                return Ok(thrown("fs.readFile requires a callback"));
            };
            if !utf8_encoding(context, args.get(1).copied().filter(|v| *v != callback))? {
                return binary_unsupported(context);
            }
            request(
                context,
                FS_READ,
                vec![path],
                COMPLETE_READ_CALLBACK,
                vec![callback, path],
                false,
            )
        }
        WRITE_SYNC | WRITE_PROMISE | WRITE_CALLBACK => {
            let Some(data) = args.get(1).copied() else {
                return Ok(thrown("fs write operation requires string data"));
            };
            if context.value_kind(data)? != ModuleValueKind::String {
                return binary_unsupported(context);
            }
            if key == WRITE_CALLBACK {
                let Some(callback) = callback_argument(context, args) else {
                    return Ok(thrown("fs.writeFile requires a callback"));
                };
                if let Some(options) = args.get(2).copied().filter(|value| *value != callback) {
                    if !utf8_encoding(context, Some(options))? {
                        return unsupported_feature(context, "fs.writeFile options");
                    }
                }
                request(
                    context,
                    FS_WRITE,
                    vec![path, data],
                    COMPLETE_WRITE_CALLBACK,
                    vec![callback, path],
                    false,
                )
            } else {
                if let Some(options) = args.get(2).copied() {
                    if !utf8_encoding(context, Some(options))? {
                        return unsupported_feature(context, "fs.writeFile options");
                    }
                }
                let (continuation, promise) = if key == WRITE_PROMISE {
                    (COMPLETE_WRITE_PROMISE, true)
                } else {
                    (COMPLETE_WRITE_SYNC, false)
                };
                request(
                    context,
                    FS_WRITE,
                    vec![path, data],
                    continuation,
                    path_state,
                    promise,
                )
            }
        }
        READDIR_SYNC | READDIR_PROMISE | READDIR_CALLBACK => {
            if args
                .get(1)
                .is_some_and(|value| !context.is_callable(*value))
            {
                return unsupported_feature(context, "fs.readdir options");
            }
            if key == READDIR_CALLBACK {
                let Some(callback) = callback_argument(context, args) else {
                    return Ok(thrown("fs.readdir requires a callback"));
                };
                request(
                    context,
                    FS_LIST,
                    vec![path],
                    COMPLETE_READDIR_CALLBACK,
                    vec![callback, path],
                    false,
                )
            } else {
                let (continuation, promise) = if key == READDIR_PROMISE {
                    (COMPLETE_READDIR_PROMISE, true)
                } else {
                    (COMPLETE_READDIR_SYNC, false)
                };
                request(
                    context,
                    FS_LIST,
                    vec![path],
                    continuation,
                    path_state,
                    promise,
                )
            }
        }
        STAT_SYNC | STAT_PROMISE | STAT_CALLBACK => {
            if args
                .get(1)
                .is_some_and(|value| !context.is_callable(*value))
            {
                return unsupported_feature(context, "fs.stat options");
            }
            if key == STAT_CALLBACK {
                let Some(callback) = callback_argument(context, args) else {
                    return Ok(thrown("fs.stat requires a callback"));
                };
                request(
                    context,
                    FS_STAT,
                    vec![path],
                    COMPLETE_STAT_CALLBACK,
                    vec![callback, path],
                    false,
                )
            } else {
                let (continuation, promise) = if key == STAT_PROMISE {
                    (COMPLETE_STAT_PROMISE, true)
                } else {
                    (COMPLETE_STAT_SYNC, false)
                };
                request(
                    context,
                    FS_STAT,
                    vec![path],
                    continuation,
                    path_state,
                    promise,
                )
            }
        }
        MKDIR_SYNC | MKDIR_PROMISE | MKDIR_CALLBACK => {
            if args
                .get(1)
                .is_some_and(|value| !context.is_callable(*value))
            {
                return unsupported_feature(context, "fs.mkdir options");
            }
            if key == MKDIR_CALLBACK {
                let Some(callback) = callback_argument(context, args) else {
                    return Ok(thrown("fs.mkdir requires a callback"));
                };
                request(
                    context,
                    FS_MKDIR,
                    vec![path],
                    COMPLETE_MKDIR_CALLBACK,
                    vec![callback, path],
                    false,
                )
            } else {
                let (continuation, promise) = if key == MKDIR_PROMISE {
                    (COMPLETE_MKDIR_PROMISE, true)
                } else {
                    (COMPLETE_MKDIR_SYNC, false)
                };
                request(
                    context,
                    FS_MKDIR,
                    vec![path],
                    continuation,
                    path_state,
                    promise,
                )
            }
        }
        EXISTS_SYNC | EXISTS_CALLBACK | ACCESS_SYNC | ACCESS_PROMISE | ACCESS_CALLBACK => {
            if matches!(key, ACCESS_SYNC | ACCESS_PROMISE | ACCESS_CALLBACK) {
                if let Some(mode) = args
                    .get(1)
                    .copied()
                    .filter(|value| !context.is_callable(*value))
                {
                    if context.value_kind(mode)? != ModuleValueKind::Number
                        || context.as_number(mode)? != 0.0
                    {
                        return unsupported_feature(context, "fs.access modes other than F_OK");
                    }
                }
            }
            let (continuation, promise, state) = match key {
                EXISTS_SYNC => (COMPLETE_EXISTS_SYNC, false, path_state),
                EXISTS_CALLBACK => {
                    let Some(callback) = callback_argument(context, args) else {
                        return Ok(thrown("fs.exists requires a callback"));
                    };
                    (COMPLETE_EXISTS_CALLBACK, false, vec![callback, path])
                }
                ACCESS_SYNC => (COMPLETE_ACCESS_SYNC, false, path_state),
                ACCESS_PROMISE => (COMPLETE_ACCESS_PROMISE, true, path_state),
                ACCESS_CALLBACK => {
                    let Some(callback) = callback_argument(context, args) else {
                        return Ok(thrown("fs.access requires a callback"));
                    };
                    (COMPLETE_ACCESS_CALLBACK, false, vec![callback, path])
                }
                _ => unreachable!(),
            };
            request(context, FS_STAT, vec![path], continuation, state, promise)
        }
        _ => Err(ModuleError::ContractViolation(
            "unknown node fs function key".into(),
        )),
    }
}

fn stats(context: &mut dyn ModuleContext, raw: ValueHandle) -> Result<ValueHandle, ModuleError> {
    let value = context.module_object(STATS_OBJECT)?;
    let kind = context.get_property(raw, "type")?;
    let size = context.get_property(raw, "size")?;
    context.set_private(value, STAT_TYPE, kind)?;
    context.set_property(value, "size", size)?;
    attach(context, value, "isFile", STAT_IS_FILE)?;
    attach(context, value, "isDirectory", STAT_IS_DIRECTORY)?;
    Ok(value)
}

fn fs_error(
    context: &mut dyn ModuleContext,
    code: &str,
    operation: &str,
    path: ValueHandle,
) -> Result<ValueHandle, ModuleError> {
    let path_text = context.as_string(path)?;
    let error = context.object()?;
    let name = context.string("Error")?;
    let code_value = context.string(code)?;
    let syscall = context.string(operation)?;
    let message = context.string(&format!("{code}: {operation} '{path_text}'"))?;
    context.set_property(error, "name", name)?;
    context.set_property(error, "code", code_value)?;
    context.set_property(error, "syscall", syscall)?;
    context.set_property(error, "path", path)?;
    context.set_property(error, "message", message)?;
    Ok(error)
}

fn operation_for(continuation: u32) -> &'static str {
    match continuation {
        COMPLETE_READ_SYNC | COMPLETE_READ_CALLBACK | COMPLETE_READ_PROMISE => "readFile",
        COMPLETE_WRITE_SYNC | COMPLETE_WRITE_CALLBACK | COMPLETE_WRITE_PROMISE => "writeFile",
        COMPLETE_READDIR_SYNC | COMPLETE_READDIR_CALLBACK | COMPLETE_READDIR_PROMISE => "readdir",
        COMPLETE_STAT_SYNC | COMPLETE_STAT_CALLBACK | COMPLETE_STAT_PROMISE => "stat",
        COMPLETE_MKDIR_SYNC | COMPLETE_MKDIR_CALLBACK | COMPLETE_MKDIR_PROMISE => "mkdir",
        COMPLETE_EXISTS_SYNC | COMPLETE_EXISTS_CALLBACK => "exists",
        COMPLETE_ACCESS_SYNC | COMPLETE_ACCESS_CALLBACK | COMPLETE_ACCESS_PROMISE => "access",
        _ => "fs",
    }
}

fn resume_api(
    continuation: ModuleContinuation,
    state: &[ValueHandle],
    completion: Result<ValueHandle, String>,
    context: &mut dyn ModuleContext,
) -> Result<ModuleCallResult, ModuleError> {
    let id = continuation.0;
    let callback_style = (COMPLETE_READ_CALLBACK..=COMPLETE_ACCESS_CALLBACK).contains(&id);
    let exists_style = matches!(id, COMPLETE_EXISTS_SYNC | COMPLETE_EXISTS_CALLBACK);
    let path_index = usize::from(callback_style);
    let path = *state.get(path_index).ok_or_else(|| {
        ModuleError::ContractViolation("node fs continuation path is missing".into())
    })?;

    let value = match completion {
        Ok(value) => {
            if exists_style {
                context.bool(true)?
            } else if matches!(
                id,
                COMPLETE_ACCESS_SYNC | COMPLETE_ACCESS_CALLBACK | COMPLETE_ACCESS_PROMISE
            ) {
                context.undefined()
            } else if matches!(
                id,
                COMPLETE_STAT_SYNC | COMPLETE_STAT_CALLBACK | COMPLETE_STAT_PROMISE
            ) {
                stats(context, value)?
            } else {
                value
            }
        }
        Err(_code) if exists_style => context.bool(false)?,
        Err(code) => {
            let error = fs_error(context, &code, operation_for(id), path)?;
            if callback_style {
                let callback = state[0];
                let undefined = context.undefined();
                context.call(callback, undefined, &[error])?;
                return Ok(ModuleCallResult::Return(undefined));
            }
            return Ok(ModuleCallResult::ThrowValue(error));
        }
    };

    if callback_style {
        let callback = state[0];
        let undefined = context.undefined();
        if id == COMPLETE_EXISTS_CALLBACK {
            context.call(callback, undefined, &[value])?;
        } else {
            let none = null(context)?;
            if matches!(
                id,
                COMPLETE_WRITE_CALLBACK | COMPLETE_MKDIR_CALLBACK | COMPLETE_ACCESS_CALLBACK
            ) {
                context.call(callback, undefined, &[none])?;
            } else {
                context.call(callback, undefined, &[none, value])?;
            }
        }
        Ok(ModuleCallResult::Return(undefined))
    } else {
        Ok(ModuleCallResult::Return(value))
    }
}

impl NativeModule for NodeFsModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Ok(ModuleCallResult::Return(fs_exports(context)?))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        call_api(key, callee, receiver, args, context)
    }

    fn resume(
        &self,
        continuation: ModuleContinuation,
        state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        resume_api(continuation, state, completion, context)
    }

    fn event(
        &self,
        _event: u32,
        _target: ValueHandle,
        _payload: ValueHandle,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "node fs has no guest events".into(),
        ))
    }
}

impl NativeModule for NodeFsPromisesModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Ok(ModuleCallResult::Return(promise_exports(context)?))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        call_api(key, callee, receiver, args, context)
    }

    fn resume(
        &self,
        continuation: ModuleContinuation,
        state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        resume_api(continuation, state, completion, context)
    }

    fn event(
        &self,
        _event: u32,
        _target: ValueHandle,
        _payload: ValueHandle,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "node fs promises has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_all_standard_fs_imports() {
        assert_eq!(NodeFsModule::default().manifest.imports, ["fs", "node:fs"]);
        assert_eq!(
            NodeFsPromisesModule::default().manifest.imports,
            ["fs/promises", "node:fs/promises"]
        );
    }
}
