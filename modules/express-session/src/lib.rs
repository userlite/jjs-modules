//! Database-backed Express sessions. All time, randomness and storage belong to the host.
use jjs_module_api::*;
use serde_json::{json, Value};

pub const SESSION_REQUEST: &str = "jjs:session/request";
const OPTIONS: u32 = 1;
const REQUEST: u32 = 2;
const RESPONSE: u32 = 3;
const ORIGINAL_END: u32 = 4;
const ORIGINAL: u32 = 5;
const TOKEN: u32 = 6;
const COMMITTED: u32 = 7;
pub struct ExpressSessionModule {
    manifest: ModuleManifest,
}
impl Default for ExpressSessionModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.express-session".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-express-session-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["express-session".into()],
                capabilities: vec![HostCapabilityDescriptor {
                    id: SESSION_REQUEST.into(),
                    contract_version: 1,
                    completion: CompletionMode::Sync,
                    schema: "jjs.session.v1".into(),
                }],
                dependencies: vec![],
                function_keys: vec![1, 2, 3, 4],
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}
fn error(s: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: "Error".into(),
        message: s.into(),
    }
}
fn request(c: &mut dyn ModuleContext, value: Value) -> Result<Result<Value, String>, ModuleError> {
    let encoded = c.string(&value.to_string())?;
    let result = c.request_host(
        HostRequestSpec {
            capability: SESSION_REQUEST.into(),
            operation: SESSION_REQUEST.into(),
            arguments: vec![encoded],
        },
        ModuleContinuation(0),
        vec![],
        false,
    )?;
    match result {
        ModuleCallResult::Return(v) => {
            let result: Value = serde_json::from_str(&c.as_string(v)?)
                .map_err(|e| ModuleError::ContractViolation(e.to_string()))?;
            if let Some(e) = result.get("error").and_then(Value::as_str) {
                Ok(Err(e.into()))
            } else {
                Ok(Ok(result))
            }
        }
        _ => Err(ModuleError::ContractViolation(
            "session host must complete synchronously".into(),
        )),
    }
}
fn json_value(c: &mut dyn ModuleContext, v: ValueHandle) -> Result<Value, ModuleError> {
    let encoded = c.json_stringify(v)?;
    serde_json::from_str(&c.as_string(encoded)?)
        .map_err(|e| ModuleError::ContractViolation(e.to_string()))
}
fn validate(v: &Value) -> Result<(), String> {
    let o = v.as_object().ok_or("session options must be an object")?;
    for k in o.keys() {
        if !["secret", "resave", "saveUninitialized", "cookie"].contains(&k.as_str()) {
            return Err(format!("unsupported session option: {k}"));
        }
    }
    if !v["secret"].as_str().is_some_and(|s| !s.trim().is_empty()) {
        return Err("session secret must be a non-empty string".into());
    }
    if v["resave"] != false || v["saveUninitialized"] != false {
        return Err("baseline sessions require resave:false and saveUninitialized:false".into());
    }
    let cookie = v["cookie"]
        .as_object()
        .ok_or("session cookie options are required")?;
    for k in cookie.keys() {
        if !["httpOnly", "secure", "sameSite", "maxAge"].contains(&k.as_str()) {
            return Err(format!("unsupported session cookie option: {k}"));
        }
    }
    if v["cookie"]["httpOnly"] != true
        || !v["cookie"]["secure"].is_boolean()
        || v["cookie"]["sameSite"] != "lax"
    {
        return Err(
            "baseline cookies require httpOnly:true, secure:boolean and sameSite:'lax'".into(),
        );
    }
    if !v["cookie"]["maxAge"]
        .as_u64()
        .is_some_and(|n| n > 0 && n <= 2_592_000_000)
    {
        return Err("cookie.maxAge must be an integer from 1 to 2592000000 milliseconds".into());
    }
    Ok(())
}
impl NativeModule for ExpressSessionModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn instantiate(&self, c: &mut dyn ModuleContext) -> Result<ModuleCallResult, ModuleError> {
        let f = c.function(ModuleFunctionKey(1))?;
        c.set_property(f, "default", f)?;
        Ok(ModuleCallResult::Return(f))
    }
    fn call(
        &self,
        key: ModuleFunctionKey,
        callee: ValueHandle,
        _: ValueHandle,
        args: &[ValueHandle],
        c: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        match key.0 {
            1 => {
                if args.len() != 1 {
                    return Ok(error("session requires one options object"));
                }
                for name in c.own_property_names(args[0])? {
                    if !["secret", "resave", "saveUninitialized", "cookie"].contains(&name.as_str())
                    {
                        return Ok(error(format!("unsupported session option: {name}")));
                    }
                }
                let cookie = c.get_property(args[0], "cookie")?;
                for name in c.own_property_names(cookie)? {
                    if !["httpOnly", "secure", "sameSite", "maxAge"].contains(&name.as_str()) {
                        return Ok(error(format!("unsupported session cookie option: {name}")));
                    }
                }
                let options = json_value(c, args[0])?;
                if let Err(e) = validate(&options) {
                    return Ok(error(e));
                }
                // Validate host storage even if no request ever changes a session.
                if let Err(e) = request(c, json!({"op":"check"}))? {
                    return Ok(error(e));
                }
                let f = c.function(ModuleFunctionKey(2))?;
                let options = c.string(&options.to_string())?;
                c.set_private(f, OPTIONS, options)?;
                Ok(ModuleCallResult::Return(f))
            }
            2 => {
                if args.len() != 3 {
                    return Ok(error("session middleware requires req, res, next"));
                }
                let options = c.get_private(callee, OPTIONS)?;
                let config: Value = serde_json::from_str(&c.as_string(options)?)
                    .map_err(|e| ModuleError::ContractViolation(e.to_string()))?;
                let headers = c.get_property(args[0], "headers")?;
                let cookie = c.get_property(headers, "cookie")?;
                let cookie = if c.value_kind(cookie)? == ModuleValueKind::Undefined {
                    String::new()
                } else {
                    c.as_string(cookie)?
                };
                let result = match request(
                    c,
                    json!({"op":"load","secret":config["secret"],"cookie":cookie,"maxAge":config["cookie"]["maxAge"]}),
                )? {
                    Ok(v) => v,
                    Err(e) => return Ok(error(e)),
                };
                let id = c.string(result["id"].as_str().ok_or_else(|| {
                    ModuleError::ContractViolation("session host omitted id".into())
                })?)?;
                c.set_property(args[0], "sessionID", id)?;
                let mut data = result["data"].clone();
                data["cookie"] = config["cookie"].clone();
                let encoded = c.string(&data.to_string())?;
                let session = c.json_parse(encoded)?;
                c.set_property(args[0], "session", session)?;
                let hook = c.function(ModuleFunctionKey(3))?;
                let raw = c.get_property(args[1], "_expressRawEnd")?;
                if !c.is_callable(raw) {
                    return Ok(error("session middleware requires an Express response"));
                }
                c.set_private(hook, ORIGINAL_END, raw)?;
                c.set_private(hook, OPTIONS, options)?;
                c.set_private(hook, REQUEST, args[0])?;
                c.set_private(hook, RESPONSE, args[1])?;
                c.set_private(hook, ORIGINAL, encoded)?;
                let token = c.string(result["token"].as_str().ok_or_else(|| {
                    ModuleError::ContractViolation("session host omitted token".into())
                })?)?;
                c.set_private(hook, TOKEN, token)?;
                let no = c.bool(false)?;
                c.set_private(hook, COMMITTED, no)?;
                c.set_property(args[1], "_expressRawEnd", hook)?;
                // Streaming commits headers before session changes are known; reject explicitly.
                let unsupported = c.function(ModuleFunctionKey(4))?;
                for name in ["_expressRawWrite", "_expressRawFlushHeaders", "writeHead"] {
                    c.set_property(args[1], name, unsupported)?;
                }
                let undefined = c.undefined();
                c.call(args[2], undefined, &[])?;
                Ok(ModuleCallResult::Return(undefined))
            }
            3 => {
                let committed = c.get_private(callee, COMMITTED)?;
                if c.is_truthy(committed)? {
                    return Ok(error("session response already committed"));
                }
                let req = c.get_private(callee, REQUEST)?;
                let res = c.get_private(callee, RESPONSE)?;
                let options = c.get_private(callee, OPTIONS)?;
                let config: Value = serde_json::from_str(&c.as_string(options)?)
                    .map_err(|e| ModuleError::ContractViolation(e.to_string()))?;
                let session = c.get_property(req, "session")?;
                let data = json_value(c, session)?;
                let original = c.get_private(callee, ORIGINAL)?;
                let original: Value = serde_json::from_str(&c.as_string(original)?)
                    .map_err(|e| ModuleError::ContractViolation(e.to_string()))?;
                if data != original {
                    if !data.is_null()
                        && (!data.is_object() || data["cookie"] != original["cookie"])
                    {
                        return Ok(error("session must be an object or null; changing cookie options is unsupported"));
                    }
                    let secure = c.get_property(req, "secure")?;
                    let secure =
                        c.value_kind(secure)? == ModuleValueKind::Bool && c.as_bool(secure)?;
                    let token = c.get_private(callee, TOKEN)?;
                    let token = c.as_string(token)?;
                    let result = match request(
                        c,
                        json!({"op":"commit","secret":config["secret"],"token":token,"data":data,"maxAge":config["cookie"]["maxAge"],"secure":config["cookie"]["secure"],"https":secure}),
                    )? {
                        Ok(v) => v,
                        Err(e) => return Ok(error(e)),
                    };
                    if let Some(cookie) = result["setCookie"].as_str() {
                        let set = c.get_property(res, "setHeader")?;
                        let get = c.get_property(res, "getHeader")?;
                        let name = c.string("Set-Cookie")?;
                        let existing = c.call(get, res, &[name])?;
                        let value = c.string(cookie)?;
                        let value = match c.value_kind(existing)? {
                            ModuleValueKind::Undefined => value,
                            ModuleValueKind::Array => {
                                c.array_push(existing, value)?;
                                existing
                            }
                            _ => {
                                let list = c.array()?;
                                c.array_push(list, existing)?;
                                c.array_push(list, value)?;
                                list
                            }
                        };
                        c.call(set, res, &[name, value])?;
                    }
                }
                let yes = c.bool(true)?;
                c.set_private(callee, COMMITTED, yes)?;
                let raw = c.get_private(callee, ORIGINAL_END)?;
                c.call(raw, res, args)?;
                Ok(ModuleCallResult::Return(c.undefined()))
            }
            4 => Ok(error(
                "streaming responses are unsupported with baseline express-session",
            )),
            _ => Err(ModuleError::ContractViolation(
                "unknown session function".into(),
            )),
        }
    }
    fn resume(
        &self,
        _: ModuleContinuation,
        _: &[ValueHandle],
        _: Result<ValueHandle, String>,
        _: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "sessions require synchronous host completion".into(),
        ))
    }
    fn event(
        &self,
        _: u32,
        _: ValueHandle,
        _: ValueHandle,
        _: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "sessions have no events".into(),
        ))
    }
}
