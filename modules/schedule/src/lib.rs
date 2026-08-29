//! Strict, deterministic scheduling helpers backed by JJS host timers.

use jjs_module_api::{
    MODULE_API_VERSION, ModuleCallResult, ModuleContext, ModuleContinuation, ModuleError,
    ModuleFunctionKey, ModuleIdentity, ModuleManifest, NativeModule, ValueHandle,
};

const AFTER: ModuleFunctionKey = ModuleFunctionKey(1);
const EVERY: ModuleFunctionKey = ModuleFunctionKey(2);
const CANCEL: ModuleFunctionKey = ModuleFunctionKey(3);
const MAX_DELAY_MS: u64 = 365 * 24 * 60 * 60 * 1_000;

pub struct ScheduleModule {
    manifest: ModuleManifest,
}

impl Default for ScheduleModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.tps-schedule".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-schedule-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["tps-schedule".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: vec![AFTER.0, EVERY.0, CANCEL.0],
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

fn parse_duration_text(text: &str) -> Option<u64> {
    let (digits, multiplier) = if let Some(value) = text.strip_suffix("ms") {
        (value, 1)
    } else if let Some(value) = text.strip_suffix('s') {
        (value, 1_000)
    } else if let Some(value) = text.strip_suffix('m') {
        (value, 60_000)
    } else if let Some(value) = text.strip_suffix('h') {
        (value, 3_600_000)
    } else if let Some(value) = text.strip_suffix('d') {
        (value, 86_400_000)
    } else {
        return None;
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits
        .parse::<u64>()
        .ok()?
        .checked_mul(multiplier)
        .filter(|value| (1..=MAX_DELAY_MS).contains(value))
}

fn parse_duration(
    context: &mut dyn ModuleContext,
    value: ValueHandle,
) -> Result<Option<u64>, ModuleError> {
    if let Ok(number) = context.as_number(value) {
        if number.is_finite()
            && number.fract() == 0.0
            && number >= 1.0
            && number <= MAX_DELAY_MS as f64
        {
            return Ok(Some(number as u64));
        }
        return Ok(None);
    }
    match context.as_string(value) {
        Ok(text) => Ok(parse_duration_text(&text)),
        Err(_) => Ok(None),
    }
}

impl NativeModule for ScheduleModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        for (name, key) in [("after", AFTER), ("every", EVERY), ("cancel", CANCEL)] {
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
            AFTER | EVERY => {
                if args.len() != 2 || !context.is_callable(args[1]) {
                    return Ok(thrown(
                        "schedule after/every requires a duration and callback",
                    ));
                }
                let Some(delay_ms) = parse_duration(context, args[0])? else {
                    return Ok(thrown(
                        "schedule duration must be a positive integer millisecond value or a strict ms/s/m/h/d string up to 365d",
                    ));
                };
                let timer = context.global(if key == AFTER {
                    "setTimeout"
                } else {
                    "setInterval"
                })?;
                let delay = context.number(delay_ms as f64)?;
                let undefined = context.undefined();
                let result = context.call(timer, undefined, &[args[1], delay])?;
                Ok(ModuleCallResult::Return(result))
            }
            CANCEL => {
                if args.len() != 1 {
                    return Ok(thrown("schedule cancel requires exactly one timer handle"));
                }
                let handle = match context.as_number(args[0]) {
                    Ok(value)
                        if value.is_finite()
                            && value.fract() == 0.0
                            && value >= 1.0
                            && value <= u32::MAX as f64 =>
                    {
                        value
                    }
                    _ => return Ok(thrown("schedule timer handle must be a positive integer")),
                };
                let clear = context.global("clearTimeout")?;
                let handle = context.number(handle)?;
                let undefined = context.undefined();
                let result = context.call(clear, undefined, &[handle])?;
                Ok(ModuleCallResult::Return(result))
            }
            _ => Err(ModuleError::ContractViolation(
                "unknown tps-schedule function key".into(),
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
            "tps-schedule never yields directly".into(),
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
            "tps-schedule has no events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_strict_timer_contract() {
        let module = ScheduleModule::default();
        assert_eq!(module.manifest.imports, ["tps-schedule"]);
        assert!(module.manifest.capabilities.is_empty());
        assert_eq!(module.manifest.function_keys, [1, 2, 3]);
    }

    #[test]
    fn parses_supported_duration_strings() {
        assert_eq!(parse_duration_text("1ms"), Some(1));
        assert_eq!(parse_duration_text("30s"), Some(30_000));
        assert_eq!(parse_duration_text("5m"), Some(300_000));
        assert_eq!(parse_duration_text("2h"), Some(7_200_000));
        assert_eq!(parse_duration_text("365d"), Some(MAX_DELAY_MS));
    }

    #[test]
    fn rejects_ambiguous_or_unbounded_durations() {
        for value in ["", "0s", "1.5s", " 1s", "1 s", "1w", "366d", "-1s"] {
            assert_eq!(parse_duration_text(value), None, "{value}");
        }
    }
}
