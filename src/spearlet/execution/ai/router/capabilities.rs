use crate::spearlet::execution::ai::ir::Operation;

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub ops: Vec<Operation>,
    pub features: Vec<String>,
    pub transports: Vec<String>,
}

impl Capabilities {
    pub fn supports_operation(&self, op: &Operation) -> bool {
        self.ops.iter().any(|x| x == op)
    }

    pub fn has_feature(&self, f: &str) -> bool {
        self.features.iter().any(|x| x == f)
    }
}
