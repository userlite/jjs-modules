use std::sync::Arc;

use jjs_module_api::{
    HostModuleCatalog, ModuleError, ModuleProvider, ModuleProviderBuilder, ModuleSelection,
    NativeModule,
};
use jjs_module_crypto::CryptoModule;
use jjs_module_express::ExpressModule;
use jjs_module_hazelcast::HazelcastModule;
use jjs_module_json_schema::JsonSchemaModule;
use jjs_module_node_http::NodeHttpModule;
use jjs_module_rate_limit::RateLimitModule;
use jjs_module_resvg::ResvgModule;
use jjs_module_schedule::ScheduleModule;
use jjs_module_service::ServiceModule;
use jjs_module_text::TextModule;
use jjs_module_tps_evidence::TpsEvidenceModule;
use jjs_module_tps_fetch::TpsFetchModule;
use jjs_module_tps_notify::TpsNotifyModule;
use jjs_module_tps_secrets::TpsSecretsModule;

pub const TPS_DEFAULT_PROFILE_ID: &str = "tps-default-v1";

pub struct ModuleProfile {
    pub id: &'static str,
    pub provider: ModuleProvider,
    pub catalog: HostModuleCatalog,
}

pub fn tps_default_profile() -> Result<ModuleProfile, ModuleError> {
    let modules: Vec<Arc<dyn NativeModule>> = vec![
        Arc::new(NodeHttpModule::default()),
        Arc::new(ExpressModule::default()),
        Arc::new(HazelcastModule::default()),
        Arc::new(JsonSchemaModule::default()),
        Arc::new(RateLimitModule::default()),
        Arc::new(ServiceModule::default()),
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
            .map(|module| ModuleSelection {
                identity: module.manifest().identity.clone(),
                imports: module.manifest().imports.clone(),
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

pub use jjs_module_crypto::{
    CRYPTO_HMAC_SHA256, CRYPTO_RANDOM_BYTES, CRYPTO_SHA256,
    capability_ids as crypto_capability_ids,
};
pub use jjs_module_hazelcast::capability_ids as hazelcast_capability_ids;
pub use jjs_module_node_http::{
    HTTP_CLOSE_EVENT, HTTP_DRAIN_EVENT, HTTP_REQUEST_EVENT, HTTP_RESPONSE_EVENT,
    HTTP_STREAM_CONTRACT_VERSION, HttpClient, HttpRequest, HttpResponse, decode_response,
};
pub use jjs_module_rate_limit::{
    RATE_LIMIT_CONSUME, RATE_LIMIT_OPEN, capability_ids as rate_limit_capability_ids,
};
pub use jjs_module_service::{
    SERVICE_FAIL, SERVICE_READY, capability_ids as service_capability_ids,
};
pub use jjs_module_tps_evidence::{
    EVIDENCE_OBSERVE, capability_ids as evidence_capability_ids,
};
pub use jjs_module_tps_fetch::{
    OUTBOUND_HTTP_FETCH, capability_ids as fetch_capability_ids,
};
pub use jjs_module_tps_notify::{
    NOTIFICATION_SEND, capability_ids as notify_capability_ids,
};
pub use jjs_module_tps_secrets::{
    SECRETS_DELETE, SECRETS_GENERATE, SECRETS_IMPORT, SECRETS_ROTATE, SECRETS_VERIFY,
    capability_ids as secrets_capability_ids,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_has_stable_identity_and_complete_catalog() {
        let profile = tps_default_profile().expect("standard profile");
        assert_eq!(profile.id, "tps-default-v1");
        assert_eq!(profile.catalog.selections.len(), 14);
        let import_count: usize = profile
            .catalog
            .selections
            .iter()
            .map(|selection| selection.imports.len())
            .sum();
        assert_eq!(import_count, 15);
    }
}

