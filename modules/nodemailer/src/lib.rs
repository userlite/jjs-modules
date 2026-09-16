//! Bounded Nodemailer-compatible session transport. SES credentials remain host-owned.
use jjs_email_contract::{decode_message, EmailError, MAX_MESSAGE_BYTES};
use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};
use serde_json::Value;

const CREATE: ModuleFunctionKey = ModuleFunctionKey(1);
const SEND: ModuleFunctionKey = ModuleFunctionKey(2);
pub const EMAIL_ENQUEUE: &str = "jjs:email/enqueue";
pub struct NodemailerModule {
    manifest: ModuleManifest,
}
impl Default for NodemailerModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.nodemailer".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-nodemailer-v2".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["nodemailer".into()],
                capabilities: vec![HostCapabilityDescriptor {
                    id: EMAIL_ENQUEUE.into(),
                    contract_version: 1,
                    completion: CompletionMode::Yield,
                    schema: "jjs.email.enqueue.v1".into(),
                }],
                dependencies: vec![],
                function_keys: vec![1, 2, 3, 4, 5, 6, 7],
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}
fn fail(
    c: &mut dyn ModuleContext,
    e: EmailError,
    promise: bool,
) -> Result<ModuleCallResult, ModuleError> {
    let constructor = c.global("Error")?;
    let message = c.string(&e.message)?;
    let receiver = c.undefined();
    let value = c.call(constructor, receiver, &[message])?;
    for (key, text) in [("code", &e.code), ("field", &e.field)] {
        let text = c.string(text)?;
        c.set_property(value, key, text)?;
    }
    if promise {
        let class = c.global("Promise")?;
        let reject = c.get_property(class, "reject")?;
        Ok(ModuleCallResult::Return(c.call(reject, class, &[value])?))
    } else {
        Ok(ModuleCallResult::ThrowValue(value))
    }
}
fn host_error(message: String) -> EmailError {
    if let Ok(e) = serde_json::from_str::<EmailError>(&message) {
        return e;
    }
    let (code, field) = if message.starts_with("email_disabled:") {
        ("EEMAILDISABLED", "session")
    } else if message.starts_with("email_unavailable:") || message == "email transport is disabled"
    {
        ("EEMAILUNAVAILABLE", "transport")
    } else if message.starts_with("enqueue_unknown:") {
        ("EQUEUEUNKNOWN", "idempotencyKey")
    } else if message.starts_with("operation_conflict:") {
        ("EIDEMPOTENCYCONFLICT", "idempotencyKey")
    } else if message == "ECAPABILITY" {
        ("ECAPABILITY", "transport")
    } else {
        ("EEMAILHOST", "transport")
    };
    EmailError::new(code, field, message)
}
// Read actual guest values, without invoking JSON.toJSON or discarding undefined/functions.
fn input(
    c: &mut dyn ModuleContext,
    value: ValueHandle,
    field: &str,
    stack: &mut Vec<ValueHandle>,
    budget: &mut usize,
) -> Result<Value, EmailError> {
    fn internal(_: ModuleError) -> EmailError {
        EmailError::new("EINVALIDTYPE", "message", "Could not read message data")
    }
    if stack.len() > 16 {
        return Err(EmailError::new(
            "EINVALIDVALUE",
            field,
            "Message nesting exceeds 16 levels",
        ));
    }
    if *budget == 0 {
        return Err(EmailError::new(
            "EMESSAGESIZE",
            field,
            format!("Message exceeds {MAX_MESSAGE_BYTES} bytes"),
        ));
    }
    *budget -= 1;
    c.charge_fuel(1).map_err(internal)?;
    match c.value_kind(value).map_err(internal)? {
        ModuleValueKind::Undefined | ModuleValueKind::Function => Err(EmailError::new("EINVALIDTYPE", field, format!("{field} cannot contain undefined or functions; omit optional fields or supply supported values"))),
        ModuleValueKind::Null => Ok(Value::Null),
        ModuleValueKind::Bool => Ok(Value::Bool(c.as_bool(value).map_err(internal)?)),
        ModuleValueKind::Number => serde_json::Number::from_f64(c.as_number(value).map_err(internal)?)
            .map(Value::Number).ok_or_else(||EmailError::new("EINVALIDTYPE", field, "Numbers must be finite")),
        ModuleValueKind::String => {
            let s = c.as_string(value).map_err(internal)?;
            if s.len() > *budget { return Err(EmailError::new("EMESSAGESIZE", field, format!("Message exceeds {MAX_MESSAGE_BYTES} bytes"))); }
            *budget -= s.len(); Ok(Value::String(s))
        }
        ModuleValueKind::Object | ModuleValueKind::Array => {
            for parent in stack.iter() {
                if c.strict_equals(*parent, value).map_err(internal)? {
                    return Err(EmailError::new("ECIRCULAR", field, "Circular references are not supported in email messages"));
                }
            }
            if c.is_bytes(value) {
                if !field.starts_with("attachments[") || !field.ends_with("].content") {
                    return Err(EmailError::new("EINVALIDTYPE", field, "Buffer values are supported only as attachment content; use strings for text and html"));
                }
                let bytes = c.read_bytes(value).map_err(internal)?;
                if bytes.len() > *budget { return Err(EmailError::new("EMESSAGESIZE", field, format!("Message exceeds {MAX_MESSAGE_BYTES} bytes"))); }
                *budget -= bytes.len();
                return Ok(serde_json::json!({"type":"Buffer","data":bytes}));
            }
            stack.push(value);
            let result = if c.value_kind(value).map_err(internal)? == ModuleValueKind::Array {
                let len = c.array_len(value).map_err(internal)?;
                if len > *budget { return Err(EmailError::new("EMESSAGESIZE", field, "Array exceeds message size limit")); }
                let mut values = Vec::new();
                for i in 0..len {
                    let child = c.array_get(value, i).map_err(internal)?;
                    values.push(input(c, child, &format!("{field}[{i}]"), stack, budget)?);
                }
                Value::Array(values)
            } else {
                let mut values = serde_json::Map::new();
                for key in c.own_property_names(value).map_err(internal)? {
                    if key.len() > *budget { return Err(EmailError::new("EMESSAGESIZE", field, "Property name exceeds message size limit")); }
                    *budget -= key.len();
                    let child = c.get_property(value, &key).map_err(internal)?;
                    let path = if field == "message" || field == "transport" { key.clone() } else { format!("{field}.{key}") };
                    values.insert(key, input(c, child, &path, stack, budget)?);
                }
                Value::Object(values)
            };
            stack.pop(); Ok(result)
        }
    }
}
impl NativeModule for NodemailerModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn instantiate(&self, c: &mut dyn ModuleContext) -> Result<ModuleCallResult, ModuleError> {
        let o = c.object()?;
        let f = c.function(CREATE)?;
        c.set_property(o, "createTransport", f)?;
        for (name, key) in [("createTestAccount", 6), ("getTestMessageUrl", 7)] {
            let f = c.function(ModuleFunctionKey(key))?;
            c.set_property(o, name, f)?;
        }
        Ok(ModuleCallResult::Return(o))
    }
    fn call(
        &self,
        key: ModuleFunctionKey,
        _callee: ValueHandle,
        _receiver: ValueHandle,
        args: &[ValueHandle],
        c: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if (3..=7).contains(&key.0) {
            let method = match key.0 {
                3 => "verify",
                4 => "use",
                5 => "close",
                6 => "createTestAccount",
                _ => "getTestMessageUrl",
            };
            return fail(c, EmailError::new("EUNSUPPORTEDMETHOD", method, format!("{method}() is not supported by the Envhost session transport; use sendMail() to enqueue email and Envhost email history to inspect outcomes")), false);
        }
        if args.len() > 1 {
            let (code, field, message) = if key == SEND {
                ("EUNSUPPORTEDCALLBACK", "sendMail.callback", "Callbacks are not supported; use await transport.sendMail(message) or .then()/.catch()")
            } else {
                (
                    "EUNSUPPORTEDARGUMENT",
                    "createTransport.defaults",
                    "Transport defaults are not supported; pass fields directly to sendMail()",
                )
            };
            return fail(c, EmailError::new(code, field, message), key == SEND);
        }
        if args.len() != 1 || c.value_kind(args[0])? != ModuleValueKind::Object {
            return fail(
                c,
                EmailError::new(
                    "EINVALIDTYPE",
                    if key == CREATE {
                        "transport"
                    } else {
                        "message"
                    },
                    "Exactly one options object is required",
                ),
                key == SEND,
            );
        }
        let mut budget = MAX_MESSAGE_BYTES;
        let value = match input(
            c,
            args[0],
            if key == CREATE {
                "transport"
            } else {
                "message"
            },
            &mut vec![],
            &mut budget,
        ) {
            Ok(v) => v,
            Err(e) => return fail(c, e, key == SEND),
        };
        match key {
            CREATE => {
                let options = value.as_object().expect("guest object");
                for name in options.keys() {
                    if name != "session" {
                        return fail(c, EmailError::new("EUNSUPPORTEDFIELD", &format!("transport.{name}"), format!("Unsupported transport field {name}; only createTransport({{ session: true }}) is supported, with SES credentials configured in Envhost")), false);
                    }
                }
                if value != serde_json::json!({"session":true}) {
                    return fail(c, EmailError::new("ETRANSPORT", "transport.session", "Use createTransport({ session: true }); SMTP and custom transports are not supported"), false);
                }
                let o = c.object()?;
                for (name, key) in [("sendMail", 2), ("verify", 3), ("use", 4), ("close", 5)] {
                    let f = c.function(ModuleFunctionKey(key))?;
                    c.set_property(o, name, f)?;
                }
                Ok(ModuleCallResult::Return(o))
            }
            SEND => {
                let text = value.to_string();
                if let Err(e) = decode_message(&text) {
                    return fail(c, host_error(e), true);
                }
                let encoded = c.string(&text)?;
                c.request_host(
                    HostRequestSpec {
                        capability: EMAIL_ENQUEUE.into(),
                        operation: EMAIL_ENQUEUE.into(),
                        arguments: vec![encoded],
                    },
                    ModuleContinuation(1),
                    vec![],
                    true,
                )
            }
            _ => Err(ModuleError::ContractViolation(
                "unknown nodemailer function".into(),
            )),
        }
    }

    fn resume(
        &self,
        k: ModuleContinuation,
        _state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        c: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if k.0 != 1 {
            return Err(ModuleError::ContractViolation(
                "unknown nodemailer continuation".into(),
            ));
        }
        match completion {
            Ok(v) => Ok(ModuleCallResult::Return(c.json_parse(v)?)),
            // A yielding call owns a Promise; return a rejected Promise so the
            // runtime adopts its original Error value without stringifying it.
            Err(e) => fail(c, host_error(e), true),
        }
    }
    fn event(
        &self,
        _event: u32,
        _target: ValueHandle,
        _payload: ValueHandle,
        _c: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "nodemailer has no guest events".into(),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn email_is_a_separate_yielding_capability() {
        let m = NodemailerModule::default();
        assert_eq!(m.manifest.imports, ["nodemailer"]);
        assert_eq!(m.manifest.capabilities[0].id, EMAIL_ENQUEUE);
        assert_eq!(m.manifest.capabilities[0].completion, CompletionMode::Yield);
    }
}
