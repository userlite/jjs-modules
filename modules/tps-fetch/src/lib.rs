//! Strict guest surface for host-owned bounded outbound HTTP.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};

const FETCH: ModuleFunctionKey = ModuleFunctionKey(1);
const RETURN_RESPONSE: u32 = 1;

/// Version-1 host capability used by TPS and conforming hosts.
pub const OUTBOUND_HTTP_FETCH: &str = "jjs:outbound-http/fetch";

pub fn capability_ids() -> [&'static str; 1] {
    [OUTBOUND_HTTP_FETCH]
}

pub struct TpsFetchModule {
    manifest: ModuleManifest,
}

impl Default for TpsFetchModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.tps-fetch".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-tps-fetch-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["tps-fetch".into()],
                capabilities: vec![HostCapabilityDescriptor {
                    id: OUTBOUND_HTTP_FETCH.into(),
                    contract_version: 1,
                    completion: CompletionMode::Yield,
                    schema: "jjs.outbound-http.request.v1".into(),
                }],
                dependencies: vec![],
                function_keys: vec![FETCH.0],
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

impl NativeModule for TpsFetchModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        let fetch = context.function(FETCH)?;
        context.set_property(exports, "fetch", fetch)?;
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
        if key != FETCH {
            return Err(ModuleError::ContractViolation(
                "unknown tps-fetch function key".into(),
            ));
        }
        if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Object {
            return Ok(thrown("tps-fetch fetch requires exactly one request object"));
        }
        let encoded = match context.json_stringify(args[0]) {
            Ok(encoded) => encoded,
            Err(_) => return Ok(thrown("tps-fetch request must be JSON-compatible and acyclic")),
        };
        context.request_host(
            HostRequestSpec {
                capability: OUTBOUND_HTTP_FETCH.into(),
                operation: OUTBOUND_HTTP_FETCH.into(),
                arguments: vec![encoded],
            },
            ModuleContinuation(RETURN_RESPONSE),
            vec![],
            true,
        )
    }

    fn resume(
        &self,
        continuation: ModuleContinuation,
        _state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if continuation.0 != RETURN_RESPONSE {
            return Err(ModuleError::ContractViolation(
                "unknown tps-fetch continuation".into(),
            ));
        }
        match completion {
            Ok(encoded) => match context.json_parse(encoded) {
                Ok(response) => Ok(ModuleCallResult::Return(response)),
                Err(_) => Err(ModuleError::ContractViolation(
                    "tps-fetch host completion was not valid JSON".into(),
                )),
            },
            Err(message) => Ok(ModuleCallResult::Throw {
                name: "TpsFetchError".into(),
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
            "tps-fetch has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_exact_yielding_host_contract() {
        let module = TpsFetchModule::default();
        assert_eq!(module.manifest.imports, ["tps-fetch"]);
        assert_eq!(module.manifest.function_keys, [FETCH.0]);
        assert_eq!(module.manifest.capabilities.len(), 1);
        assert_eq!(module.manifest.capabilities[0].id, OUTBOUND_HTTP_FETCH);
        assert_eq!(module.manifest.capabilities[0].contract_version, 1);
        assert_eq!(
            module.manifest.capabilities[0].completion,
            CompletionMode::Yield
        );
        assert_eq!(
            module.manifest.capabilities[0].schema,
            "jjs.outbound-http.request.v1"
        );
    }
}
