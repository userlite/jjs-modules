use jjs::{
    compile, parse, tokenize, Host, HostResult, ModuleHost, NullHost, RunResult, RuntimeBuilder,
    Value,
};
use jjs_module_api::{
    HostCapabilityDescriptor, HostModuleCatalog, ModuleProviderBuilder, ModuleSelection,
    NativeModule,
};
use jjs_module_node_events::EventsModule;
use std::sync::Arc;
struct EventsHost;
impl Host for EventsHost {
    fn call(&mut self, name: &str, args: &[Value]) -> jjs::Result<HostResult> {
        NullHost.call(name, args)
    }
}
impl ModuleHost for EventsHost {
    fn module_catalog(&self) -> HostModuleCatalog {
        HostModuleCatalog {
            selections: vec![ModuleSelection {
                identity: EventsModule::default().manifest().identity.clone(),
                imports: vec!["events".into(), "node:events".into()],
            }],
        }
    }
    fn module_capabilities(&self) -> Vec<HostCapabilityDescriptor> {
        vec![]
    }
}
fn runtime(host: &EventsHost) -> jjs::Runtime {
    let mut provider = ModuleProviderBuilder::new();
    provider
        .add_implementation(Arc::new(EventsModule::default()))
        .unwrap();
    RuntimeBuilder::new(&provider.build(), host)
        .build_font_empty()
        .unwrap()
}
#[test]
fn listeners_match_node_in_interpreter_and_jit() {
    let expected = include_str!("fixtures/listeners.expected.json").trim();
    for jit in [false, true] {
        jjs::discard_jit_cache();
        jjs::set_enabled(jit);
        let mut host = EventsHost;
        let runtime = runtime(&host);
        let program =
            compile(&parse(&tokenize(include_str!("fixtures/listeners.js")).unwrap()).unwrap())
                .unwrap();
        if jit {
            for name in ["a", "b", "d"] {
                let index = program
                    .functions
                    .iter()
                    .position(|f| f.name.as_deref() == Some(name))
                    .unwrap();
                jjs::force_compile(&program, index as u32).unwrap();
            }
        }
        let result = if jit {
            runtime.run(&program, &mut host, &[])
        } else {
            runtime.run_interpreted(&program, &mut host, &[])
        };
        let RunResult::Halt { output, .. } = result.unwrap_or_else(|e| panic!("jit={jit}: {e}"))
        else {
            panic!("unexpected yield")
        };
        assert_eq!(output.as_str().unwrap(), expected, "jit={jit}");
        if jit {
            let b = program
                .functions
                .iter()
                .position(|f| f.name.as_deref() == Some("b"))
                .unwrap();
            assert!(jjs::native_completion_count(&program, b as u32) > 0);
        }
    }
}
#[test]
fn listeners_survive_freeze_and_thaw() {
    for jit in [false, true] {
        jjs::discard_jit_cache();
        jjs::set_enabled(jit);
        let mut host = EventsHost;
        let runtime = runtime(&host);
        let source = "const E = require('events'); var e = new E(); var total = 0; function setup(n) { e.on('x', function() { total += n; }); e.once('x', function() { total += 10; }); } setup(3); http_get('pause'); e.emit('x'); e.emit('x'); return total;";
        let program = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
        let result = if jit {
            runtime.run(&program, &mut host, &[])
        } else {
            runtime.run_interpreted(&program, &mut host, &[])
        };
        let RunResult::Yield { state, .. } = result.unwrap() else {
            panic!("expected yield")
        };
        let bytes = jjs::freeze(&program, &state).unwrap();
        let mut state = runtime.thaw(&program, &bytes).unwrap();
        let result = runtime
            .resume_host(
                &program,
                &mut state,
                &mut host,
                jjs::HostCompletion::Success(Value::Undefined),
            )
            .unwrap();
        assert!(matches!(
            result,
            RunResult::Halt {
                output: Value::Number(16.0),
                ..
            }
        ));
    }
}

#[test]
fn listener_limits_and_unsupported_options_are_explicit() {
    let source = r#"
const E = require('events'); const e = new E(); function listener() {}
for (var i=0; i<1024; i++) e.on('x', listener);
var full = false; try { e.on('x', listener); } catch (error) { full = error.name === 'RangeError'; }
var options = false; try { new E({ captureRejections: true }); } catch (error) { options = error.name === 'TypeError'; }
var event = false; try { e.on(1, listener); } catch (error) { event = error.name === 'TypeError'; }
return full && options && event && e.listenerCount('x') === 1024;
"#;
    for jit in [false, true] {
        jjs::discard_jit_cache();
        jjs::set_enabled(jit);
        let mut host = EventsHost;
        let runtime = runtime(&host);
        let program = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
        let result = if jit {
            runtime.run(&program, &mut host, &[])
        } else {
            runtime.run_interpreted(&program, &mut host, &[])
        };
        assert!(matches!(
            result.unwrap(),
            RunResult::Halt {
                output: Value::Bool(true),
                ..
            }
        ));
    }
}
