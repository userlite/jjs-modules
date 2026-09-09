use jjs::{
    compile, parse, tokenize, Host, HostResult, ModuleHost, RunResult, RuntimeBuilder, Value,
};
use jjs_module_api::{
    HostCapabilityDescriptor, HostModuleCatalog, ModuleProviderBuilder, ModuleSelection,
    NativeModule,
};
use jjs_module_node_http::{
    decode_response, NodeHttpModule, HTTP_REQUEST_EVENT, HTTP_RESPONSE_EVENT,
};
use std::sync::Arc;

#[derive(Default)]
struct TextHttpHost {
    target: Option<Value>,
}
impl Host for TextHttpHost {
    fn call(&mut self, name: &str, args: &[Value]) -> jjs::Result<HostResult> {
        if name == "jjs:http/get" {
            return Ok(HostResult::Yield(jjs::HostRequest::new(
                name,
                args.to_vec(),
            )));
        }
        if name == "jjs:http/stream" {
            return Ok(HostResult::Done(Value::Bool(true)));
        }
        assert_eq!(name, "jjs:net/listen");
        assert_eq!(args[0], Value::Number(3000.0));
        self.target = Some(args[1].clone());
        // Shipping contract: the host confirms the assigned numeric port.
        Ok(HostResult::Done(Value::Number(3000.0)))
    }
}
impl ModuleHost for TextHttpHost {
    fn module_catalog(&self) -> HostModuleCatalog {
        HostModuleCatalog {
            selections: vec![ModuleSelection {
                identity: NodeHttpModule::default().manifest().identity.clone(),
                imports: vec!["http".into(), "node:http".into()],
            }],
        }
    }
    fn module_capabilities(&self) -> Vec<HostCapabilityDescriptor> {
        NodeHttpModule::default().manifest().capabilities.clone()
    }
}

#[test]
fn shipping_http_string_body_interpreter_and_native() {
    for jit in [false, true] {
        jjs::discard_jit_cache();
        jjs::set_enabled(jit);
        let mut host = TextHttpHost::default();
        let mut provider = ModuleProviderBuilder::new();
        provider
            .add_implementation(Arc::new(NodeHttpModule::default()))
            .unwrap();
        let runtime = RuntimeBuilder::new(&provider.build(), &host)
            .build_font_empty()
            .unwrap();
        let source = r#"
const http = require('http');
if (http !== require('node:http')) throw new Error('alias identity');
http.createServer(function(req, res) {
    var body = '';
    req.on('data', function onData(chunk) { body += chunk.toString(); });
    req.on('end', function() {
        res.writeHead(201, {'Content-Type': 'text/plain; charset=utf-8'});
        res.end(body);
    });
}).listen(3000);
"#;
        let program = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
        let result = if jit {
            runtime.run(&program, &mut host, &[])
        } else {
            runtime.run_interpreted(&program, &mut host, &[])
        };
        let RunResult::Halt { mut state, .. } = result.unwrap() else {
            panic!("listen must finish")
        };
        let callback = program
            .functions
            .iter()
            .position(|f| f.name.as_deref() == Some("onData"))
            .unwrap() as u32;
        if jit {
            jjs::force_compile(&program, callback).unwrap();
        }
        let proto = state.globals.get("Object.prototype").unwrap().clone();
        let headers = state.heap.alloc_object(proto.clone(), None).unwrap();
        let payload = state.heap.alloc_object(proto, None).unwrap();
        let text = "username=admin&password=wrongpassword + héllo 世界";
        for (name, value) in [
            ("connectionId", Value::from("connection-1")),
            ("requestId", Value::from("request-1")),
            ("method", Value::from("POST")),
            ("url", Value::from("/login")),
            ("headers", Value::Object(headers)),
            ("body", Value::from(text)),
        ] {
            state
                .heap
                .object_set(payload, name.into(), value, None)
                .unwrap();
        }
        let target = host.target.take().unwrap();
        let response = runtime
            .deliver_event(
                &program,
                &mut state,
                &mut host,
                target.clone(),
                HTTP_REQUEST_EVENT,
                Value::Object(payload),
            )
            .unwrap();
        let encoded = runtime
            .deliver_event(
                &program,
                &mut state,
                &mut host,
                target,
                HTTP_RESPONSE_EVENT,
                response,
            )
            .unwrap();
        let response = decode_response(encoded.as_str().unwrap()).unwrap();
        assert_eq!(response.status, 201);
        assert_eq!(response.body, text);
        assert_eq!(
            response.headers["content-type"],
            "text/plain; charset=utf-8"
        );
        if jit {
            assert!(jjs::native_completion_count(&program, callback) > 0);
        }
    }
}

fn listener_setup(
    source: &str,
    jit: bool,
) -> (
    jjs::Runtime,
    jjs::CompiledProgram,
    jjs::State,
    TextHttpHost,
    Value,
) {
    jjs::discard_jit_cache();
    jjs::set_enabled(jit);
    let mut host = TextHttpHost::default();
    let mut provider = ModuleProviderBuilder::new();
    provider
        .add_implementation(Arc::new(NodeHttpModule::default()))
        .unwrap();
    let runtime = RuntimeBuilder::new(&provider.build(), &host)
        .build_font_empty()
        .unwrap();
    let program = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
    let budget = jjs::StateBudget::new(1000000, 32, 64 * 1024 * 1024);
    let result = if jit {
        runtime.run_budgeted(&program, &mut host, &[], budget)
    } else {
        runtime.run_interpreted_budgeted(&program, &mut host, &[], budget)
    };
    let RunResult::Halt { state, .. } = result.unwrap() else {
        panic!("startup yield")
    };
    let target = host.target.as_ref().unwrap().clone();
    (runtime, program, state, host, target)
}
fn listener_request(
    runtime: &jjs::Runtime,
    program: &jjs::CompiledProgram,
    state: &mut jjs::State,
    host: &mut TextHttpHost,
    target: &Value,
    path: &str,
    body: &str,
) -> jjs::Result<Value> {
    state
        .budget
        .as_deref()
        .unwrap()
        .begin_delivery(20000)
        .unwrap();
    let headers = state.heap.alloc_object(Value::Null, None).unwrap();
    let payload = state.heap.alloc_object(Value::Null, None).unwrap();
    for (name, value) in [
        ("connectionId", Value::from("c1")),
        ("requestId", Value::from("r1")),
        ("method", Value::from("POST")),
        ("url", Value::from(path)),
        ("headers", Value::Object(headers)),
        ("body", Value::from(body)),
    ] {
        state
            .heap
            .object_set(payload, name.into(), value, None)
            .unwrap();
    }
    runtime.deliver_event(
        program,
        state,
        host,
        target.clone(),
        HTTP_REQUEST_EVENT,
        Value::Object(payload),
    )
}
fn listener_response(
    runtime: &jjs::Runtime,
    program: &jjs::CompiledProgram,
    state: &mut jjs::State,
    host: &mut TextHttpHost,
    target: &Value,
    response: Value,
) -> String {
    let encoded = runtime
        .deliver_event(
            program,
            state,
            host,
            target.clone(),
            HTTP_RESPONSE_EVENT,
            response,
        )
        .unwrap();
    decode_response(encoded.as_str().unwrap()).unwrap().body
}
#[test]
fn ordered_http_listeners_empty_input_errors_and_warm_callbacks() {
    let source = r#"
const http = require('http');
var ended = 0;
http.createServer(function(req, res) {
    var trace = [];
    function first(chunk) {
        if (this !== req) throw new Error('request receiver');
        if (req.url === '/throw') { try { throw new Error('listener-sentinel'); } finally { trace.push('finally'); } }
        if (req.url === '/yield') http_get('unsupported-sync-yield');
        if (req.url === '/fuel') { while (true) {} }
        trace.push('a:' + chunk.toString());
        req.off('data', second);
    }
    function second(chunk) { trace.push('b:' + chunk.toString()); }
    function recurse() { req.emit('data', ''); }
    if (req.url === '/frames') req.on('data', recurse);
    req.on('data', first).addListener('data', second).once('data', function() { trace.push('once'); });
    req.on('end', function() { ended++; trace.push('end-a'); });
    req.once('end', function() { trace.push('end-b'); res.end(trace.join('|')); });
}).listen(3000);
"#;
    for jit in [false, true] {
        let (runtime, program, mut state, mut host, target) = listener_setup(source, jit);
        let frozen = jjs::freeze(&program, &state).unwrap();
        state = runtime.thaw(&program, &frozen).unwrap();
        // Runtime.thaw deliberately removes the old budget; set one for this delivery test.
        state.budget = jjs::StateBudget::new(1000000, 32, 64 * 1024 * 1024);
        for (path, expected) in [
            ("/throw", "listener-sentinel"),
            (
                "/yield",
                "guest callback yielded during synchronous module call",
            ),
            ("/fuel", "fuel"),
            ("/frames", "call_frame"),
        ] {
            let stack_len = state.stack.len();
            let frame_depth = state.frames.len();
            let error = listener_request(
                &runtime, &program, &mut state, &mut host, &target, path, "bad",
            )
            .unwrap_err();
            if path == "/fuel" {
                assert!(matches!(error, jjs::Error::Fuel), "{error:?}");
            } else if path == "/frames" {
                assert!(matches!(error, jjs::Error::CallFrame), "{error:?}");
            } else {
                assert!(error.to_string().contains(expected), "{error:?}");
            }
            assert_eq!(
                state.frames.len(),
                frame_depth,
                "jit={jit} path={path} frame restoration"
            );
            assert_eq!(state.stack.len(), stack_len);
            let response = listener_request(
                &runtime, &program, &mut state, &mut host, &target, "/healthy", "ok",
            )
            .unwrap();
            assert_eq!(
                listener_response(&runtime, &program, &mut state, &mut host, &target, response),
                "a:ok|b:ok|once|end-a|end-b"
            );
        }
        for i in 0..12 {
            let body = if i == 0 { "" } else { "text" };
            let response = listener_request(
                &runtime, &program, &mut state, &mut host, &target, "/healthy", body,
            )
            .unwrap();
            assert_eq!(
                listener_response(&runtime, &program, &mut state, &mut host, &target, response),
                format!("a:{body}|b:{body}|once|end-a|end-b")
            );
        }
        assert_eq!(state.globals.get("ended"), Some(&Value::Number(16.0)));
        if jit {
            let index = program
                .functions
                .iter()
                .position(|f| f.name.as_deref() == Some("first"))
                .unwrap();
            assert!(jjs::native_completion_count(&program, index as u32) > 0);
        }
    }
}
#[test]
fn http_drain_and_close_listeners_survive_freeze() {
    let source = r#"
var trace = '';
require('http').createServer(function(req, res) {
    req.on('close', function() { trace += 'q1'; }).once('close', function() { trace += 'q2'; });
    res.on('close', function() { trace += 's1'; }).once('close', function() { trace += 's2'; });
    res.on('drain', function() { trace += 'd1'; }).once('drain', function() { trace += 'd2'; });
    res.flushHeaders();
}).listen(3000);
"#;
    for jit in [false, true] {
        let (runtime, program, mut state, mut host, target) = listener_setup(source, jit);
        let response = listener_request(
            &runtime, &program, &mut state, &mut host, &target, "/stream", "",
        )
        .unwrap();
        state
            .globals
            .insert_budgeted("savedResponse".into(), response.clone(), None)
            .unwrap();
        let frozen = jjs::freeze(&program, &state).unwrap();
        state = runtime.thaw(&program, &frozen).unwrap();
        for (event, sequence) in [
            (jjs_module_node_http::HTTP_DRAIN_EVENT, 2.0),
            (jjs_module_node_http::HTTP_DRAIN_EVENT, 3.0),
            (jjs_module_node_http::HTTP_CLOSE_EVENT, 4.0),
        ] {
            let payload = state.heap.alloc_object(Value::Null, None).unwrap();
            for (name, value) in [
                ("version", Value::Number(1.0)),
                ("connectionId", Value::from("c1")),
                ("requestId", Value::from("r1")),
                ("sequence", Value::Number(sequence)),
            ] {
                state
                    .heap
                    .object_set(payload, name.into(), value, None)
                    .unwrap();
            }
            runtime
                .deliver_event(
                    &program,
                    &mut state,
                    &mut host,
                    response.clone(),
                    event,
                    Value::Object(payload),
                )
                .unwrap();
        }
        assert_eq!(
            state.globals.get("trace"),
            Some(&Value::from("d1d2d1q1q2s1s2"))
        );
    }
}

#[test]
fn corrupted_stored_listener_is_an_error_not_an_absent_listener() {
    let source = "require('http').createServer(function(req,res) { res.end('must not run'); }).listen(3000);";
    let (runtime, program, mut state, mut host, target) = listener_setup(source, false);
    let Value::Object(id) = target else {
        panic!("server object")
    };
    let table = state.module_objects[&id].private[&100].clone();
    let Value::Array(table) = table else {
        panic!("listener table")
    };
    let Value::Object(entry) = state.heap.arrays[table as usize].elements[0] else {
        panic!("listener entry")
    };
    state
        .heap
        .object_set(entry, "listener".into(), Value::Number(42.0), None)
        .unwrap();
    let error = listener_request(
        &runtime,
        &program,
        &mut state,
        &mut host,
        &Value::Object(id),
        "/",
        "",
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("events_state_invalid: stored listener is not callable"));
}

#[test]
fn server_request_and_listening_listeners_keep_order() {
    let source = r#"
var trace = '';
const server = require('http').createServer();
server.on('listening', function() { if(this !== server) throw new Error('server receiver'); trace += 'l1'; });
server.once('listening', function() { trace += 'l2'; });
server.on('request', function(req, res) { trace += 'r1'; });
server.on('request', function(req, res) { trace += 'r2'; res.end(trace); });
server.listen(3000, function() { trace += 'l3'; });
"#;
    for jit in [false, true] {
        let (runtime, program, mut state, mut host, target) = listener_setup(source, jit);
        let response =
            listener_request(&runtime, &program, &mut state, &mut host, &target, "/", "").unwrap();
        assert_eq!(
            listener_response(&runtime, &program, &mut state, &mut host, &target, response),
            "l1l2l3r1r2"
        );
    }
}
