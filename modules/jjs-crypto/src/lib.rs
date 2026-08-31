//! Strict guest surface for host-recorded cryptographic operations.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};

const RANDOM_BYTES: ModuleFunctionKey = ModuleFunctionKey(1);
const SHA256: ModuleFunctionKey = ModuleFunctionKey(2);
const HMAC_SHA256: ModuleFunctionKey = ModuleFunctionKey(3);
const BYTES_TO_STRING: ModuleFunctionKey = ModuleFunctionKey(4);
const RETURN_COMPLETION: u32 = 1;
const RETURN_RANDOM_BYTES: u32 = 2;
const RANDOM_COUNTER: u32 = 1;

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
                state_version: 2,
                imports: vec!["crypto".into(), "node:crypto".into()],
                capabilities,
                dependencies: vec![],
                function_keys: vec![
                    RANDOM_BYTES.0,
                    SHA256.0,
                    HMAC_SHA256.0,
                    BYTES_TO_STRING.0,
                ],
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
            if key == RANDOM_BYTES {
                let counter = context.number(0.0)?;
                context.set_private(function, RANDOM_COUNTER, counter)?;
            }
            context.set_property(exports, name, function)?;
        }
        Ok(ModuleCallResult::Return(exports))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        if key == BYTES_TO_STRING {
            if args.len() > 1 {
                return Ok(thrown("Buffer.toString accepts at most one encoding"));
            }
            if let Some(encoding) = args.first().copied() {
                if context.value_kind(encoding)? != ModuleValueKind::String
                    || context.as_string(encoding)?.to_ascii_lowercase() != "hex"
                {
                    return Ok(thrown("only Buffer.toString('hex') is currently supported"));
                }
            }
            let mut encoded = String::with_capacity(context.array_len(receiver)? * 2);
            for index in 0..context.array_len(receiver)? {
                let value = context.array_get(receiver, index)?;
                let byte = context.as_number(value)?;
                if !byte.is_finite() || byte.fract() != 0.0 || !(0.0..=255.0).contains(&byte) {
                    return Err(ModuleError::ContractViolation(
                        "crypto byte result contained an invalid byte".into(),
                    ));
                }
                encoded.push_str(&format!("{:02x}", byte as u8));
            }
            return Ok(ModuleCallResult::Return(context.string(&encoded)?));
        }

        if key == RANDOM_BYTES {
            if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Number {
                return Ok(thrown("crypto.randomBytes requires one byte count"));
            }
            let length = context.as_number(args[0])?;
            if !length.is_finite() || length.fract() != 0.0 || !(1.0..=256.0).contains(&length) {
                return Ok(thrown("crypto.randomBytes byte count must be an integer from 1 to 256"));
            }
            let counter = context.get_private(callee, RANDOM_COUNTER)?;
            let counter = context.as_number(counter)? + 1.0;
            let next = context.number(counter)?;
            context.set_private(callee, RANDOM_COUNTER, next)?;
            let encoded = context.string(&format!(
                "{{\"operationId\":\"node-random-{counter:.0}\",\"length\":{length:.0}}}"
            ))?;
            return context.request_host(
                HostRequestSpec {
                    capability: CRYPTO_RANDOM_BYTES.into(),
                    operation: CRYPTO_RANDOM_BYTES.into(),
                    arguments: vec![encoded],
                },
                ModuleContinuation(RETURN_RANDOM_BYTES),
                vec![],
                false,
            );
        }

        let operation = match key {
            SHA256 => CRYPTO_SHA256,
            HMAC_SHA256 => CRYPTO_HMAC_SHA256,
            _ => {
                return Err(ModuleError::ContractViolation(
                    "unknown crypto function key".into(),
                ));
            }
        };
        if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Object {
            return Ok(thrown(format!(
                "crypto {operation} requires exactly one options object"
            )));
        }
        let encoded = match context.json_stringify(args[0]) {
            Ok(encoded) => encoded,
            Err(_) => {
                return Ok(thrown(
                    "crypto options must be JSON-compatible and acyclic",
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
        if !matches!(continuation.0, RETURN_COMPLETION | RETURN_RANDOM_BYTES) {
            return Err(ModuleError::ContractViolation(
                "unknown crypto continuation".into(),
            ));
        }
        match completion {
            Ok(encoded) => {
                let completion = context.json_parse(encoded).map_err(|_| {
                    ModuleError::ContractViolation(
                        "crypto host completion was not valid JSON".into(),
                    )
                })?;
                if continuation.0 == RETURN_RANDOM_BYTES {
                    let result = context.get_property(completion, "result")?;
                    let bytes = context.get_property(result, "bytes")?;
                    let to_string = context.function(BYTES_TO_STRING)?;
                    context.set_property(bytes, "toString", to_string)?;
                    Ok(ModuleCallResult::Return(bytes))
                } else {
                    Ok(ModuleCallResult::Return(completion))
                }
            }
            Err(message) => Ok(ModuleCallResult::Throw {
                name: "CryptoError".into(),
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
            "crypto has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_exact_yielding_crypto_contracts() {
        let module = CryptoModule::default();
        assert_eq!(module.manifest.imports, ["crypto", "node:crypto"]);
        assert_eq!(module.manifest.function_keys, [1, 2, 3, 4]);
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
