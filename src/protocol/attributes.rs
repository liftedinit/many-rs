pub mod network {
    use crate::protocol::Attribute;
    pub const BASE: Attribute = Attribute::id(0);
}

pub mod response {
    use crate::protocol::Attribute;

    pub const ASYNC: Attribute = Attribute::id(1);
}
