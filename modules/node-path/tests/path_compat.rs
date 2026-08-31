use jjs::jjs_module_api::{
    HostCapabilityDescriptor, HostModuleCatalog, ModuleIdentity, ModuleProviderBuilder,
    ModuleSelection,
};
use jjs::{
    compile, parse, tokenize, Host, HostResult, ModuleHost, RunResult, RuntimeBuilder, Value,
};
use jjs_module_node_path::NodePathModule;
use std::sync::Arc;

struct TestHost;

impl Host for TestHost {
    fn call(&mut self, name: &str, _args: &[Value]) -> jjs::Result<HostResult> {
        Err(jjs::Error::Host(format!("unexpected host call {name}")))
    }
}

impl ModuleHost for TestHost {
    fn module_catalog(&self) -> HostModuleCatalog {
        HostModuleCatalog {
            selections: vec![ModuleSelection {
                identity: ModuleIdentity {
                    id: "org.jjs.node-path".into(),
                    version: "0.1.0".into(),
                    implementation: "jjs-module-node-path-v1".into(),
                },
                imports: vec!["path".into(), "node:path".into()],
            }],
        }
    }

    fn module_capabilities(&self) -> Vec<HostCapabilityDescriptor> {
        vec![]
    }
}

#[test]
fn require_path_and_node_path_expose_posix_compatibility() {
    let source = r#"
const path = require('path');
const nodePath = require('node:path');
let score = 0;
if (path.join('/app', 'public', '..', 'ui.html') === '/app/ui.html') score++;
if (path.resolve('public', 'ui.html') === '/public/ui.html') score++;
if (path.dirname('/app/ui.html') === '/app') score++;
if (path.basename('/app/ui.html', '.html') === 'ui') score++;
if (path.extname('/app/ui.html') === '.html') score++;
if (path.relative('/app/src', '/app/public/ui.html') === '../public/ui.html') score++;
const parsed = path.parse('/app/ui.html');
if (parsed.dir === '/app' && parsed.name === 'ui' && parsed.ext === '.html') score++;
if (path.format({ dir: '/app', name: 'ui', ext: '.html' }) === '/app/ui.html') score++;
if (nodePath.sep === '/' && nodePath.posix === nodePath) score++;
score;
"#;
    let program = compile(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
    let mut provider = ModuleProviderBuilder::new();
    provider
        .add_implementation(Arc::new(NodePathModule::default()))
        .unwrap();
    let mut host = TestHost;
    let runtime = RuntimeBuilder::new(&provider.build(), &host)
        .build_font_empty()
        .unwrap();
    let result = runtime.run(&program, &mut host, &[]).unwrap();
    assert!(
        matches!(
            result,
            RunResult::Halt {
                output: Value::Number(9.0),
                ..
            }
        ),
        "{result:?}"
    );
}
