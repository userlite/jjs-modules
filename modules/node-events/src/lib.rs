//! Bounded, synchronous EventEmitter subset; all listener state belongs to the VM.
use jjs_module_api::{
    ModuleCallResult as Outcome, ModuleContext as Context, ModuleContinuation,
    ModuleError as Error, ModuleFunctionKey as Key, ModuleIdentity, ModuleManifest,
    ModuleObjectKind, ModuleValueKind as Kind, NativeModule, ValueHandle as Value,
    MODULE_API_VERSION,
};

pub const ON: Key = Key(100);
pub const ONCE: Key = Key(101);
pub const REMOVE: Key = Key(102);
pub const REMOVE_ALL: Key = Key(103);
pub const LISTENERS: Key = Key(104);
pub const COUNT: Key = Key(105);
pub const NAMES: Key = Key(106);
pub const EMIT: Key = Key(107);
pub const FUNCTION_KEYS: [u32; 8] = [100, 101, 102, 103, 104, 105, 106, 107];
const TABLE: u32 = 100;
const ALLOWED: u32 = 101;
pub const MAX_LISTENERS: usize = 1024;
pub const MAX_EVENT_BYTES: usize = 1024;

fn invalid(message: &str) -> Outcome {
    Outcome::Throw {
        name: "TypeError".into(),
        message: message.into(),
    }
}
fn corrupt(message: &str) -> Error {
    Error::ContractViolation(format!("events_state_invalid: {message}"))
}

/// Install this module's function keys on its own receiver; HTTP uses the same code.
pub fn install(c: &mut dyn Context, receiver: Value, allowed: &[&str]) -> Result<(), Error> {
    let table = c.array()?;
    c.set_private(receiver, TABLE, table)?;
    let events = c.array()?;
    for name in allowed {
        let value = c.string(name)?;
        c.array_push(events, value)?;
    }
    c.set_private(receiver, ALLOWED, events)?;
    for (names, key) in [
        (&["on", "addListener"][..], ON),
        (&["once"][..], ONCE),
        (&["removeListener", "off"][..], REMOVE),
        (&["removeAllListeners"][..], REMOVE_ALL),
        (&["listeners"][..], LISTENERS),
        (&["listenerCount"][..], COUNT),
        (&["eventNames"][..], NAMES),
        (&["emit"][..], EMIT),
    ] {
        let function = c.function(key)?;
        for name in names {
            c.set_property(receiver, name, function)?;
        }
    }
    Ok(())
}

fn table(c: &mut dyn Context, receiver: Value) -> Result<Value, Error> {
    let value = c.get_private(receiver, TABLE)?;
    if c.value_kind(value)? != Kind::Array {
        return Err(corrupt("listener table is not an array"));
    }
    if c.array_len(value)? > MAX_LISTENERS {
        return Err(corrupt("listener table exceeds its limit"));
    }
    Ok(value)
}

fn record(c: &mut dyn Context, entry: Value) -> Result<(String, Value, bool, bool), Error> {
    c.charge_fuel(1)?;
    let name = c.get_property(entry, "event")?;
    let name = c.as_string(name)?;
    let callback = c.get_property(entry, "listener")?;
    if !c.is_callable(callback) {
        return Err(corrupt("stored listener is not callable"));
    }
    let once = c.get_property(entry, "once")?;
    let fired = c.get_property(entry, "fired")?;
    Ok((name, callback, c.as_bool(once)?, c.as_bool(fired)?))
}

fn event_order(c: &mut dyn Context, entry: Value) -> Result<u64, Error> {
    let value = c.get_property(entry, "order")?;
    let value = c.as_number(value)?;
    if !value.is_finite() || value < 0.0 || value > 9007199254740990.0 || value.fract() != 0.0 {
        return Err(corrupt("invalid event order"));
    }
    Ok(value as u64)
}

fn allowed(c: &mut dyn Context, receiver: Value, event: &str) -> Result<bool, Error> {
    if event.len() > MAX_EVENT_BYTES {
        return Ok(false);
    }
    let events = c.get_private(receiver, ALLOWED)?;
    let len = c.array_len(events)?;
    if len == 0 {
        return Ok(true);
    }
    for i in 0..len {
        c.charge_fuel(1)?;
        let item = c.array_get(events, i)?;
        if c.as_string(item)? == event {
            return Ok(true);
        }
    }
    Ok(false)
}

fn remove_index(
    c: &mut dyn Context,
    receiver: Value,
    old: Value,
    index: usize,
) -> Result<(), Error> {
    let next = c.array()?;
    for i in 0..c.array_len(old)? {
        c.charge_fuel(1)?;
        if i != index {
            let item = c.array_get(old, i)?;
            c.array_push(next, item)?;
        }
    }
    c.set_private(receiver, TABLE, next)
}

/// Snapshot dispatch: mutations affect later emissions; once records share fired state.
/// Guest exceptions retain their original value. Resource errors are never caught.
pub fn emit(
    c: &mut dyn Context,
    receiver: Value,
    event: &str,
    args: &[Value],
) -> Result<Outcome, Error> {
    emit_capturing(c, receiver, event, args, None)
}

/// HTTP observes async listener failures without making listener calls asynchronous.
pub fn emit_capturing(
    c: &mut dyn Context,
    receiver: Value,
    event: &str,
    args: &[Value],
    rejection: Option<Value>,
) -> Result<Outcome, Error> {
    if !allowed(c, receiver, event)? {
        return Ok(invalid("events_event_unsupported_or_too_long"));
    }
    let snapshot = table(c, receiver)?;
    let mut found = false;
    for i in 0..c.array_len(snapshot)? {
        let entry = c.array_get(snapshot, i)?;
        let (name, callback, once, fired) = record(c, entry)?;
        if name != event {
            continue;
        }
        found = true;
        if once {
            if fired {
                continue;
            }
            let current = table(c, receiver)?;
            for j in 0..c.array_len(current)? {
                c.charge_fuel(1)?;
                let candidate = c.array_get(current, j)?;
                if c.strict_equals(candidate, entry)? {
                    remove_index(c, receiver, current, j)?;
                    break;
                }
            }
            let yes = c.bool(true)?;
            c.set_property(entry, "fired", yes)?;
        }
        match c.try_call(callback, receiver, args)? {
            Ok(value) => {
                if let Some(rejection) = rejection {
                    if c.value_kind(value)? == Kind::Object {
                        let catch = c.get_property(value, "catch")?;
                        if c.is_callable(catch) {
                            c.call(catch, value, &[rejection])?;
                        }
                    }
                }
            }
            Err(exception) => return Ok(Outcome::ThrowValue(exception)),
        }
    }
    if !found && event == "error" {
        if let Some(value) = args.first() {
            if c.value_kind(*value)? == Kind::Object {
                return Ok(Outcome::ThrowValue(*value));
            }
        }
        return Ok(Outcome::Throw {
            name: "Error".into(),
            message: "Unhandled error event".into(),
        });
    }
    Ok(Outcome::Return(c.bool(found)?))
}

pub fn call(
    c: &mut dyn Context,
    key: Key,
    receiver: Value,
    args: &[Value],
) -> Result<Outcome, Error> {
    let old = table(c, receiver)?;
    if key == NAMES {
        if !args.is_empty() {
            return Ok(invalid("events_eventNames_arity"));
        }
        let result = c.array()?;
        let mut seen = std::collections::BTreeMap::new();
        for i in 0..c.array_len(old)? {
            let item = c.array_get(old, i)?;
            let (name, _, _, _) = record(c, item)?;
            let order = event_order(c, item)?;
            let integer = name
                .parse::<u32>()
                .ok()
                .filter(|n| *n < u32::MAX && n.to_string() == name);
            let sort = match integer {
                Some(n) => (0, u64::from(n)),
                None => (1, order),
            };
            seen.insert(sort, name);
        }
        for name in seen.values() {
            let value = c.string(name)?;
            c.array_push(result, value)?;
        }
        return Ok(Outcome::Return(result));
    }
    if key == REMOVE_ALL && args.is_empty() {
        let empty = c.array()?;
        c.set_private(receiver, TABLE, empty)?;
        return Ok(Outcome::Return(receiver));
    }
    let Some(event) = args.first() else {
        return Ok(invalid("events_event_required"));
    };
    if c.value_kind(*event)? != Kind::String {
        return Ok(invalid("events_event_must_be_string"));
    }
    let name = c.as_string(*event)?;
    if !allowed(c, receiver, &name)? {
        return Ok(invalid("events_event_unsupported_or_too_long"));
    }
    if key == EMIT {
        return emit(c, receiver, &name, &args[1..]);
    }
    let expected = if [ON, ONCE, REMOVE].contains(&key) {
        2
    } else {
        1
    };
    if args.len() != expected {
        return Ok(invalid("events_method_arity"));
    }
    if expected == 2 && !c.is_callable(args[1]) {
        return Ok(invalid(&format!("events_listener_not_callable: {name}")));
    }
    if key == ON || key == ONCE {
        if c.array_len(old)? == MAX_LISTENERS {
            return Ok(Outcome::Throw {
                name: "RangeError".into(),
                message: "events_listener_limit".into(),
            });
        }
        let entry = c.object()?;
        let once = c.bool(key == ONCE)?;
        let fired = c.bool(false)?;
        for (property, value) in [
            ("event", *event),
            ("listener", args[1]),
            ("once", once),
            ("fired", fired),
        ] {
            c.set_property(entry, property, value)?;
        }
        let next = c.array()?;
        let mut previous_order = None;
        let mut next_order = 0;
        for i in 0..c.array_len(old)? {
            let value = c.array_get(old, i)?;
            let (event, _, _, _) = record(c, value)?;
            let order = event_order(c, value)?;
            if event == name {
                previous_order = Some(order);
            }
            next_order = next_order.max(order + 1);
            c.array_push(next, value)?;
        }
        let order = previous_order.unwrap_or(next_order);
        if order > 9007199254740990 {
            return Ok(invalid("events_order_limit"));
        }
        let order = c.number(order as f64)?;
        c.set_property(entry, "order", order)?;
        c.array_push(next, entry)?;
        c.set_private(receiver, TABLE, next)?;
        return Ok(Outcome::Return(receiver));
    }
    if key == REMOVE {
        for i in (0..c.array_len(old)?).rev() {
            let item = c.array_get(old, i)?;
            let (event, callback, _, _) = record(c, item)?;
            if event == name && c.strict_equals(callback, args[1])? {
                remove_index(c, receiver, old, i)?;
                break;
            }
        }
        return Ok(Outcome::Return(receiver));
    }
    if ![REMOVE_ALL, LISTENERS, COUNT].contains(&key) {
        return Err(corrupt("unknown function key"));
    }
    let result = c.array()?;
    let mut count = 0;
    for i in 0..c.array_len(old)? {
        let item = c.array_get(old, i)?;
        let (event, callback, _, _) = record(c, item)?;
        if event == name {
            count += 1;
            if key == LISTENERS {
                c.array_push(result, callback)?;
            }
        } else if key == REMOVE_ALL {
            c.array_push(result, item)?;
        }
    }
    if key == REMOVE_ALL {
        c.set_private(receiver, TABLE, result)?;
        return Ok(Outcome::Return(receiver));
    }
    if key == COUNT {
        return Ok(Outcome::Return(c.number(count as f64)?));
    }
    Ok(Outcome::Return(result))
}

pub struct EventsModule {
    manifest: ModuleManifest,
}
impl Default for EventsModule {
    fn default() -> Self {
        let mut keys = vec![1];
        keys.extend(FUNCTION_KEYS);
        Self {
            manifest: ModuleManifest {
                identity: ModuleIdentity {
                    id: "org.jjs.node-events".into(),
                    version: "0.1.0".into(),
                    implementation: "jjs-module-node-events-v1".into(),
                },
                api_version: MODULE_API_VERSION,
                state_version: 1,
                imports: vec!["events".into(), "node:events".into()],
                capabilities: vec![],
                dependencies: vec![],
                function_keys: keys,
                object_kind_keys: vec![1],
                deterministic_resources: vec![],
            },
        }
    }
}
impl NativeModule for EventsModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }
    fn instantiate(&self, c: &mut dyn Context) -> Result<Outcome, Error> {
        let constructor = c.function(Key(1))?;
        c.set_property(constructor, "EventEmitter", constructor)?;
        Ok(Outcome::Return(constructor))
    }
    fn call(
        &self,
        key: Key,
        _callee: Value,
        receiver: Value,
        args: &[Value],
        c: &mut dyn Context,
    ) -> Result<Outcome, Error> {
        if key == Key(1) {
            if !args.is_empty() {
                return Ok(invalid("events_constructor_options_unsupported"));
            }
            let emitter = c.module_object(ModuleObjectKind(1))?;
            install(c, emitter, &[])?;
            return Ok(Outcome::Return(emitter));
        }
        call(c, key, receiver, args)
    }
    fn resume(
        &self,
        _: ModuleContinuation,
        _: &[Value],
        _: Result<Value, String>,
        _: &mut dyn Context,
    ) -> Result<Outcome, Error> {
        Err(corrupt("events has no asynchronous continuation"))
    }
    fn event(&self, _: u32, _: Value, _: Value, _: &mut dyn Context) -> Result<Outcome, Error> {
        Err(corrupt("events has no host event contract"))
    }
}
