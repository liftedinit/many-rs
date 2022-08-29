use many_client_macros::many_client;
pub use many_modules::base::{Endpoints, Status};
use many_protocol::ManyError;

use crate::ManyClient;

#[many_client(BaseClient)]
trait BaseClientTrait {
    fn status(&self) -> Result<Status, ManyError>;
    fn heartbeat(&self) -> Result<(), ManyError>;
    fn endpoints(&self) -> Result<Endpoints, ManyError>;
}

#[derive(Debug, Clone)]
pub struct BaseClient(ManyClient);
