use std::sync::Arc;

use jjs_module_api::{
    HostModuleCatalog, ModuleError, ModuleProvider, ModuleProviderBuilder, ModuleSelection,
    NativeModule,
};
use jjs_module_better_sqlite3::BetterSqlite3Module;
use jjs_module_crypto::CryptoModule;
use jjs_module_express::ExpressModule;
use jjs_module_hazelcast::HazelcastModule;
use jjs_module_json_schema::JsonSchemaModule;
use jjs_module_node_fs::{NodeFsModule, NodeFsPromisesModule};
use jjs_module_node_http::NodeHttpModule;
use jjs_module_node_path::NodePathModule;
use jjs_module_rate_limit::RateLimitModule;
use jjs_module_resvg::ResvgModule;
use jjs_module_schedule::ScheduleModule;
use jjs_module_service::ServiceModule;
use jjs_module_sqlite3::Sqlite3Module;
use jjs_module_text::TextModule;
use jjs_module_tps_evidence::TpsEvidenceModule;
use jjs_module_tps_fetch::TpsFetchModule;
use jjs_module_tps_notify::TpsNotifyModule;
use jjs_module_tps_secrets::TpsSecretsModule;

pub const TPS_DEFAULT_PROFILE_ID: &str = "tps-default-v2";

pub struct ModuleProfile {
    pub id: &'static str,
    pub provider: ModuleProvider,
    pub catalog: HostModuleCatalog,
}

pub fn tps_default_profile() -> Result<ModuleProfile, ModuleError> {
    let modules: Vec<Arc<dyn NativeModule>> = vec![
        Arc::new(BetterSqlite3Module::default()),
        Arc::new(NodeFsModule::default()),
        Arc::new(NodeFsPromisesModule::default()),
        Arc::new(NodeHttpModule::default()),
        Arc::new(NodePathModule::default()),
        Arc::new(ExpressModule::default()),
        Arc::new(HazelcastModule::default()),
        Arc::new(JsonSchemaModule::default()),
        Arc::new(RateLimitModule::default()),
        Arc::new(ServiceModule::default()),
        Arc::new(Sqlite3Module::default()),
        Arc::new(TextModule::default()),
        Arc::new(ScheduleModule::default()),
        Arc::new(TpsFetchModule::default()),
        Arc::new(TpsEvidenceModule::default()),
        Arc::new(TpsNotifyModule::default()),
        Arc::new(TpsSecretsModule::default()),
        Arc::new(CryptoModule::default()),
        Arc::new(ResvgModule::default()),
    ];
    let catalog = HostModuleCatalog {
        selections: modules
            .iter()
            .filter_map(|module| {
                let imports: Vec<String> = module
                    .manifest()
                    .imports
                    .iter()
                    .filter(|specifier| !specifier.starts_with("tps-"))
                    .cloned()
                    .collect();
                (!imports.is_empty()).then(|| ModuleSelection {
                    identity: module.manifest().identity.clone(),
                    imports,
                })
            })
            .collect(),
    };
    let mut builder = ModuleProviderBuilder::new();
    for module in modules {
        builder.add_implementation(module)?;
    }
    Ok(ModuleProfile {
        id: TPS_DEFAULT_PROFILE_ID,
        provider: builder.build(),
        catalog,
    })
}

pub use jjs_module_better_sqlite3::BETTER_SQLITE3_CLOSE;
pub use jjs_module_crypto::{
    capability_ids as crypto_capability_ids, CRYPTO_HMAC_SHA256, CRYPTO_RANDOM_BYTES, CRYPTO_SHA256,
};
pub use jjs_module_hazelcast::capability_ids as hazelcast_capability_ids;
pub use jjs_module_node_fs::{FS_LIST, FS_MKDIR, FS_READ, FS_STAT, FS_WRITE};
pub use jjs_module_node_http::{
    decode_response, HttpClient, HttpRequest, HttpResponse, HTTP_CLOSE_EVENT, HTTP_DRAIN_EVENT,
    HTTP_REQUEST_EVENT, HTTP_RESPONSE_EVENT, HTTP_STREAM_CONTRACT_VERSION,
};
pub use jjs_module_rate_limit::{
    capability_ids as rate_limit_capability_ids, RATE_LIMIT_CONSUME, RATE_LIMIT_OPEN,
};
pub use jjs_module_service::{
    capability_ids as service_capability_ids, SERVICE_FAIL, SERVICE_READY,
};
pub use jjs_module_sqlite3::{
    OPEN_CREATE, OPEN_READONLY, OPEN_READWRITE, SQLITE_ALL, SQLITE_CLOSE, SQLITE_EXEC, SQLITE_GET,
    SQLITE_OPEN, SQLITE_PREPARE, SQLITE_REQUEST, SQLITE_RUN, SQLITE_STATEMENT_ALL,
    SQLITE_STATEMENT_BIND, SQLITE_STATEMENT_FINALIZE, SQLITE_STATEMENT_GET, SQLITE_STATEMENT_RESET,
    SQLITE_STATEMENT_RUN,
};
pub use jjs_module_tps_evidence::{capability_ids as evidence_capability_ids, EVIDENCE_OBSERVE};
pub use jjs_module_tps_fetch::{capability_ids as fetch_capability_ids, OUTBOUND_HTTP_FETCH};
pub use jjs_module_tps_notify::{capability_ids as notify_capability_ids, NOTIFICATION_SEND};
pub use jjs_module_tps_secrets::{
    capability_ids as secrets_capability_ids, SECRETS_DELETE, SECRETS_GENERATE, SECRETS_IMPORT,
    SECRETS_ROTATE, SECRETS_VERIFY,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_has_stable_identity_and_complete_catalog() {
        let profile = tps_default_profile().expect("standard profile");
        assert_eq!(profile.id, "tps-default-v2");
        assert_eq!(profile.catalog.selections.len(), 10);
        let import_count: usize = profile
            .catalog
            .selections
            .iter()
            .map(|selection| selection.imports.len())
            .sum();
        assert_eq!(import_count, 15);
        assert!(profile.catalog.selections.iter().all(|selection| {
            selection
                .imports
                .iter()
                .all(|specifier| !specifier.starts_with("tps-"))
        }));
    }
}
