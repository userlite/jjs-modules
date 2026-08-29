//! Strict guest surface for host-recorded cryptographic operations.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};

const RANDOM_BYTES: ModuleFunctionKey = ModuleFunctionKey(1);
const SHA256: ModuleFunctionKey = ModuleFunctionKey(2);
const HMAC_SHA256: ModuleFunctionKey = ModuleFunctionKey(3);
const RETURN_COMPLETION: u32 = 1;

pub const CRYPTO_RANDOM_BYTES: &str = "jjs:crypto/randomBytes";
pub const CRYPTO_SHA256: &str = "jjs:crypto/sha256";
pub const CRYPTO_HMAC_SHA256: &str = "jjs:crypto/hmacSha256";

pub fn capability_ids() -> [&'static str; 3] {
    [CRYPTO_RANDOM_BYTES, CRYPTO_SHA256, CRYPTO_HMAC_SHA256]
}

pub struct CryptoModule {
    manifest: ModuleManifest,
}

impl Default for CryptoModule {
    fn default() -> Self {
        let capabilities = [
            (CRYPTO_RANDOM_BYTES, "jjs.crypto.random-bytes.v1"),
            (CRYPTO_SHA256, "jjs.crypto.sha256.v1"),
            (CRYPTO_HMAC_SHA256, "jjs.crypto.hmac-sha256.v1"),
        ]
        .into_iter()
        .map(|(id, schema)| HostCapabilityDescriptor {
            id: id.into(),
            contract_version: 1,
            completion: CompletionMode::Yield,
            schema: schema.into(),
        })
        .collect();
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.crypto".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-crypto-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["jjs-crypto".into()],
                capabilities,
                dependencies: vec![],
                function_keys: vec![RANDOM_BYTES.0, SHA256.0, HMAC_SHA256.0],
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

impl NativeModule for CryptoModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        for (name, key) in [
            ("randomBytes", RANDOM_BYTES),
            ("sha256", SHA256),
            ("hmacSha256", HMAC_SHA256),
        ] {
            let function = context.function(key)?;
            context.set_property(exports, name, function)?;
        }
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
        let operation = match key {
            RANDOM_BYTES => CRYPTO_RANDOM_BYTES,
            SHA256 => CRYPTO_SHA256,
            HMAC_SHA256 => CRYPTO_HMAC_SHA256,
            _ => {
                return Err(ModuleError::ContractViolation(
                    "unknown jjs-crypto function key".into(),
                ));
            }
        };
        if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Object {
            return Ok(thrown(format!(
                "jjs-crypto {operation} requires exactly one options object"
            )));
        }
        let encoded = match context.json_stringify(args[0]) {
            Ok(encoded) => encoded,
            Err(_) => {
                return Ok(thrown(
                    "jjs-crypto options must be JSON-compatible and acyclic",
                ));
            }
        };
        context.request_host(
            HostRequestSpec {
                capability: operation.into(),
                operation: operation.into(),
                arguments: vec![encoded],
            },
            ModuleContinuation(RETURN_COMPLETION),
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
        if continuation.0 != RETURN_COMPLETION {
            return Err(ModuleError::ContractViolation(
                "unknown jjs-crypto continuation".into(),
            ));
        }
        match completion {
            Ok(encoded) => context
                .json_parse(encoded)
                .map(ModuleCallResult::Return)
                .map_err(|_| {
                    ModuleError::ContractViolation(
                        "jjs-crypto host completion was not valid JSON".into(),
                    )
                }),
            Err(message) => Ok(ModuleCallResult::Throw {
                name: "JjsCryptoError".into(),
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
            "jjs-crypto has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_exact_yielding_crypto_contracts() {
        let module = CryptoModule::default();
        assert_eq!(module.manifest.imports, ["jjs-crypto"]);
        assert_eq!(module.manifest.function_keys, [1, 2, 3]);
        assert_eq!(module.manifest.capabilities.len(), 3);
        for (index, id) in capability_ids().into_iter().enumerate() {
            assert_eq!(module.manifest.capabilities[index].id, id);
            assert_eq!(module.manifest.capabilities[index].contract_version, 1);
            assert_eq!(
                module.manifest.capabilities[index].completion,
                CompletionMode::Yield
            );
            assert!(module.manifest.capabilities[index].schema.ends_with(".v1"));
        }
    }
}
