//! Small, dependency-free exact-schema validation for JJS applications.

use jjs_module_api::{
    ModuleCallResult, ModuleContext, ModuleError, ModuleFunctionKey, ModuleIdentity,
    ModuleManifest, ModuleValueKind, NativeModule, ValueHandle, MODULE_API_VERSION,
};

const EXACT_OBJECT: ModuleFunctionKey = ModuleFunctionKey(1);
const STRING: ModuleFunctionKey = ModuleFunctionKey(2);
const SAFE_INTEGER: ModuleFunctionKey = ModuleFunctionKey(3);
const BOOLEAN: ModuleFunctionKey = ModuleFunctionKey(4);
const ARRAY: ModuleFunctionKey = ModuleFunctionKey(5);
const OBJECT: ModuleFunctionKey = ModuleFunctionKey(6);

pub struct JsonSchemaModule {
    manifest: ModuleManifest,
}

impl Default for JsonSchemaModule {
    fn default() -> Self {
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.json-schema".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    implementation: "jjs-module-json-schema-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["jjs-schema".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: (1..=6).collect(),
                object_kind_keys: vec![],
                deterministic_resources: vec![],
            },
        }
    }
}

fn thrown(name: &str, message: impl Into<String>) -> ModuleCallResult {
    ModuleCallResult::Throw {
        name: name.into(),
        message: message.into(),
    }
}

fn attach(
    context: &mut dyn ModuleContext,
    exports: ValueHandle,
    name: &str,
    key: ModuleFunctionKey,
) -> Result<(), ModuleError> {
    let function = context.function(key)?;
    context.set_property(exports, name, function)
}

fn exact_names(
    context: &dyn ModuleContext,
    value: ValueHandle,
    allowed: &[&str],
    label: &str,
) -> Result<Vec<String>, ModuleCallResult> {
    if context.value_kind(value).ok() != Some(ModuleValueKind::Object) {
        return Err(thrown("TypeError", format!("{label} must be an object")));
    }
    let mut names = context
        .own_property_names(value)
        .map_err(|_| thrown("TypeError", format!("{label} must be an object")))?;
    names.sort();
    if names.iter().any(|name| !allowed.contains(&name.as_str())) {
        return Err(thrown(
            "TypeError",
            format!("{label} contains an unknown keyword"),
        ));
    }
    Ok(names)
}

fn optional_safe_integer(
    context: &mut dyn ModuleContext,
    options: ValueHandle,
    names: &[String],
    property: &str,
    label: &str,
) -> Result<Option<f64>, ModuleCallResult> {
    if !names.iter().any(|name| name == property) {
        return Ok(None);
    }
    let value = context
        .get_property(options, property)
        .map_err(|_| thrown("TypeError", format!("{label}.{property} is unreadable")))?;
    let number = context.as_number(value).map_err(|_| {
        thrown(
            "TypeError",
            format!("{label}.{property} must be a safe integer"),
        )
    })?;
    if !number.is_finite()
        || number.fract() != 0.0
        || !(-9_007_199_254_740_991.0..=9_007_199_254_740_991.0).contains(&number)
    {
        return Err(thrown(
            "TypeError",
            format!("{label}.{property} must be a safe integer"),
        ));
    }
    Ok(Some(number))
}

fn descriptor(context: &mut dyn ModuleContext, kind: &str) -> Result<ValueHandle, ModuleError> {
    let schema = context.object()?;
    let kind = context.string(kind)?;
    context.set_property(schema, "_jjsSchemaKind", kind)?;
    Ok(schema)
}

fn bounded_descriptor(
    context: &mut dyn ModuleContext,
    kind: &str,
    options: ValueHandle,
    low: &str,
    high: &str,
    label: &str,
    nonnegative: bool,
) -> Result<ModuleCallResult, ModuleError> {
    let names = match exact_names(context, options, &[low, high], label) {
        Ok(names) => names,
        Err(error) => return Ok(error),
    };
    let minimum = match optional_safe_integer(context, options, &names, low, label) {
        Ok(value) => value,
        Err(error) => return Ok(error),
    };
    let maximum = match optional_safe_integer(context, options, &names, high, label) {
        Ok(value) => value,
        Err(error) => return Ok(error),
    };
    if (nonnegative && minimum.is_some_and(|value| value < 0.0))
        || (nonnegative && maximum.is_some_and(|value| value < 0.0))
        || matches!((minimum, maximum), (Some(min), Some(max)) if min > max)
    {
        return Ok(thrown("TypeError", format!("{label} bounds are invalid")));
    }
    let schema = descriptor(context, kind)?;
    if let Some(value) = minimum {
        let value = context.number(value)?;
        context.set_property(schema, low, value)?;
    }
    if let Some(value) = maximum {
        let value = context.number(value)?;
        context.set_property(schema, high, value)?;
    }
    Ok(ModuleCallResult::Return(schema))
}

fn optional_number_property(
    context: &mut dyn ModuleContext,
    object: ValueHandle,
    name: &str,
) -> Result<Option<f64>, String> {
    let names = context
        .own_property_names(object)
        .map_err(|_| "schema descriptor is invalid".to_owned())?;
    if !names.iter().any(|candidate| candidate == name) {
        return Ok(None);
    }
    let value = context
        .get_property(object, name)
        .map_err(|_| format!("schema descriptor {name} is unreadable"))?;
    context
        .as_number(value)
        .map(Some)
        .map_err(|_| format!("schema descriptor {name} is invalid"))
}

fn validate(
    context: &mut dyn ModuleContext,
    value: ValueHandle,
    schema: ValueHandle,
    path: &str,
) -> Result<(), String> {
    context
        .charge_fuel(1)
        .map_err(|error| format!("schema validation budget failed: {error}"))?;
    let kind = context
        .get_property(schema, "_jjsSchemaKind")
        .and_then(|value| context.as_string(value))
        .map_err(|_| format!("{path} schema descriptor is invalid"))?;
    match kind.as_str() {
        "string" => {
            let text = context
                .as_string(value)
                .map_err(|_| format!("{path} must be a string"))?;
            let length = text.chars().count() as f64;
            if optional_number_property(context, schema, "minLength")?
                .is_some_and(|minimum| length < minimum)
            {
                return Err(format!("{path} is shorter than minLength"));
            }
            if optional_number_property(context, schema, "maxLength")?
                .is_some_and(|maximum| length > maximum)
            {
                return Err(format!("{path} is longer than maxLength"));
            }
        }
        "safeInteger" => {
            let number = context
                .as_number(value)
                .map_err(|_| format!("{path} must be a safe integer"))?;
            if !number.is_finite()
                || number.fract() != 0.0
                || !(-9_007_199_254_740_991.0..=9_007_199_254_740_991.0).contains(&number)
            {
                return Err(format!("{path} must be a safe integer"));
            }
            if optional_number_property(context, schema, "minimum")?
                .is_some_and(|minimum| number < minimum)
            {
                return Err(format!("{path} is below minimum"));
            }
            if optional_number_property(context, schema, "maximum")?
                .is_some_and(|maximum| number > maximum)
            {
                return Err(format!("{path} is above maximum"));
            }
        }
        "boolean" => {
            if context
                .value_kind(value)
                .map_err(|error| error.to_string())?
                != ModuleValueKind::Bool
            {
                return Err(format!("{path} must be a boolean"));
            }
        }
        "array" => {
            if context
                .value_kind(value)
                .map_err(|error| error.to_string())?
                != ModuleValueKind::Array
            {
                return Err(format!("{path} must be an array"));
            }
            let length = context
                .array_len(value)
                .map_err(|error| error.to_string())?;
            if optional_number_property(context, schema, "minLength")?
                .is_some_and(|minimum| (length as f64) < minimum)
            {
                return Err(format!("{path} is shorter than minLength"));
            }
            if optional_number_property(context, schema, "maxLength")?
                .is_some_and(|maximum| (length as f64) > maximum)
            {
                return Err(format!("{path} is longer than maxLength"));
            }
            let item_schema = context
                .get_property(schema, "items")
                .map_err(|_| format!("{path} array item schema is missing"))?;
            for index in 0..length {
                let item = context
                    .array_get(value, index)
                    .map_err(|error| error.to_string())?;
                validate(context, item, item_schema, &format!("{path}[{index}]"))?;
            }
        }
        "object" => {
            let shape = context
                .get_property(schema, "shape")
                .map_err(|_| format!("{path} object shape is missing"))?;
            validate_exact_object(context, value, shape, path)?;
        }
        _ => return Err(format!("{path} schema kind is unknown")),
    }
    Ok(())
}

fn validate_exact_object(
    context: &mut dyn ModuleContext,
    value: ValueHandle,
    shape: ValueHandle,
    path: &str,
) -> Result<(), String> {
    if context
        .value_kind(value)
        .map_err(|error| error.to_string())?
        != ModuleValueKind::Object
    {
        return Err(format!("{path} must be an object"));
    }
    let mut expected = context
        .own_property_names(shape)
        .map_err(|_| "exact object shape must be an object".to_owned())?;
    let mut actual = context
        .own_property_names(value)
        .map_err(|_| format!("{path} must be an object"))?;
    expected.sort();
    actual.sort();
    if expected != actual {
        return Err(format!("{path} fields must match the exact schema"));
    }
    for field in expected {
        let field_value = context
            .get_property(value, &field)
            .map_err(|_| format!("{path}.{field} is unreadable"))?;
        let field_schema = context
            .get_property(shape, &field)
            .map_err(|_| format!("{path}.{field} schema is unreadable"))?;
        validate(
            context,
            field_value,
            field_schema,
            &format!("{path}.{field}"),
        )?;
    }
    Ok(())
}

impl NativeModule for JsonSchemaModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn instantiate(
        &self,
        context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        let exports = context.object()?;
        for (name, key) in [
            ("exactObject", EXACT_OBJECT),
            ("string", STRING),
            ("safeInteger", SAFE_INTEGER),
            ("boolean", BOOLEAN),
            ("array", ARRAY),
            ("object", OBJECT),
        ] {
            attach(context, exports, name, key)?;
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
            EXACT_OBJECT => {
                if args.len() != 2 {
                    return Ok(thrown("TypeError", "exactObject requires value and shape"));
                }
                match validate_exact_object(context, args[0], args[1], "$body") {
                    Ok(()) => Ok(ModuleCallResult::Return(args[0])),
                    Err(message) => Ok(thrown("SchemaValidationError", message)),
                }
            }
            STRING => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "schema.string requires one options object",
                    ));
                }
                bounded_descriptor(
                    context,
                    "string",
                    args[0],
                    "minLength",
                    "maxLength",
                    "schema.string",
                    true,
                )
            }
            SAFE_INTEGER => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "schema.safeInteger requires one options object",
                    ));
                }
                bounded_descriptor(
                    context,
                    "safeInteger",
                    args[0],
                    "minimum",
                    "maximum",
                    "schema.safeInteger",
                    false,
                )
            }
            BOOLEAN => {
                if !args.is_empty() {
                    return Ok(thrown("TypeError", "schema.boolean accepts no arguments"));
                }
                Ok(ModuleCallResult::Return(descriptor(context, "boolean")?))
            }
            ARRAY => {
                if args.len() != 2 {
                    return Ok(thrown(
                        "TypeError",
                        "schema.array requires item schema and options",
                    ));
                }
                let result = bounded_descriptor(
                    context,
                    "array",
                    args[1],
                    "minLength",
                    "maxLength",
                    "schema.array",
                    true,
                )?;
                let ModuleCallResult::Return(schema) = result else {
                    return Ok(result);
                };
                context.set_property(schema, "items", args[0])?;
                Ok(ModuleCallResult::Return(schema))
            }
            OBJECT => {
                if args.len() != 1 {
                    return Ok(thrown(
                        "TypeError",
                        "schema.object requires one exact shape",
                    ));
                }
                if context.value_kind(args[0])? != ModuleValueKind::Object {
                    return Ok(thrown("TypeError", "schema.object shape must be an object"));
                }
                let schema = descriptor(context, "object")?;
                context.set_property(schema, "shape", args[0])?;
                Ok(ModuleCallResult::Return(schema))
            }
            _ => Err(ModuleError::ContractViolation(format!(
                "unknown jjs-schema function key {}",
                key.0
            ))),
        }
    }

    fn resume(
        &self,
        _continuation: jjs_module_api::ModuleContinuation,
        _state: &[ValueHandle],
        _completion: Result<ValueHandle, String>,
        _context: &mut dyn ModuleContext,
    ) -> Result<ModuleCallResult, ModuleError> {
        Err(ModuleError::ContractViolation(
            "jjs-schema has no host continuations".into(),
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
            "jjs-schema has no host events".into(),
        ))
    }
}
