use jjs::{
    compile, parse, tokenize, Host, HostResult, ModuleHost, NullHost, RunResult, RuntimeBuilder,
    Value,
};
use jjs_module_api::{
    HostCapabilityDescriptor, HostModuleCatalog, ModuleProviderBuilder, ModuleSelection,
    NativeModule,
};
use jjs_module_node_querystring::QuerystringModule;
use std::sync::Arc;
struct ParserHost;
impl Host for ParserHost {
    fn call(&mut self, n: &str, a: &[Value]) -> jjs::Result<HostResult> {
        NullHost.call(n, a)
    }
}
impl ModuleHost for ParserHost {
    fn module_catalog(&self) -> HostModuleCatalog {
        let m = QuerystringModule::default();
        HostModuleCatalog {
            selections: vec![ModuleSelection {
                identity: m.manifest().identity.clone(),
                imports: m.manifest().imports.clone(),
            }],
        }
    }
    fn module_capabilities(&self) -> Vec<HostCapabilityDescriptor> {
        vec![]
    }
}
fn runtime(host: &ParserHost) -> jjs::Runtime {
    let mut p = ModuleProviderBuilder::new();
    p.add_implementation(Arc::new(QuerystringModule::default()))
        .unwrap();
    RuntimeBuilder::new(&p.build(), host)
        .build_font_empty()
        .unwrap()
}
fn execute(source: &str, jit: bool) -> jjs::Result<RunResult> {
    jjs::discard_jit_cache();
    jjs::set_enabled(jit);
    let mut host = ParserHost;
    let rt = runtime(&host);
    let p = compile(&parse(&tokenize(source)?)?)?;
    let probe = p
        .functions
        .iter()
        .position(|f| f.name.as_deref() == Some("probe"));
    if jit {
        if let Some(i) = probe {
            jjs::force_compile(&p, i as u32).expect("compile parser probe");
        }
    }
    let result = if jit {
        rt.run(&p, &mut host, &[])
    } else {
        rt.run_interpreted(&p, &mut host, &[])
    };
    if jit && result.is_ok() {
        if let Some(i) = probe {
            assert!(
                jjs::native_completion_count(&p, i as u32) > 0,
                "native parser caller must complete"
            );
        }
    }
    result
}
#[test]
fn differential_node_24_interpreter_and_native() {
    for jit in [false, true] {
        let result = execute(include_str!("fixtures/parsing.js"), jit)
            .unwrap_or_else(|e| panic!("jit={jit}: {e}"));
        let RunResult::Halt { output, .. } = result else {
            panic!("unexpected yield")
        };
        assert_eq!(
            output.as_str().unwrap(),
            include_str!("fixtures/parsing.expected.json").trim(),
            "jit={jit}"
        );
    }
}
#[test]
fn unknown_import_is_rejected() {
    for jit in [false, true] {
        assert!(execute("return require('node:parser-does-not-exist');", jit).is_err());
    }
}
#[test]
fn unsupported_options_and_limits_fail_explicitly() {
    for jit in [false, true] {
        for source in ["require('querystring').parse('x=1','&','=',{decodeURIComponent:function(x){return x;}});", "require('querystring').stringify({a:1},'&','=',{encodeURIComponent:function(x){return x;}});", "require('querystring').parse('x=1','','=');", "require('querystring').parse('x=1','&','=',{maxKeys:-1});"]{assert!(execute(source,jit).is_err(),"{source}");}
        let source = format!(
            "return require('querystring').parse('{}');",
            "x".repeat(65537)
        );
        assert!(execute(&source, jit)
            .unwrap_err()
            .to_string()
            .contains("parser_input_limit"));
        let source = format!(
            "return require('querystring').parse('{}','&','=',{{maxKeys:0}});",
            vec!["x=1"; 4097].join("&")
        );
        assert!(execute(&source, jit)
            .unwrap_err()
            .to_string()
            .contains("parser_pair_limit"));
    }
}
