use super::{RuleEvaluator, RuleStorage};
use crate::config::ServiceConfig;

#[derive(Clone)]
pub struct RuleService {
    #[allow(dead_code)]
    config: ServiceConfig,
    evaluator: RuleEvaluator,
    storage: RuleStorage,
}

impl RuleService {
    pub fn new(config: ServiceConfig, evaluator: RuleEvaluator, storage: RuleStorage) -> Self {
        Self {
            config,
            evaluator,
            storage,
        }
    }

    pub fn evaluator(&self) -> &RuleEvaluator {
        &self.evaluator
    }

    pub fn storage(&self) -> &RuleStorage {
        &self.storage
    }
}
