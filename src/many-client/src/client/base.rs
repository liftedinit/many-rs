use many_client_macros::many_client;
use many_modules::base::{Endpoints, Status};

#[many_client(methods(
    status(returns = "Status"),
    heartbeat(),
    endpoints(returns = "Endpoints"),
))]
pub struct BaseClient;
