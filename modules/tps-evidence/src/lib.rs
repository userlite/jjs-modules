//! Strict guest surface for a host-owned, trusted network observation.
//!
//! The guest supplies only an operation identifier and outbound request. The
//! host must inject environment, program, code, and triggering-event identity;
//! accepting those fields from the guest would make the receipt untrustworthy.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};
use serde_json::Value;

const OBSERVE: ModuleFunctionKey = ModuleFunctionKey(1);
const RETURN_OBSERVATION: u32 = 1;

/// Version-1 capability implemented by TPS and every conforming host.
pub const EVIDENCE_OBSERVE: &str = "jjs:evidence/observe";

pub fn capability_ids() -> [&'static str; 1] {
    [EVIDENCE_OBSERVE]
}

pub struct TpsEvidenceModule {
    manifest: ModuleManifest,
}

impl Default for TpsEvidenceModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.tps-evidence".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-tps-evidence-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["tps-evidence".into()],
                capabilities: vec![HostCapabilityDescriptor {
                    id: EVIDENCE_OBSERVE.into(),
                    contract_version: 1,
                    completion: CompletionMode::Yield,
                    schema: "tps.evidence.observe.v1".into(),
                }],
                dependencies: vec![],
                function_keys: vec![OBSERVE.0],
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

fn validate_guest_request(encoded: &str) -> Result<(), &'static str> {
    let Value::Object(root) = serde_json::from_str(encoded)
        .map_err(|_| "tps-evidence request must be JSON-compatible and acyclic")?
    else {
        return Err("tps-evidence observe requires exactly one request object");
    };
    if root.len() != 2 || !root.contains_key("operationId") || !root.contains_key("request") {
        return Err("tps-evidence request must contain exactly operationId and request");
    }
    let operation_id = root
        .get("operationId")
        .and_then(Value::as_str)
        .ok_or("tps-evidence operationId must be a string")?;
    if operation_id.is_empty() || operation_id.len() > 128 {
        return Err("tps-evidence operationId length must be 1..=128 bytes");
    }
    if !root.get("request").is_some_and(Value::is_object) {
        return Err("tps-evidence request must be an outbound request object");
    }
    Ok(())
}

impl NativeModule for TpsEvidenceModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        let observe = context.function(OBSERVE)?;
        context.set_property(exports, "observe", observe)?;
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
        if key != OBSERVE {
            return Err(ModuleError::ContractViolation(
                "unknown tps-evidence function key".into(),
            ));
        }
        if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Object {
            return Ok(thrown(
                "tps-evidence observe requires exactly one request object",
            ));
        }
        let encoded = match context.json_stringify(args[0]) {
            Ok(encoded) => encoded,
            Err(_) => {
                return Ok(thrown(
                    "tps-evidence request must be JSON-compatible and acyclic",
                ))
            }
        };
        let encoded_text = context.as_string(encoded)?;
        if let Err(message) = validate_guest_request(&encoded_text) {
            return Ok(thrown(message));
        }
        context.request_host(
            HostRequestSpec {
                capability: EVIDENCE_OBSERVE.into(),
                operation: EVIDENCE_OBSERVE.into(),
                arguments: vec![encoded],
            },
            ModuleContinuation(RETURN_OBSERVATION),
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
        if continuation.0 != RETURN_OBSERVATION {
            return Err(ModuleError::ContractViolation(
                "unknown tps-evidence continuation".into(),
            ));
        }
        match completion {
            Ok(encoded) => match context.json_parse(encoded) {
                Ok(observation) => Ok(ModuleCallResult::Return(observation)),
                Err(_) => Err(ModuleError::ContractViolation(
                    "tps-evidence host completion was not valid JSON".into(),
                )),
            },
            Err(message) => Ok(ModuleCallResult::Throw {
                name: "TpsEvidenceError".into(),
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
            "tps-evidence has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_exact_yielding_host_contract() {
        let module = TpsEvidenceModule::default();
        assert_eq!(module.manifest.imports, ["tps-evidence"]);
        assert_eq!(module.manifest.function_keys, [OBSERVE.0]);
        assert_eq!(module.manifest.capabilities.len(), 1);
        assert_eq!(module.manifest.capabilities[0].id, EVIDENCE_OBSERVE);
        assert_eq!(module.manifest.capabilities[0].contract_version, 1);
        assert_eq!(
            module.manifest.capabilities[0].completion,
            CompletionMode::Yield
        );
        assert_eq!(
            module.manifest.capabilities[0].schema,
            "tps.evidence.observe.v1"
        );
    }

    #[test]
    fn accepts_only_guest_owned_top_level_fields() {
        assert!(validate_guest_request(
            r#"{"operationId":"watch-1","request":{"url":"https://example.test"}}"#
        )
        .is_ok());
        for field in ["environmentId", "programId", "codeSha256", "inputEventId"] {
            let encoded =
                format!(r#"{{"operationId":"watch-1","request":{{}},"{field}":"forged"}}"#);
            assert_eq!(
                validate_guest_request(&encoded),
                Err("tps-evidence request must contain exactly operationId and request")
            );
        }
    }

    #[test]
    fn rejects_invalid_operation_and_request_shapes() {
        assert!(validate_guest_request(r#"{"operationId":"","request":{}}"#).is_err());
        assert!(validate_guest_request(r#"{"operationId":"x","request":[]}"#).is_err());
        assert!(validate_guest_request(r#"{"operationId":"x"}"#).is_err());
    }
}
