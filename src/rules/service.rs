use super::{CatalogManager, RuleEvaluator, RuleStorage};
use crate::config::ServiceConfig;

#[derive(Clone)]
pub struct RuleService {
    #[allow(dead_code)]
    config: ServiceConfig,
    evaluator: RuleEvaluator,
    storage: RuleStorage,
    catalog: CatalogManager,
}

impl RuleService {
    pub fn new(
        config: ServiceConfig,
        evaluator: RuleEvaluator,
        storage: RuleStorage,
        catalog: CatalogManager,
    ) -> Self {
        Self {
            config,
            evaluator,
            storage,
            catalog,
        }
    }

    pub fn evaluator(&self) -> &RuleEvaluator {
        &self.evaluator
    }

    pub fn storage(&self) -> &RuleStorage {
        &self.storage
    }

    pub fn catalog(&self) -> &CatalogManager {
        &self.catalog
    }
}
