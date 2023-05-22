use many_client_macros::many_client;
use many_error::ManyError;
pub use many_identity::Identity;
use many_modules::base::HeartbeatReturn;
pub use many_modules::base::{Endpoints, Status};

use crate::ManyClient;

#[many_client(BaseClient)]
trait BaseClientTrait {
    fn status(&self) -> Result<Status, ManyError>;
    fn heartbeat(&self) -> Result<HeartbeatReturn, ManyError>;
    fn endpoints(&self) -> Result<Endpoints, ManyError>;
}

#[derive(Debug, Clone)]
pub struct BaseClient<I: Identity>(ManyClient<I>);
