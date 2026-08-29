//! Strict guest surface for opaque, environment-owned secrets.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};

const GENERATE: ModuleFunctionKey = ModuleFunctionKey(1);
const IMPORT: ModuleFunctionKey = ModuleFunctionKey(2);
const ROTATE: ModuleFunctionKey = ModuleFunctionKey(3);
const DELETE: ModuleFunctionKey = ModuleFunctionKey(4);
const VERIFY: ModuleFunctionKey = ModuleFunctionKey(5);
const RETURN_COMPLETION: u32 = 1;

pub const SECRETS_GENERATE: &str = "jjs:secrets/generate";
pub const SECRETS_IMPORT: &str = "jjs:secrets/import";
pub const SECRETS_ROTATE: &str = "jjs:secrets/rotate";
pub const SECRETS_DELETE: &str = "jjs:secrets/delete";
pub const SECRETS_VERIFY: &str = "jjs:secrets/verify";

pub fn capability_ids() -> [&'static str; 5] {
    [
        SECRETS_GENERATE,
        SECRETS_IMPORT,
        SECRETS_ROTATE,
        SECRETS_DELETE,
        SECRETS_VERIFY,
    ]
}

pub struct TpsSecretsModule {
    manifest: ModuleManifest,
}

impl Default for TpsSecretsModule {
    fn default() -> Self {
        let capabilities = [
            (SECRETS_GENERATE, "jjs.secrets.generate.v1"),
            (SECRETS_IMPORT, "jjs.secrets.import.v1"),
            (SECRETS_ROTATE, "jjs.secrets.rotate.v1"),
            (SECRETS_DELETE, "jjs.secrets.delete.v1"),
            (SECRETS_VERIFY, "jjs.secrets.verify.v1"),
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
                    id: "org.jjs.tps-secrets".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-tps-secrets-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["tps-secrets".into()],
                capabilities,
                dependencies: vec![],
                function_keys: vec![GENERATE.0, IMPORT.0, ROTATE.0, DELETE.0, VERIFY.0],
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

impl NativeModule for TpsSecretsModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        for (name, key) in [
            ("generate", GENERATE),
            ("importSecret", IMPORT),
            ("rotate", ROTATE),
            ("deleteSecret", DELETE),
            ("verify", VERIFY),
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
            GENERATE => SECRETS_GENERATE,
            IMPORT => SECRETS_IMPORT,
            ROTATE => SECRETS_ROTATE,
            DELETE => SECRETS_DELETE,
            VERIFY => SECRETS_VERIFY,
            _ => {
                return Err(ModuleError::ContractViolation(
                    "unknown tps-secrets function key".into(),
                ));
            }
        };
        if args.len() != 1 || context.value_kind(args[0])? != ModuleValueKind::Object {
            return Ok(thrown(format!(
                "tps-secrets {operation} requires exactly one options object"
            )));
        }
        let encoded = match context.json_stringify(args[0]) {
            Ok(encoded) => encoded,
            Err(_) => {
                return Ok(thrown(
                    "tps-secrets options must be JSON-compatible and acyclic",
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
                "unknown tps-secrets continuation".into(),
            ));
        }
        match completion {
            Ok(encoded) => context
                .json_parse(encoded)
                .map(ModuleCallResult::Return)
                .map_err(|_| {
                    ModuleError::ContractViolation(
                        "tps-secrets host completion was not valid JSON".into(),
                    )
                }),
            Err(message) => Ok(ModuleCallResult::Throw {
                name: "TpsSecretsError".into(),
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
            "tps-secrets has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_exact_yielding_secret_contracts() {
        let module = TpsSecretsModule::default();
        assert_eq!(module.manifest.imports, ["tps-secrets"]);
        assert_eq!(module.manifest.function_keys, [1, 2, 3, 4, 5]);
        assert_eq!(module.manifest.capabilities.len(), 5);
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
