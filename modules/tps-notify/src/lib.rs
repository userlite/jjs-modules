//! Strict guest surface for host-owned durable notification delivery.
//!
//! The guest can name only an operation, opaque destination handle, bounded
//! message, and the trusted evidence receipt it is reporting. Environment,
//! provider, attempt, time, retry, and receipt identity remain host-owned.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};
use serde_json::Value;

const SEND: ModuleFunctionKey = ModuleFunctionKey(1);
const RETURN_ACCEPTANCE: u32 = 1;
const MAX_OPERATION_ID_BYTES: usize = 128;
const MAX_DESTINATION_ID_BYTES: usize = 128;
const MAX_SUBJECT_BYTES: usize = 512;
const MAX_BODY_BYTES: usize = 128 * 1024;
const MAX_CONTENT_TYPE_BYTES: usize = 128;

/// Version-1 capability implemented by TPS and every conforming host.
pub const NOTIFICATION_SEND: &str = "jjs:notification/send";

pub fn capability_ids() -> [&'static str; 1] {
    [NOTIFICATION_SEND]
}

pub struct TpsNotifyModule {
    manifest: ModuleManifest,
}

impl Default for TpsNotifyModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.tps-notify".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-tps-notify-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["tps-notify".into()],
                capabilities: vec![HostCapabilityDescriptor {
                    id: NOTIFICATION_SEND.into(),
                    contract_version: 1,
                    completion: CompletionMode::Yield,
                    schema: "tps.notification.send.v1".into(),
                }],
                dependencies: vec![],
                function_keys: vec![SEND.0],
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

fn bounded_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
    minimum: usize,
    maximum: usize,
) -> Result<&'a str, &'static str> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or("tps-notify string field is missing or invalid")?;
    if value.len() < minimum || value.len() > maximum {
        return Err("tps-notify string field exceeds its version-1 byte bounds");
    }
    Ok(value)
}

fn validate_guest_request(encoded: &str) -> Result<(), &'static str> {
    let Value::Object(root) = serde_json::from_str(encoded)
        .map_err(|_| "tps-notify request must be JSON-compatible and acyclic")?
    else {
        return Err("tps-notify send requires exactly one request object");
    };
    let required = [
        "operationId",
        "destination",
        "subject",
        "body",
        "contentType",
        "evidenceReceiptSha256",
    ];
    if root.len() != required.len() || !required.iter().all(|field| root.contains_key(*field)) {
        return Err("tps-notify request contains missing or unknown fields");
    }
    bounded_string(&root, "operationId", 1, MAX_OPERATION_ID_BYTES)?;
    bounded_string(&root, "subject", 1, MAX_SUBJECT_BYTES)?;
    bounded_string(&root, "body", 1, MAX_BODY_BYTES)?;
    bounded_string(&root, "contentType", 1, MAX_CONTENT_TYPE_BYTES)?;
    let digest = bounded_string(&root, "evidenceReceiptSha256", 64, 64)?;
    if !digest
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("tps-notify evidenceReceiptSha256 must be 64 lowercase hexadecimal bytes");
    }

    let Some(Value::Object(destination)) = root.get("destination") else {
        return Err("tps-notify destination must be an opaque handle object");
    };
    if destination.len() != 2
        || !destination.contains_key("id")
        || !destination.contains_key("generation")
    {
        return Err("tps-notify destination must contain exactly id and generation");
    }
    bounded_string(destination, "id", 1, MAX_DESTINATION_ID_BYTES)?;
    let generation = destination
        .get("generation")
        .and_then(Value::as_u64)
        .ok_or("tps-notify destination generation must be a positive safe integer")?;
    if generation == 0 || generation > 9_007_199_254_740_991 {
        return Err("tps-notify destination generation must be a positive safe integer");
    }
    Ok(())
}

impl NativeModule for TpsNotifyModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        let send = context.function(SEND)?;
        context.set_property(exports, "send", send)?;
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
        if key != SEND {
            return Err(ModuleError::ContractViolation(
                "unknown tps-notify function key".into(),
            ));
        }
        if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Object {
            return Ok(thrown(
                "tps-notify send requires exactly one request object",
            ));
        }
        let encoded = match context.json_stringify(args[0]) {
            Ok(encoded) => encoded,
            Err(_) => {
                return Ok(thrown(
                    "tps-notify request must be JSON-compatible and acyclic",
                ));
            }
        };
        let encoded_text = context.as_string(encoded)?;
        if let Err(message) = validate_guest_request(&encoded_text) {
            return Ok(thrown(message));
        }
        context.request_host(
            HostRequestSpec {
                capability: NOTIFICATION_SEND.into(),
                operation: NOTIFICATION_SEND.into(),
                arguments: vec![encoded],
            },
            ModuleContinuation(RETURN_ACCEPTANCE),
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
        if continuation.0 != RETURN_ACCEPTANCE {
            return Err(ModuleError::ContractViolation(
                "unknown tps-notify continuation".into(),
            ));
        }
        match completion {
            Ok(encoded) => match context.json_parse(encoded) {
                Ok(receipt) => Ok(ModuleCallResult::Return(receipt)),
                Err(_) => Err(ModuleError::ContractViolation(
                    "tps-notify host completion was not valid JSON".into(),
                )),
            },
            Err(message) => Ok(ModuleCallResult::Throw {
                name: "TpsNotifyError".into(),
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
            "tps-notify has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> String {
        serde_json::json!({
            "operationId": "guardian-42-alert-17",
            "destination": {"id": "owner-primary", "generation": 1},
            "subject": "Watchtower condition detected",
            "body": "The monitored filing changed.",
            "contentType": "text/plain; charset=utf-8",
            "evidenceReceiptSha256": "a".repeat(64)
        })
        .to_string()
    }

    #[test]
    fn declares_exact_yielding_host_contract() {
        let module = TpsNotifyModule::default();
        assert_eq!(module.manifest.imports, ["tps-notify"]);
        assert_eq!(module.manifest.function_keys, [SEND.0]);
        assert_eq!(module.manifest.capabilities.len(), 1);
        assert_eq!(module.manifest.capabilities[0].id, NOTIFICATION_SEND);
        assert_eq!(module.manifest.capabilities[0].contract_version, 1);
        assert_eq!(
            module.manifest.capabilities[0].completion,
            CompletionMode::Yield
        );
        assert_eq!(
            module.manifest.capabilities[0].schema,
            "tps.notification.send.v1"
        );
    }

    #[test]
    fn accepts_only_guest_owned_fields() {
        assert!(validate_guest_request(&valid_request()).is_ok());
        for field in [
            "environmentId",
            "address",
            "providerToken",
            "acceptedAtMs",
            "nextAttemptAtMs",
            "attemptId",
            "notificationId",
            "status",
        ] {
            let mut request: Value = serde_json::from_str(&valid_request()).unwrap();
            request[field] = Value::String("forged".into());
            assert_eq!(
                validate_guest_request(&request.to_string()),
                Err("tps-notify request contains missing or unknown fields")
            );
        }
    }

    #[test]
    fn rejects_invalid_bounds_destination_and_digest() {
        let mut request: Value = serde_json::from_str(&valid_request()).unwrap();
        request["destination"]["generation"] = Value::from(0);
        assert!(validate_guest_request(&request.to_string()).is_err());
        request = serde_json::from_str(&valid_request()).unwrap();
        request["destination"]["credential"] = Value::String("forged".into());
        assert!(validate_guest_request(&request.to_string()).is_err());
        request = serde_json::from_str(&valid_request()).unwrap();
        request["evidenceReceiptSha256"] = Value::String("A".repeat(64));
        assert!(validate_guest_request(&request.to_string()).is_err());
        request = serde_json::from_str(&valid_request()).unwrap();
        request["body"] = Value::String("x".repeat(MAX_BODY_BYTES + 1));
        assert!(validate_guest_request(&request.to_string()).is_err());
    }
}
