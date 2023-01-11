use many_error::Reason;
use many_identity::Address;
use many_types::Either;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct QueryArgs {
    #[n(0)]
    pub key: ByteVec,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct QueryReturns {
    #[n(0)]
    pub owner: Address,

    #[n(1)]
    pub disabled: Option<Either<bool, Reason<u64>>>,
}
