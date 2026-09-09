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
