//! Database-backed Express sessions. Time, randomness and storage belong to the host.
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
const DESTROYED: u32 = 8;
const HOOK: u32 = 9;
const OWNER: u32 = 10;
const MESSAGE: u32 = 11;
const PENDING_COOKIE: u32 = 12;

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
                state_version: 2,
                imports: vec!["express-session".into()],
                capabilities: vec![HostCapabilityDescriptor {
                    id: SESSION_REQUEST.into(),
                    contract_version: 2,
                    completion: CompletionMode::Sync,
                    schema: "jjs.session.v2".into(),
                }],
                dependencies: vec![],
                function_keys: (1..=7).collect(),
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}
fn error(s: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: "Error".into(),
        message: format!("express-session: {}", s.into()),
    }
}
fn unsupported_option(name: &str) -> String {
    let help = match name {
        "rolling" => "Expiry renews only when session data changes. Remove rolling only if that behavior fits your app.",
        "store" => "Envhost supplies persistent SQLite or MySQL storage. Configure the database in Envhost and omit store.",
        "genid" => "Envhost generates cryptographically random session IDs. Omit genid.",
        "proxy" => "Configure ENVHOST_TRUSTED_PROXY_IPS in Envhost; the application cannot override proxy trust.",
        "unset" => "Use req.session.destroy(callback) to delete a session, or leave req.session unchanged to keep it.",
        _ => "Supported options are secret, resave, saveUninitialized, name and cookie. Check the option spelling.",
    };
    format!("Option '{name}' is not supported. {help}")
}
fn unsupported_cookie(name: &str) -> String {
    let help = match name {
        "expires" => "Set cookie.maxAge in milliseconds instead (1 millisecond to 30 days).",
        "domain" => "Cookies are limited to the current hostname. Cross-domain cookies are not supported.",
        "partitioned" | "priority" => "This cookie attribute is not implemented. Omit it only if your app does not need it.",
        _ => "Supported cookie options are httpOnly, secure, sameSite, maxAge and path. Check the option spelling.",
    };
    format!("Cookie option '{name}' is not supported. {help}")
}
fn stub(c: &mut dyn ModuleContext, message: &str) -> Result<ValueHandle, ModuleError> {
    let f = c.function(ModuleFunctionKey(7))?;
    let message = c.string(message)?;
    c.set_private(f, MESSAGE, message)?;
    Ok(f)
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
// Native method handles must not become serialized session fields.
fn session_value(c: &mut dyn ModuleContext, value: ValueHandle) -> Result<Value, ModuleError> {
    if c.value_kind(value)? == ModuleValueKind::Null {
        return Ok(Value::Null);
    }
    let data = c.object()?;
    for name in c.own_property_names(value)? {
        let property = c.get_property(value, &name)?;
        if ["destroy", "regenerate", "save", "reload", "touch"].contains(&name.as_str()) {
            if !c.is_callable(property) {
                return Err(ModuleError::ContractViolation(format!("express-session: req.session.{name} is reserved for a session method. Store application data under a different property name.")));
            }
            continue;
        }
        c.set_property(data, &name, property)?;
    }
    json_value(c, data)
}
fn validate(v: &Value) -> Result<(), String> {
    let o = v.as_object().ok_or("Pass one session options object.")?;
    for k in o.keys() {
        if !["secret", "resave", "saveUninitialized", "name", "cookie"].contains(&k.as_str()) {
            return Err(unsupported_option(k));
        }
    }
    let valid_secret = |s: &Value| s.as_str().is_some_and(|s| !s.trim().is_empty());
    if !valid_secret(&v["secret"])
        && !v["secret"]
            .as_array()
            .is_some_and(|a| !a.is_empty() && a.iter().all(valid_secret))
    {
        return Err("secret must be a non-empty string or a non-empty array of non-empty strings. Configure SESSION_SECRET in Envhost. For rotation, put the new secret first and retain previous secrets while their cookies remain valid.".into());
    }
    for name in ["resave", "saveUninitialized"] {
        if v[name] != false {
            return Err(format!("Set {name}: false explicitly. Other values are not supported; sessions are saved only when changed."));
        }
    }
    if let Some(name) = o.get("name") {
        if !name.as_str().is_some_and(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        }) {
            return Err("name must be a non-empty HTTP cookie name, without spaces, separators or control characters. Example: name: 'myapp.sid'.".into());
        }
    }
    let cookie = v["cookie"].as_object().ok_or(
        "cookie must be an options object; per-request cookie functions are not supported.",
    )?;
    for k in cookie.keys() {
        if !["httpOnly", "secure", "sameSite", "maxAge", "path"].contains(&k.as_str()) {
            return Err(unsupported_cookie(k));
        }
    }
    if v["cookie"]["httpOnly"] != true {
        return Err(
            "Set cookie.httpOnly: true. JavaScript-readable session cookies are not supported."
                .into(),
        );
    }
    if !v["cookie"]["secure"].is_boolean() {
        return Err("cookie.secure must be true or false; 'auto' is not supported. Use true for HTTPS sites.".into());
    }
    if v["cookie"]["sameSite"] != "lax" {
        return Err("Only cookie.sameSite: 'lax' is supported. Other modes are not implemented; do not substitute 'lax' if your app requires cross-site cookies.".into());
    }
    if !v["cookie"]["maxAge"]
        .as_u64()
        .is_some_and(|n| n > 0 && n <= 2_592_000_000)
    {
        return Err("cookie.maxAge must be an integer from 1 to 2592000000 milliseconds (30 days). Browser-session cookies and longer lifetimes are not supported.".into());
    }
    if let Some(path) = cookie.get("path") {
        if !path.as_str().is_some_and(|s| {
            s.starts_with('/') && s.bytes().all(|b| (0x20..=0x7e).contains(&b) && b != b';')
        }) {
            return Err("cookie.path must start with '/' and contain only printable ASCII, without semicolons or control characters. Example: cookie.path: '/myapp'.".into());
        }
    }
    Ok(())
}
fn append_cookie(
    c: &mut dyn ModuleContext,
    res: ValueHandle,
    cookie: &str,
) -> Result<(), ModuleError> {
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
    Ok(())
}
fn install_session(
    c: &mut dyn ModuleContext,
    hook: ValueHandle,
    result: &Value,
    config: &Value,
) -> Result<(), ModuleError> {
    let req = c.get_private(hook, REQUEST)?;
    let id = c.string(
        result["id"]
            .as_str()
            .ok_or_else(|| ModuleError::ContractViolation("session host omitted id".into()))?,
    )?;
    c.set_property(req, "sessionID", id)?;
    let mut data = result["data"].clone();
    for name in ["destroy", "regenerate", "save", "reload", "touch"] {
        if data.get(name).is_some() {
            return Err(ModuleError::ContractViolation(format!("express-session: Stored session property '{name}' conflicts with a session method. Rename that application data field before upgrading.")));
        }
    }
    data["cookie"] = config["cookie"].clone();
    let encoded = c.string(&data.to_string())?;
    let session = c.json_parse(encoded)?;
    c.set_property(req, "session", session)?;
    c.set_private(hook, ORIGINAL, encoded)?;
    let token = c.string(
        result["token"]
            .as_str()
            .ok_or_else(|| ModuleError::ContractViolation("session host omitted token".into()))?,
    )?;
    c.set_private(hook, TOKEN, token)?;
    let no = c.bool(false)?;
    c.set_private(hook, DESTROYED, no)?;
    for (name, key) in [("destroy", 5), ("regenerate", 6)] {
        let f = c.function(ModuleFunctionKey(key))?;
        c.set_private(f, HOOK, hook)?;
        c.set_private(f, OWNER, session)?;
        c.set_property(session, name, f)?;
    }
    for (name, help) in [
        ("save", "Changes are saved automatically when the response ends. Explicit early saving is not implemented."),
        ("reload", "Reloading within a request is not implemented. Each new request loads the stored session."),
        ("touch", "Expiry renews when session data changes. Extending expiry without changing data is not implemented."),
    ] {
        let f = stub(c, &format!("req.session.{name}() is not supported. {help}"))?;
        c.set_property(session, name, f)?;
    }
    Ok(())
}
fn callback(
    c: &mut dyn ModuleContext,
    f: ValueHandle,
    failure: Option<&str>,
) -> Result<ModuleCallResult, ModuleError> {
    let receiver = c.undefined();
    if let Some(message) = failure {
        let constructor = c.global("Error")?;
        let message = c.string(&format!("express-session: {message}"))?;
        let err = c.call(constructor, receiver, &[message])?;
        c.call(f, receiver, &[err])?;
    } else {
        c.call(f, receiver, &[])?;
    }
    Ok(ModuleCallResult::Return(c.undefined()))
}
impl NativeModule for ExpressSessionModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn instantiate(&self, c: &mut dyn ModuleContext) -> Result<ModuleCallResult, ModuleError> {
        let f = c.function(ModuleFunctionKey(1))?;
        c.set_property(f, "default", f)?;
        for name in ["Store", "MemoryStore", "Session", "Cookie"] {
            let unsupported = stub(c, &format!("session.{name} is not supported. Use session(options); Envhost manages persistent storage and session creation."))?;
            c.set_property(f, name, unsupported)?;
        }
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
                if args.len() != 1 || c.value_kind(args[0])? != ModuleValueKind::Object { return Ok(error("Pass one session options object.")); }
                for name in c.own_property_names(args[0])? {
                    if !["secret", "resave", "saveUninitialized", "name", "cookie"].contains(&name.as_str()) { return Ok(error(unsupported_option(&name))); }
                }
                let cookie = c.get_property(args[0], "cookie")?;
                if c.value_kind(cookie)? != ModuleValueKind::Object { return Ok(error("cookie must be an options object; per-request cookie functions are not supported.")); }
                for name in c.own_property_names(cookie)? {
                    if !["httpOnly", "secure", "sameSite", "maxAge", "path"].contains(&name.as_str()) { return Ok(error(unsupported_cookie(&name))); }
                }
                // Validate optional fields before JSON serialization can omit undefined/functions.
                let names = c.own_property_names(args[0])?;
                if names.iter().any(|n| n == "name") {
                    let value = c.get_property(args[0], "name")?;
                    if c.value_kind(value)? != ModuleValueKind::String { return Ok(error("name must be a non-empty HTTP cookie name, for example 'myapp.sid'.")); }
                }
                if c.own_property_names(cookie)?.iter().any(|n| n == "path") {
                    let value = c.get_property(cookie, "path")?;
                    if c.value_kind(value)? != ModuleValueKind::String { return Ok(error("cookie.path must be a string starting with '/', for example '/myapp'.")); }
                }
                let mut options = json_value(c, args[0])?;
                if let Err(e) = validate(&options) { return Ok(error(e)); }
                if options.get("name").is_none() { options["name"] = json!("connect.sid"); }
                if options["cookie"].get("path").is_none() { options["cookie"]["path"] = json!("/"); }
                match request(c, json!({"op":"check","version":2}))? {
                    Err(e) => return Ok(error(e)),
                    Ok(reply) if reply["version"] != 2 => return Ok(error("The Envhost session host is incompatible with this module. Update Envhost and TPS together; this module requires session protocol version 2.")),
                    Ok(_) => {}
                }
                let f = c.function(ModuleFunctionKey(2))?;
                let options = c.string(&options.to_string())?;
                c.set_private(f, OPTIONS, options)?;
                Ok(ModuleCallResult::Return(f))
            }
            2 => {
                if args.len() != 3 { return Ok(error("Install session(options) with app.use(); middleware requires req, res and next.")); }
                let options = c.get_private(callee, OPTIONS)?;
                let config: Value = serde_json::from_str(&c.as_string(options)?).map_err(|e| ModuleError::ContractViolation(e.to_string()))?;
                let url = c.get_property(args[0], "url")?;
                let url = c.as_string(url)?;
                let path = url.split('?').next().unwrap_or("");
                let scope = config["cookie"]["path"].as_str().expect("validated cookie path");
                if !(path == scope || (path.starts_with(scope) && (scope.ends_with('/') || path.as_bytes().get(scope.len()) == Some(&b'/')))) {
                    let undefined = c.undefined();
                    c.call(args[2], undefined, &[])?;
                    return Ok(ModuleCallResult::Return(undefined));
                }
                let headers = c.get_property(args[0], "headers")?;
                let cookie = c.get_property(headers, "cookie")?;
                let cookie = if c.value_kind(cookie)? == ModuleValueKind::Undefined { String::new() } else { c.as_string(cookie)? };
                let result = match request(c, json!({"op":"load","secret":config["secret"],"cookie":cookie,"name":config["name"],"path":config["cookie"]["path"],"maxAge":config["cookie"]["maxAge"]}))? {
                    Ok(v) => v, Err(e) => return Ok(error(e)),
                };
                let hook = c.function(ModuleFunctionKey(3))?;
                let raw = c.get_property(args[1], "_expressRawEnd")?;
                if !c.is_callable(raw) { return Ok(error("Install this middleware on a JJS Express app; a compatible Express response is required.")); }
                c.set_private(hook, ORIGINAL_END, raw)?;
                c.set_private(hook, OPTIONS, options)?;
                c.set_private(hook, REQUEST, args[0])?;
                c.set_private(hook, RESPONSE, args[1])?;
                let no = c.bool(false)?;
                c.set_private(hook, COMMITTED, no)?;
                let empty = c.undefined();
                c.set_private(hook, PENDING_COOKIE, empty)?;
                install_session(c, hook, &result, &config)?;
                c.set_property(args[1], "_expressRawEnd", hook)?;
                let unsupported = c.function(ModuleFunctionKey(4))?;
                for name in ["_expressRawWrite", "_expressRawFlushHeaders", "writeHead"] { c.set_property(args[1], name, unsupported)?; }
                let undefined = c.undefined();
                c.call(args[2], undefined, &[])?;
                Ok(ModuleCallResult::Return(undefined))
            }
            3 => {
                let committed = c.get_private(callee, COMMITTED)?;
                if c.is_truthy(committed)? { return Ok(error("The response has already ended. Update the session before ending the response.")); }
                let req = c.get_private(callee, REQUEST)?;
                let res = c.get_private(callee, RESPONSE)?;
                let options = c.get_private(callee, OPTIONS)?;
                let config: Value = serde_json::from_str(&c.as_string(options)?).map_err(|e| ModuleError::ContractViolation(e.to_string()))?;
                let destroyed = c.get_private(callee, DESTROYED)?;
                if !c.is_truthy(destroyed)? {
                    let session = c.get_property(req, "session")?;
                    if !matches!(c.value_kind(session)?, ModuleValueKind::Object | ModuleValueKind::Null) {
                        return Ok(error("req.session must be an object or null. To remove it, use req.session.destroy(callback); deleting the property is not supported."));
                    }
                    let data = session_value(c, session)?;
                    let original = c.get_private(callee, ORIGINAL)?;
                    let original: Value = serde_json::from_str(&c.as_string(original)?).map_err(|e| ModuleError::ContractViolation(e.to_string()))?;
                    if data != original {
                        if !data.is_null() && data["cookie"] != original["cookie"] {
                            return Ok(error("Changing req.session.cookie during a request is not supported. Configure cookie options once in session({ cookie: ... })."));
                        }
                        let secure = c.get_property(req, "secure")?;
                        let https = c.value_kind(secure)? == ModuleValueKind::Bool && c.as_bool(secure)?;
                        let token = c.get_private(callee, TOKEN)?;
                        let token = c.as_string(token)?;
                        let result = match request(c, json!({"op":"commit","secret":config["secret"],"token":token,"data":data,"maxAge":config["cookie"]["maxAge"],"name":config["name"],"path":config["cookie"]["path"],"secure":config["cookie"]["secure"],"https":https}))? {
                            Ok(v) => v, Err(e) => return Ok(error(e)),
                        };
                        if let Some(cookie) = result["setCookie"].as_str() { let cookie = c.string(cookie)?; c.set_private(callee, PENDING_COOKIE, cookie)?; }
                    }
                }
                let pending = c.get_private(callee, PENDING_COOKIE)?;
                if c.value_kind(pending)? == ModuleValueKind::String {
                    let cookie = c.as_string(pending)?;
                    append_cookie(c, res, &cookie)?;
                }
                let yes = c.bool(true)?;
                c.set_private(callee, COMMITTED, yes)?;
                let raw = c.get_private(callee, ORIGINAL_END)?;
                c.call(raw, res, args)?;
                Ok(ModuleCallResult::Return(c.undefined()))
            }
            4 => Ok(error("Streaming and early response headers are not supported with express-session. Use res.send(), res.json() or res.end(); session changes must be saved before headers are sent.")),
            5 | 6 => {
                let name = if key.0 == 5 { "destroy" } else { "regenerate" };
                if args.len() != 1 || !c.is_callable(args[0]) { return Ok(error(format!("req.session.{name} requires a callback: req.session.{name}(function(err) {{ ... }}). Check err before continuing."))); }
                let hook = c.get_private(callee, HOOK)?;
                let req = c.get_private(hook, REQUEST)?;
                let current = c.get_property(req, "session")?;
                let owner = c.get_private(callee, OWNER)?;
                let committed = c.get_private(hook, COMMITTED)?;
                if c.is_truthy(committed)? || !c.strict_equals(current, owner)? {
                    return callback(c, args[0], Some("This session is no longer active. Use the current req.session before ending the response."));
                }
                let options = c.get_private(hook, OPTIONS)?;
                let config: Value = serde_json::from_str(&c.as_string(options)?).map_err(|e| ModuleError::ContractViolation(e.to_string()))?;
                let token = c.get_private(hook, TOKEN)?;
                let token = c.as_string(token)?;
                let secure = c.get_property(req, "secure")?;
                let https = c.value_kind(secure)? == ModuleValueKind::Bool && c.as_bool(secure)?;
                let result = match request(c, json!({"op":name,"secret":config["secret"],"token":token,"maxAge":config["cookie"]["maxAge"],"name":config["name"],"path":config["cookie"]["path"],"secure":config["cookie"]["secure"],"https":https}))? {
                    Ok(v) => v, Err(e) => return callback(c, args[0], Some(&e)),
                };
                if let Some(cookie) = result["setCookie"].as_str() { let cookie = c.string(cookie)?; c.set_private(hook, PENDING_COOKIE, cookie)?; }
                if key.0 == 5 {
                    let undefined = c.undefined();
                    c.set_property(req, "session", undefined)?;
                    let yes = c.bool(true)?;
                    c.set_private(hook, DESTROYED, yes)?;
                } else { install_session(c, hook, &result, &config)?; }
                callback(c, args[0], None)
            }
            7 => { let message = c.get_private(callee, MESSAGE)?; Ok(error(c.as_string(message)?)) }
            _ => Err(ModuleError::ContractViolation("unknown session function".into())),
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
#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> Value {
        json!({"secret":["new","old"],"resave":false,"saveUninitialized":false,"name":"app.sid","cookie":{"httpOnly":true,"secure":true,"sameSite":"lax","maxAge":1000,"path":"/app"}})
    }
    #[test]
    fn validates_rotation_and_cookie_scope_without_ignoring_invalid_values() {
        assert!(validate(&options()).is_ok());
        for secret in [json!([]), json!(["ok", ""]), json!(["ok", null]), json!(42)] {
            let mut v = options();
            v["secret"] = secret;
            assert!(validate(&v).unwrap_err().contains("secret"));
        }
        for name in ["", "bad name", "bad;name", "bad\r\nname"] {
            let mut v = options();
            v["name"] = json!(name);
            assert!(validate(&v).unwrap_err().contains("name"));
        }
        for path in ["", "app", "/app; Domain=evil", "/app\r\n"] {
            let mut v = options();
            v["cookie"]["path"] = json!(path);
            assert!(validate(&v).unwrap_err().contains("cookie.path"));
        }
    }
    #[test]
    fn unsupported_options_explain_the_next_step() {
        let mut v = options();
        v["rolling"] = json!(false);
        assert!(validate(&v).unwrap_err().contains("Expiry renews only"));
        let mut v = options();
        v["cookie"]["expires"] = json!(100);
        assert!(validate(&v).unwrap_err().contains("cookie.maxAge"));
    }
}
