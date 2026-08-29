//! Explicit host-owned readiness transitions for persistent JJS services.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    NativeModule, ValueHandle, MODULE_API_VERSION,
};

const READY: ModuleFunctionKey = ModuleFunctionKey(1);
const FAIL: ModuleFunctionKey = ModuleFunctionKey(2);
const RETURN_UNDEFINED: u32 = 1;
pub const SERVICE_READY: &str = "jjs:service/ready";
pub const SERVICE_FAIL: &str = "jjs:service/fail";
pub const MAX_FAILURE_MESSAGE_BYTES: usize = 1024;

pub fn capability_ids() -> [&'static str; 2] {
    [SERVICE_READY, SERVICE_FAIL]
}

pub struct ServiceModule {
    manifest: ModuleManifest,
}

impl Default for ServiceModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.tps-service".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-service-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["tps-service".into()],
                capabilities: capability_ids()
                    .into_iter()
                    .map(|id| HostCapabilityDescriptor {
                        id: id.into(),
                        contract_version: 1,
                        completion: CompletionMode::Sync,
                        schema: "jjs.service-readiness.v1".into(),
                    })
                    .collect(),
                dependencies: vec![],
                function_keys: vec![READY.0, FAIL.0],
                object_kind_keys: vec![],
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

fn request(
    context: &mut dyn ModuleContext,
    capability: &str,
    arguments: Vec<ValueHandle>,
) -> Result<ModuleCallResult, ModuleError> {
    context.request_host(
        HostRequestSpec {
            capability: capability.into(),
            operation: capability.into(),
            arguments,
        },
        ModuleContinuation(RETURN_UNDEFINED),
        vec![],
        false,
    )
}

impl NativeModule for ServiceModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        let ready = context.function(READY)?;
        let fail = context.function(FAIL)?;
        context.set_property(exports, "ready", ready)?;
        context.set_property(exports, "fail", fail)?;
        Ok(ModuleCallResult::Return(exports))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        _callee: ValueHandle,
        _receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        match key {
            READY => {
                if !args.is_empty() {
                    return Ok(thrown("tps-service ready accepts no arguments"));
                }
                request(context, SERVICE_READY, vec![])
            }
            FAIL => {
                if args.len() != 1 {
                    return Ok(thrown("tps-service fail requires one reason string"));
                }
                let reason = match context.as_string(args[0]) {
                    Ok(reason)
                        if !reason.is_empty() && reason.len() <= MAX_FAILURE_MESSAGE_BYTES =>
                    {
                        reason
                    }
                    _ => {
                        return Ok(thrown(format!(
                            "tps-service failure reason must be 1 through {MAX_FAILURE_MESSAGE_BYTES} bytes"
                        )));
                    }
                };
                let reason = context.string(&reason)?;
                request(context, SERVICE_FAIL, vec![reason])
            }
            _ => Err(ModuleError::ContractViolation(
                "unknown service readiness function key".into(),
            )),
        }
    }

    fn resume(
        &self,
        continuation: ModuleContinuation,
        _state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if continuation.0 != RETURN_UNDEFINED {
            return Err(ModuleError::ContractViolation(
                "unknown service readiness continuation".into(),
            ));
        }
        match completion {
            Ok(_) => Ok(ModuleCallResult::Return(context.undefined())),
            Err(message) => Ok(ModuleCallResult::Throw {
                name: "ServiceReadinessError".into(),
                message,
            }),
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
            "service readiness has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_exact_sync_host_contract() {
        let module = ServiceModule::default();
        assert_eq!(module.manifest.imports, ["tps-service"]);
        assert_eq!(module.manifest.function_keys, [READY.0, FAIL.0]);
        assert!(module
            .manifest
            .capabilities
            .iter()
            .all(|capability| capability.completion == CompletionMode::Sync));
    }
}
