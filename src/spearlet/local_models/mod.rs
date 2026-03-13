pub mod controller;
pub mod llamacpp;
pub mod managed_backends;

pub use controller::LocalModelController;
pub use managed_backends::global_managed_backends;
pub use managed_backends::ManagedBackendRegistry;

pub const DEFAULT_LOCAL_MODELS_DIR: &str = "/var/lib/spear/local_models";
