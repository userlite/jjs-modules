//! Deterministic, bounded text helpers for JJS applications.

use std::collections::BTreeSet;

use jjs_module_api::{
    ModuleCallResult, ModuleContext, ModuleContinuation, ModuleError, ModuleFunctionKey,
    ModuleIdentity, ModuleManifest, NativeModule, ValueHandle, MODULE_API_VERSION,
};

const NORMALIZE: ModuleFunctionKey = ModuleFunctionKey(1);
const WORDS: ModuleFunctionKey = ModuleFunctionKey(2);
const SHARED_WORDS: ModuleFunctionKey = ModuleFunctionKey(3);
const UTF8_ENCODE: ModuleFunctionKey = ModuleFunctionKey(4);
const UTF8_DECODE: ModuleFunctionKey = ModuleFunctionKey(5);

pub struct TextModule {
    manifest: ModuleManifest,
}

impl Default for TextModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.text".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-text-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["tps-text".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: vec![
                    NORMALIZE.0,
                    WORDS.0,
                    SHARED_WORDS.0,
                    UTF8_ENCODE.0,
                    UTF8_DECODE.0,
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

fn normalized_words(
    context: &mut dyn ModuleContext,
    text: &str,
) -> Result<Vec<String>, ModuleError> {
    context.charge_fuel(text.len().saturating_add(1) as u64)?;
    let mut words = Vec::new();
    let mut word = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() {
            word.extend(character.to_lowercase());
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    Ok(words)
}

impl NativeModule for TextModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        for (name, key) in [
            ("normalize", NORMALIZE),
            ("words", WORDS),
            ("sharedWords", SHARED_WORDS),
            ("utf8Encode", UTF8_ENCODE),
            ("utf8Decode", UTF8_DECODE),
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
        match key {
            NORMALIZE | WORDS => {
                if args.len() != 1 {
                    return Ok(thrown("tps-text helper requires exactly one string"));
                }
                let text = match context.as_string(args[0]) {
                    Ok(text) => text,
                    Err(_) => return Ok(thrown("tps-text helper requires a string")),
                };
                let words = normalized_words(context, &text)?;
                if key == NORMALIZE {
                    return Ok(ModuleCallResult::Return(context.string(&words.join(" "))?));
                }
                let result = context.array()?;
                for word in words {
                    let word = context.string(&word)?;
                    context.array_push(result, word)?;
                }
                Ok(ModuleCallResult::Return(result))
            }
            SHARED_WORDS => {
                if args.len() != 2 {
                    return Ok(thrown("tps-text sharedWords requires exactly two strings"));
                }
                let left = match context.as_string(args[0]) {
                    Ok(value) => value,
                    Err(_) => return Ok(thrown("tps-text sharedWords requires strings")),
                };
                let right = match context.as_string(args[1]) {
                    Ok(value) => value,
                    Err(_) => return Ok(thrown("tps-text sharedWords requires strings")),
                };
                let left = normalized_words(context, &left)?
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                let right = normalized_words(context, &right)?
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                let result = context.array()?;
                for word in left.intersection(&right) {
                    let word = context.string(word)?;
                    context.array_push(result, word)?;
                }
                Ok(ModuleCallResult::Return(result))
            }
            UTF8_ENCODE => {
                if args.len() != 1 {
                    return Ok(thrown("tps-text utf8Encode requires exactly one string"));
                }
                let text = match context.as_string(args[0]) {
                    Ok(value) => value,
                    Err(_) => return Ok(thrown("tps-text utf8Encode requires a string")),
                };
                context.charge_fuel(text.len().saturating_add(1) as u64)?;
                let result = context.array()?;
                for byte in text.as_bytes() {
                    let byte = context.number(f64::from(*byte))?;
                    context.array_push(result, byte)?;
                }
                Ok(ModuleCallResult::Return(result))
            }
            UTF8_DECODE => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "tps-text utf8Decode requires exactly one byte array",
                    ));
                }
                let length = match context.array_len(args[0]) {
                    Ok(value) => value,
                    Err(_) => return Ok(thrown("tps-text utf8Decode requires a byte array")),
                };
                context.charge_fuel(length.saturating_add(1) as u64)?;
                let mut bytes = Vec::with_capacity(length);
                for index in 0..length {
                    let value = context.array_get(args[0], index)?;
                    let number = match context.as_number(value) {
                        Ok(value)
                            if value.is_finite()
                                && value.fract() == 0.0
                                && (0.0..=255.0).contains(&value) =>
                        {
                            value
                        }
                        _ => {
                            return Ok(thrown(
                                "tps-text utf8Decode accepts only integer bytes 0..255",
                            ));
                        }
                    };
                    bytes.push(number as u8);
                }
                let decoded = match String::from_utf8(bytes) {
                    Ok(value) => value,
                    Err(_) => return Ok(thrown("tps-text utf8Decode requires valid UTF-8")),
                };
                Ok(ModuleCallResult::Return(context.string(&decoded)?))
            }
            _ => Err(ModuleError::ContractViolation(
                "unknown tps-text function key".into(),
            )),
        }
    }

    fn resume(
        &self,
        _continuation: ModuleContinuation,
        _state: &[ValueHandle],
        _completion: Result<ValueHandle, String>,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "tps-text never yields".into(),
        ))
    }

    fn event(
        &self,
        _event: u32,
        _target: ValueHandle,
        _payload: ValueHandle,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "tps-text has no events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_pure_deterministic_contract() {
        let module = TextModule::default();
        assert_eq!(module.manifest.imports, ["tps-text"]);
        assert!(module.manifest.capabilities.is_empty());
        assert_eq!(module.manifest.function_keys, [1, 2, 3, 4, 5]);
    }
}
