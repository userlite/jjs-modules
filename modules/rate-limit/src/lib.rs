//! Strict host-owned rate limiting for public JJS applications.

use jjs_module_api::{
    CompletionMode, HostCapabilityDescriptor, HostRequestSpec, ModuleCallResult, ModuleContext,
    ModuleContinuation, ModuleError, ModuleFunctionKey, ModuleIdentity, ModuleManifest,
    ModuleObjectKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};

const OPEN: ModuleFunctionKey = ModuleFunctionKey(1);
const CONSUME: ModuleFunctionKey = ModuleFunctionKey(2);
const MIDDLEWARE: ModuleFunctionKey = ModuleFunctionKey(3);
const MIDDLEWARE_CALL: ModuleFunctionKey = ModuleFunctionKey(4);
const LIMITER: ModuleObjectKind = ModuleObjectKind(1);
const LIMITER_NAME: u32 = 1;
const MIDDLEWARE_NAME: u32 = 2;
const MIDDLEWARE_WEIGHT: u32 = 3;
const MIDDLEWARE_STATUS: u32 = 4;
const MIDDLEWARE_MESSAGE: u32 = 5;
const MIDDLEWARE_HANDLER: u32 = 6;
const RETURN_LIMITER: u32 = 1;
const RETURN_JSON: u32 = 2;
const RETURN_MIDDLEWARE: u32 = 3;

pub const RATE_LIMIT_OPEN: &str = "jjs:rate-limit/open";
pub const RATE_LIMIT_CONSUME: &str = "jjs:rate-limit/consume";

pub fn capability_ids() -> [&'static str; 2] {
    [RATE_LIMIT_OPEN, RATE_LIMIT_CONSUME]
}

pub struct RateLimitModule {
    manifest: ModuleManifest,
}

impl Default for RateLimitModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.tps-rate-limit".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-rate-limit-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 2,
                imports: vec!["tps-rate-limit".into()],
                capabilities: capability_ids()
                    .into_iter()
                    .map(|id| HostCapabilityDescriptor {
                        id: id.into(),
                        contract_version: 1,
                        completion: CompletionMode::Yield,
                        schema: "jjs.rate-limit.v1".into(),
                    })
                    .collect(),
                dependencies: vec![],
                function_keys: vec![OPEN.0, CONSUME.0, MIDDLEWARE.0, MIDDLEWARE_CALL.0],
                object_kind_keys: vec![LIMITER.0],
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

fn positive_safe_integer(
    context: &dyn ModuleContext,
    value: ValueHandle,
    name: &str,
) -> Result<(), ModuleCallResult> {
    match context.as_number(value) {
        Ok(number)
            if number.is_finite()
                && number.fract() == 0.0
                && (1.0..=9_007_199_254_740_991.0).contains(&number) =>
        {
            Ok(())
        }
        _ => Err(thrown(format!(
            "Rate limiter {name} must be a positive safe integer"
        ))),
    }
}

fn request(
    context: &mut dyn ModuleContext,
    capability: &str,
    arguments: Vec<ValueHandle>,
    continuation: u32,
    state: Vec<ValueHandle>,
) -> Result<ModuleCallResult, ModuleError> {
    context.request_host(
        HostRequestSpec {
            capability: capability.into(),
            operation: capability.into(),
            arguments,
        },
        ModuleContinuation(continuation),
        state,
        true,
    )
}

impl NativeModule for RateLimitModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        let rate_limiter = context.object()?;
        let open = context.function(OPEN)?;
        context.set_property(rate_limiter, "open", open)?;
        context.set_property(exports, "RateLimiter", rate_limiter)?;
        Ok(ModuleCallResult::Return(exports))
    }

    fn call(
        &self,
        key: ModuleFunctionKey,
        _callee: ValueHandle,
        receiver: ValueHandle,
        args: &[ValueHandle],
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        match key {
            OPEN => {
                if args.len() != 2 {
                    return Ok(thrown(
                        "RateLimiter.open requires a name and exact configuration",
                    ));
                }
                match context.as_string(args[0]) {
                    Ok(name) if !name.is_empty() => {}
                    _ => return Ok(thrown("Rate limiter name must be a non-empty string")),
                }
                let limit = context.get_property(args[1], "limit")?;
                let window_ms = context.get_property(args[1], "windowMs")?;
                if let Err(error) = positive_safe_integer(context, limit, "limit") {
                    return Ok(error);
                }
                if let Err(error) = positive_safe_integer(context, window_ms, "windowMs") {
                    return Ok(error);
                }
                let limiter = context.module_object(LIMITER)?;
                context.set_private(limiter, LIMITER_NAME, args[0])?;
                let consume = context.function(CONSUME)?;
                context.set_property(limiter, "consume", consume)?;
                let middleware = context.function(MIDDLEWARE)?;
                context.set_property(limiter, "middleware", middleware)?;
                request(
                    context,
                    RATE_LIMIT_OPEN,
                    vec![args[0], limit, window_ms],
                    RETURN_LIMITER,
                    vec![limiter],
                )
            }
            CONSUME => {
                if args.len() != 2 {
                    return Ok(thrown(
                        "Rate limiter consume requires a client identity and weight",
                    ));
                }
                match context.as_string(args[0]) {
                    Ok(identity) if !identity.is_empty() => {}
                    _ => {
                        return Ok(thrown(
                            "Rate limiter client identity must be a non-empty string",
                        ));
                    }
                }
                if let Err(error) = positive_safe_integer(context, args[1], "weight") {
                    return Ok(error);
                }
                let name = context.get_private(receiver, LIMITER_NAME)?;
                request(
                    context,
                    RATE_LIMIT_CONSUME,
                    vec![name, args[0], args[1]],
                    RETURN_JSON,
                    vec![],
                )
            }
            MIDDLEWARE => {
                if args.len() != 2 || !context.is_callable(args[1]) {
                    return Ok(thrown(
                        "Rate limiter middleware requires one exact options object and one handler",
                    ));
                }
                let mut names = match context.own_property_names(args[0]) {
                    Ok(names) => names,
                    Err(_) => {
                        return Ok(thrown("Rate limiter middleware options must be an object"))
                    }
                };
                names.sort();
                if names != ["message", "status", "weight"] {
                    return Ok(thrown(
                        "Rate limiter middleware options require exactly message, status, and weight",
                    ));
                }
                let weight = context.get_property(args[0], "weight")?;
                if let Err(error) = positive_safe_integer(context, weight, "weight") {
                    return Ok(error);
                }
                let status = context.get_property(args[0], "status")?;
                let status_number = context.as_number(status).unwrap_or(f64::NAN);
                if !status_number.is_finite()
                    || status_number.fract() != 0.0
                    || !(400.0..=599.0).contains(&status_number)
                {
                    return Ok(thrown(
                        "Rate limiter middleware status must be an HTTP error status",
                    ));
                }
                let message = context.get_property(args[0], "message")?;
                if context.as_string(message).is_err() {
                    return Ok(thrown("Rate limiter middleware message must be a string"));
                }
                let middleware = context.function(MIDDLEWARE_CALL)?;
                let name = context.get_private(receiver, LIMITER_NAME)?;
                context.set_private(middleware, MIDDLEWARE_NAME, name)?;
                context.set_private(middleware, MIDDLEWARE_WEIGHT, weight)?;
                context.set_private(middleware, MIDDLEWARE_STATUS, status)?;
                context.set_private(middleware, MIDDLEWARE_MESSAGE, message)?;
                context.set_private(middleware, MIDDLEWARE_HANDLER, args[1])?;
                Ok(ModuleCallResult::Return(middleware))
            }
            MIDDLEWARE_CALL => {
                if args.len() != 3 || !context.is_callable(args[2]) {
                    return Ok(thrown(
                        "Rate limiter middleware requires req, res, and next",
                    ));
                }
                let client = context.get_property(args[0], "client")?;
                let identity = context.get_property(client, "id")?;
                match context.as_string(identity) {
                    Ok(identity) if !identity.is_empty() => {}
                    _ => return Ok(thrown("Rate limiter middleware requires req.client.id")),
                }
                let name = context.get_private(_callee, MIDDLEWARE_NAME)?;
                let weight = context.get_private(_callee, MIDDLEWARE_WEIGHT)?;
                let status = context.get_private(_callee, MIDDLEWARE_STATUS)?;
                let message = context.get_private(_callee, MIDDLEWARE_MESSAGE)?;
                let handler = context.get_private(_callee, MIDDLEWARE_HANDLER)?;
                request(
                    context,
                    RATE_LIMIT_CONSUME,
                    vec![name, identity, weight],
                    RETURN_MIDDLEWARE,
                    vec![args[0], args[1], status, message, handler],
                )
            }
            _ => Err(ModuleError::ContractViolation(
                "unknown rate limiter function key".into(),
            )),
        }
    }

    fn resume(
        &self,
        continuation: ModuleContinuation,
        state: &[ValueHandle],
        completion: Result<ValueHandle, String>,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let value = match completion {
            Ok(value) => value,
            Err(message) => {
                return Ok(ModuleCallResult::Throw {
                    name: "Error".into(),
                    message,
                });
            }
        };
        match continuation.0 {
            RETURN_LIMITER => Ok(ModuleCallResult::Return(
                state.first().copied().ok_or_else(|| {
                    ModuleError::ContractViolation(
                        "rate limiter continuation state is missing".into(),
                    )
                })?,
            )),
            RETURN_JSON => Ok(ModuleCallResult::Return(context.json_parse(value)?)),
            RETURN_MIDDLEWARE => {
                if state.len() != 5 {
                    return Err(ModuleError::ContractViolation(
                        "rate limiter middleware continuation state is invalid".into(),
                    ));
                }
                let decision = context.json_parse(value)?;
                let allowed = context.get_property(decision, "allowed")?;
                if context.as_bool(allowed).unwrap_or(false) {
                    let undefined = context.undefined();
                    let result = context.call(state[4], undefined, &[state[0], state[1]])?;
                    return Ok(ModuleCallResult::Return(result));
                }
                let status_method = context.get_property(state[1], "status")?;
                context.call(status_method, state[1], &[state[2]])?;
                let retry = context.get_property(decision, "retryAfterMs")?;
                let retry_text = context.to_string(retry)?;
                let retry_text = context.string(&retry_text)?;
                let set = context.get_property(state[1], "set")?;
                let header = context.string("Retry-After-Ms")?;
                context.call(set, state[1], &[header, retry_text])?;
                let body = context.object()?;
                context.set_property(body, "error", state[3])?;
                context.set_property(body, "retryAfterMs", retry)?;
                let json = context.get_property(state[1], "json")?;
                context.call(json, state[1], &[body])?;
                Ok(ModuleCallResult::Return(context.undefined()))
            }
            _ => Err(ModuleError::ContractViolation(
                "unknown rate limiter continuation".into(),
            )),
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
            "rate limiter has no guest events".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_strict_host_owned_contract() {
        let module = RateLimitModule::default();
        assert_eq!(module.manifest.imports, ["tps-rate-limit"]);
        assert_eq!(
            module.manifest.function_keys,
            [OPEN.0, CONSUME.0, MIDDLEWARE.0, MIDDLEWARE_CALL.0]
        );
        assert_eq!(module.manifest.object_kind_keys, [LIMITER.0]);
        assert_eq!(capability_ids(), [RATE_LIMIT_OPEN, RATE_LIMIT_CONSUME]);
        assert!(module
            .manifest
            .capabilities
            .iter()
            .all(|capability| capability.completion == CompletionMode::Yield));
    }
}
