//! Bounded Nodemailer-compatible session transport. SES credentials remain host-owned.
use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};
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
                    implementation: "jjs-module-nodemailer-v1".into(),
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
                function_keys: vec![1, 2],
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}
fn fail(message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: "EmailSendError".into(),
        message: message.into(),
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
        if args.len() != 1 || c.value_kind(args[0])? != ModuleValueKind::Object {
            return Ok(fail(
                "exactly one object is required; only Promise sendMail is supported",
            ));
        }
        let encoded = match c.json_stringify(args[0]) {
            Ok(v) => v,
            Err(_) => return Ok(fail("message must be acyclic JSON")),
        };
        let text = c.as_string(encoded)?;
        match key {
            CREATE => {
                if serde_json::from_str::<serde_json::Value>(&text).ok()
                    != Some(serde_json::json!({"session":true}))
                {
                    return Ok(fail("only createTransport({ session: true }) is supported"));
                }
                let o = c.object()?;
                let f = c.function(SEND)?;
                c.set_property(o, "sendMail", f)?;
                Ok(ModuleCallResult::Return(o))
            }
            SEND => {
                if text.len() > 256 * 1024 {
                    return Ok(fail("message exceeds 256 KiB"));
                }
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
            Err(e) => Ok(fail(e)),
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
