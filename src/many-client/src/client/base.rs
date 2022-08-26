use many_client_macros::many_client;
pub use many_modules::base::{Endpoints, Status};
use many_protocol::ManyError;

use crate::ManyClient;

#[many_client(BaseClient)]
trait BaseClientTrait {
    async fn status(&self) -> Result<Status, ManyError>;
    async fn heartbeat(&self) -> Result<(), ManyError>;
    async fn endpoints(&self) -> Result<Endpoints, ManyError>;
}

pub struct BaseClient(ManyClient);
