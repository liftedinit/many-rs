use crate as module;
use crate::account::features::multisig::MultisigTransactionState;
use many_error::{ManyError, Reason};
use many_identity::Address;
use many_macros::many_module;
use many_protocol::ResponseMessage;
use many_types::ledger;
use many_types::ledger::{Symbol, TokenAmount};
use many_types::{AttributeRelatedIndex, CborRange, Memo, Timestamp, VecOrSingle};
use minicbor::bytes::ByteVec;
use minicbor::{encode, Decode, Decoder, Encode, Encoder};
use num_bigint::BigUint;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use mockall::{automock, predicate::*};

mod info;
mod list;

pub use info::*;
pub use list::*;

#[many_module(name = EventsModule, id = 4, namespace = events, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait EventsModuleBackend: Send {
    fn info(&self, args: InfoArgs) -> Result<InfoReturn, ManyError>;
    fn list(&self, args: ListArgs) -> Result<ListReturns, ManyError>;
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct EventId(ByteVec);

impl From<ByteVec> for EventId {
    fn from(t: ByteVec) -> EventId {
        EventId(t)
    }
}

impl From<EventId> for ByteVec {
    fn from(id: EventId) -> Self {
        id.0
    }
}

impl AsRef<[u8]> for EventId {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<Vec<u8>> for EventId {
    fn from(t: Vec<u8>) -> EventId {
        EventId(ByteVec::from(t))
    }
}

impl From<u64> for EventId {
    fn from(v: u64) -> EventId {
        EventId(ByteVec::from(v.to_be_bytes().to_vec()))
    }
}

impl From<BigUint> for EventId {
    fn from(b: BigUint) -> EventId {
        EventId(ByteVec::from(b.to_bytes_be()))
    }
}

impl std::ops::Add<ByteVec> for EventId {
    type Output = EventId;

    fn add(self, rhs: ByteVec) -> Self::Output {
        (BigUint::from_bytes_be(&self.0) + BigUint::from_bytes_be(&rhs)).into()
    }
}

impl std::ops::Add<u32> for EventId {
    type Output = EventId;

    fn add(self, rhs: u32) -> Self::Output {
        (BigUint::from_bytes_be(&self.0) + rhs).into()
    }
}

impl std::ops::AddAssign<u32> for EventId {
    fn add_assign(&mut self, other: u32) {
        *self = self.clone() + other;
    }
}

impl std::ops::Sub<ByteVec> for EventId {
    type Output = EventId;

    fn sub(self, rhs: ByteVec) -> Self::Output {
        (BigUint::from_bytes_be(&self.0) - BigUint::from_bytes_be(&rhs)).into()
    }
}

impl std::ops::Sub<u32> for EventId {
    type Output = EventId;

    fn sub(self, rhs: u32) -> Self::Output {
        (BigUint::from_bytes_be(&self.0) - rhs).into()
    }
}

impl<C> Encode<C> for EventId {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.bytes(&self.0)?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for EventId {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        Ok(EventId(ByteVec::from(d.bytes()?.to_vec())))
    }
}

impl From<EventId> for Vec<u8> {
    fn from(t: EventId) -> Vec<u8> {
        t.0.to_vec()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventFilter {
    pub account: Option<VecOrSingle<Address>>,

    pub kind: Option<VecOrSingle<EventKind>>,

    // TODO: remove this. Kept for backward compatibility.
    pub symbol: Option<VecOrSingle<Address>>,

    pub id_range: Option<CborRange<EventId>>,

    pub date_range: Option<CborRange<Timestamp>>,

    pub events_filter_attribute_specific:
        BTreeMap<EventFilterAttributeSpecificIndex, EventFilterAttributeSpecific>,
}

impl<C> Encode<C> for EventFilter {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.map(5 + self.events_filter_attribute_specific.len() as u64)?
            .u8(0)?
            .encode(&self.account)?
            .u8(1)?
            .encode(&self.kind)?
            .u8(2)?
            .encode(&self.symbol)?
            .u8(3)?
            .encode(&self.id_range)?
            .u8(4)?
            .encode(self.date_range)?;
        for (key, value) in self.events_filter_attribute_specific.iter() {
            e.encode(key)?.encode(value)?;
        }
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for EventFilter {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        use minicbor::decode::Error;

        let len = d.map()?;
        let mut account = None;
        let mut kind = None;
        let mut symbol = None;
        let mut id_range = None;
        let mut date_range = None;
        let mut events_filter_attribute_specific = BTreeMap::new();
        for _ in 0..len.unwrap_or_default() {
            use minicbor::data::Type;
            match d.datatype()? {
                Type::U8 | Type::U16 | Type::U32 | Type::U64 => {
                    let index = d.u16()?;
                    match index {
                        0 => account = d.decode()?,
                        1 => kind = d.decode()?,
                        2 => symbol = d.decode()?,
                        3 => id_range = d.decode()?,
                        4 => date_range = d.decode()?,
                        i => return Err(Error::message(format!("Unknown key {i}"))),
                    }
                }
                Type::Array => {
                    let mut key: EventFilterAttributeSpecificIndex = d.decode()?;
                    events_filter_attribute_specific.insert(key, d.decode_with(&mut key)?);
                }
                t => return Err(Error::type_mismatch(t)),
            }
        }
        Ok(EventFilter {
            account,
            kind,
            symbol,
            id_range,
            date_range,
            events_filter_attribute_specific,
        })
    }
}

// TODO refactor to a trait object
#[derive(Clone, Copy, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub enum EventFilterAttributeSpecificIndex {
    MultisigTransactionState,
}

impl From<EventFilterAttributeSpecificIndex> for AttributeRelatedIndex {
    fn from(idx: EventFilterAttributeSpecificIndex) -> Self {
        match idx {
            EventFilterAttributeSpecificIndex::MultisigTransactionState => {
                Self::new(9).with_index(1).with_index(0)
            }
        }
    }
}

impl TryFrom<AttributeRelatedIndex> for EventFilterAttributeSpecificIndex {
    type Error = minicbor::decode::Error;
    fn try_from(idx: AttributeRelatedIndex) -> Result<Self, Self::Error> {
        if idx == AttributeRelatedIndex::new(9).with_index(1).with_index(0) {
            return Ok(EventFilterAttributeSpecificIndex::MultisigTransactionState);
        }
        Err(Self::Error::message(format!("Unknown variant {idx:?}")))
    }
}

impl<C> Encode<C> for EventFilterAttributeSpecificIndex {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        let a: AttributeRelatedIndex = (*self).into();
        e.encode(a)?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for EventFilterAttributeSpecificIndex {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        let a: AttributeRelatedIndex = d.decode()?;
        a.try_into()
    }
}

// TODO refactor to a trait object
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum EventFilterAttributeSpecific {
    MultisigTransactionState(VecOrSingle<MultisigTransactionState>),
}

impl<C> Encode<C> for EventFilterAttributeSpecific {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        match self {
            EventFilterAttributeSpecific::MultisigTransactionState(state) => e.encode(state),
        }
        .map(|_| ())
    }
}

impl<'b> Decode<'b, EventFilterAttributeSpecificIndex> for EventFilterAttributeSpecific {
    fn decode(
        d: &mut Decoder<'b>,
        ctx: &mut EventFilterAttributeSpecificIndex,
    ) -> Result<Self, minicbor::decode::Error> {
        match ctx {
            EventFilterAttributeSpecificIndex::MultisigTransactionState => {
                Ok(Self::MultisigTransactionState(d.decode()?))
            }
        }
    }
}

macro_rules! define_event_kind {
    ( $( [ $index: literal $(, $sub: literal )* ] $name: ident { $( $idx: literal | $fname: ident : $type: ty, )* }, )* ) => {
        #[derive(
            Copy,
            Clone,
            Debug,
            Ord,
            PartialOrd,
            Eq,
            PartialEq,
            strum_macros::Display,
            strum_macros::EnumIter,
            strum_macros::EnumString,
        )]
        #[repr(u8)]
        #[strum(serialize_all = "kebab-case")]
        #[non_exhaustive]
        pub enum EventKind {
            $( $name ),*
        }

        impl From<EventKind> for AttributeRelatedIndex {
            fn from(other: EventKind) -> Self {
                match other {
                    $( EventKind :: $name => AttributeRelatedIndex::new($index) $(.with_index($sub))* ),*
                }
            }
        }

        impl From<&EventInfo> for EventKind {
            fn from(other: &EventInfo) -> Self {
                match other {
                    $( EventInfo :: $name { .. } => EventKind :: $name ),*
                }
            }
        }

        impl TryFrom<AttributeRelatedIndex> for EventKind {
            type Error = Vec<u32>;

            fn try_from(other: AttributeRelatedIndex) -> Result<Self, Vec<u32>> {
                match &other.flattened()[..] {
                    $( [ $index $(, $sub)* ] => Ok( EventKind :: $name ), )*
                    x => Err(x.to_vec()),
                }
            }
        }

        impl<C> Encode<C> for EventKind {
            fn encode<W: encode::Write>(&self, e: &mut Encoder<W>, ctx: &mut C) -> Result<(), encode::Error<W::Error>> {
                Into::<AttributeRelatedIndex>::into(*self).encode(e, ctx)
            }
        }

        impl<'b, C> Decode<'b, C> for EventKind {
            fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
                TryFrom::try_from(d.decode::<AttributeRelatedIndex>()?)
                    .map_err(|_| minicbor::decode::Error::message("Invalid attribute index"))
            }
        }
    }
}

macro_rules! define_event_info_symbol {
    (@pick_symbol) => {};
    (@pick_symbol $name: ident symbol $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        return Some(& $name)
    };
    (@pick_symbol $name_: ident $( $tag_: ident )*, $( $name: ident $( $tag: ident )*, )* ) => {
        define_event_info_symbol!(@pick_symbol $( $name $( $tag )*, )* )
    };

    (@inner) => {};
    (@inner $name: ident inner $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        if let Some(s) = $name .symbol() {
            return Some(s);
        }
    };
    (@inner $name_: ident $( $tag_: ident )*, $( $name: ident $( $tag: ident )*, )* ) => {
        define_event_info_symbol!(@inner $( $name $( $tag )*, )* )
    };

    ( $( $name: ident { $( $fname: ident $( $tag: ident )* , )* } )* ) => {
        pub fn symbol(&self) -> Option<&Symbol> {
            match self {
                $( EventInfo :: $name {
                    $( $fname, )*
                } => {
                    // Remove warnings.
                    $( let _ = $fname; )*
                    define_event_info_symbol!(@pick_symbol $( $fname $( $tag )*, )* );

                    // If we're here, we need to go deeper. Check if there's an inner.
                    define_event_info_symbol!(@inner $( $fname $( $tag )*, )*);
                } )*
            }

            None
        }
    };
}

macro_rules! define_event_info_addresses {
    (@field $set: ident) => {};
    (@field $set: ident $name: ident id $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        $set.insert(&$name);
        define_event_info_addresses!(@field $set $( $name_ $( $tag_ )*, )* );
    };
    (@field $set: ident $name: ident id_non_null $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        if let Some(n) = $name.as_ref() {
            $set.insert(n);
        }
        define_event_info_addresses!(@field $set $( $name_ $( $tag_ )*, )* );
    };
    // TODO: Make this recursive...
    (@field $set: ident $name: ident double_id_non_null $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        if let Some(ref _r @ Some(ref n)) = $name.as_ref() {
            $set.insert(n);
        }
        define_event_info_addresses!(@field $set $( $name_ $( $tag_ )*, )* );
    };
    (@field $set: ident $name_: ident $( $tag_: ident )*, $( $name: ident $( $tag: ident )*, )* ) => {
        define_event_info_addresses!(@field $set $( $name $( $tag )*, )* );
    };

    (@inner $set: ident) => {};
    (@inner $set: ident $name: ident inner $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        $set.append( &mut $name.addresses() );
        define_event_info_addresses!(@inner $set $( $name_ $( $tag_ )*, )* );
    };
    (@inner $set: ident $name_: ident $( $tag_: ident )*, $( $name: ident $( $tag: ident )*, )* ) => {
        define_event_info_addresses!(@inner $set $( $name $( $tag )*, )* );
    };

    ( $( $name: ident { $( $fname: ident $( $tag: ident )* , )* } )* ) => {
        pub fn addresses(&self) -> BTreeSet<&Address> {
            match self {
                $( EventInfo :: $name {
                    $( $fname, )*
                } => {
                    // Remove warnings.
                    $( let _ = $fname; )*

                    let mut set = BTreeSet::<&Address>::new();

                    define_event_info_addresses!(@field set $( $fname $( $tag )*, )* );

                    // Inner fields might match the address.
                    define_event_info_addresses!(@inner set $( $fname $( $tag )*, )* );

                    return set;
                } )*
            }
        }
    };
}

macro_rules! define_event_info {
    ( $( $name: ident { $( $idx: literal | $fname: ident : $type: ty $([ $( $tag: ident )* ])?, )* }, )* ) => {
        #[derive(Clone, Debug)]
        #[non_exhaustive]
        pub enum EventInfo {
            $( $name {
                $( $fname: $type ),*
            } ),*
        }

        impl EventInfo {
            define_event_info_symbol!( $( $name { $( $fname $( $( $tag )* )?, )* } )* );
            define_event_info_addresses!( $( $name { $( $fname $( $( $tag )* )?, )* } )* );

            fn is_about(&self, id: &Address) -> bool {
                self.addresses().contains(id)
            }
        }

        encode_event_info!( $( $name { $( $idx => $fname : $type, )* }, )* );
    };
}

// This is necessary because variables must be used in repeating patterns.
macro_rules! replace_expr {
    ($_t:tt $sub:expr) => {
        $sub
    };
}

macro_rules! encode_event_info {
    ( $( $sname: ident { $( $idx: literal => $name: ident : $type: ty, )* }, )* ) => {
        impl<C> Encode<C> for EventInfo {
            fn encode<W: encode::Write>(
                &self,
                e: &mut Encoder<W>,
                _: &mut C,
            ) -> Result<(), encode::Error<W::Error>> {
                match self {
                    $(  EventInfo :: $sname { $( $name, )* } => {
                            e.map( 1u64 $(+ replace_expr!($idx 1u64))* )?
                                .u8(0)?.encode(EventKind :: $sname)?
                                $( .u8($idx)?.encode($name)? )*
                            ;
                            Ok(())
                        }, )*
                }
            }
        }

        impl<'b, C> Decode<'b, C> for EventInfo {
            fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
                let mut len = d.map()?.ok_or(minicbor::decode::Error::message(
                    "Invalid event type.",
                ))?;

                if d.u8()? != 0 {
                    return Err(minicbor::decode::Error::message(
                        "EventKind should be the first item.",
                    ));
                }
                #[allow(unreachable_patterns)]
                match d.decode::<EventKind>()? {
                    $(  EventKind :: $sname => {
                        $( let mut $name : Option< $type > = None; )*
                        // len also includes the index 0 which is treated outside this macro.
                        while len > 1 {
                            match d.u32()? {
                                $( $idx => $name = Some(d.decode()?), )*

                                x => return Err(minicbor::decode::Error::unknown_variant(x)),
                            }
                            len -= 1;
                        }

                        $( let $name: $type = $name.ok_or(minicbor::decode::Error::missing_value($idx))?; )*

                        Ok(EventInfo :: $sname {
                            $( $name, )*
                        })
                    }, )*
                    _ => Err(minicbor::decode::Error::message("Unsupported event kind"))
                }
            }
        }
    }
}

macro_rules! define_multisig_event {
    ( $( $name: ident $(: $arg: ty )?, )* ) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        #[non_exhaustive]
        pub enum AccountMultisigTransaction {
            $( $( $name($arg), )? )*
        }

        impl AccountMultisigTransaction {
            pub fn symbol(&self) -> Option<&Address> {
                // TODO: implement this for recursively checking if inner infos
                // has a symbol defined.
                None
            }

            pub fn addresses(&self) -> BTreeSet<&Address> {
                BTreeSet::new()
            }

            pub fn is_about(&self, _id: &Address) -> bool {
                false
            }
        }

        impl<C> Encode<C> for AccountMultisigTransaction {
            fn encode<W: encode::Write>(
                &self,
                e: &mut Encoder<W>,
                _: &mut C,
            ) -> Result<(), encode::Error<W::Error>> {
                match self {
                    $(
                    $( AccountMultisigTransaction :: $name(arg) => {
                        let _: $arg;  // We do this to remove a macro error for not using $arg.
                        e.map(2)?
                         .u8(0)?.encode(EventKind:: $name)?
                         .u8(1)?.encode(arg)?;
                    }, )?
                    )*
                }
                Ok(())
            }
        }

        impl<'b, C> Decode<'b, C> for AccountMultisigTransaction {
            fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
                let len = d.map()?.ok_or(minicbor::decode::Error::message(
                    "Invalid event type.",
                ))?;

                if len != 2 {
                    return Err(minicbor::decode::Error::message("Transactions must have 2 values"));
                }
                if d.u8()? != 0 {
                    return Err(minicbor::decode::Error::message(
                        "EventKind should be the first item.",
                    ));
                }
                #[allow(unreachable_patterns)]
                match d.decode::<EventKind>()? {
                    $(
                    $(  EventKind :: $name => {
                        let _: $arg;  // We do this to remove a macro error for not using $arg.
                        if d.u8()? != 1 {
                            Err(minicbor::decode::Error::message("Invalid field index"))
                        } else {
                            Ok(Self :: $name(d.decode()?))
                        }
                    }, )?
                    )*
                    _ => return Err(minicbor::decode::Error::message("Unsupported transaction kind"))
                }
            }
        }
    }
}

macro_rules! define_event {
    ( $( [ $index: literal $(, $sub: literal )* ] $name: ident $(($method_arg: ty))? { $( $idx: literal | $fname: ident : $type: ty $([ $($tag: ident)* ])?, )* }, )* ) => {
        define_event_kind!( $( [ $index $(, $sub )* ] $name { $( $idx | $fname : $type, )* }, )* );
        define_event_info!( $( $name { $( $idx | $fname : $type $([ $( $tag )* ])?, )* }, )* );

        define_multisig_event!( $( $name $(: $method_arg)?, )*);
    }
}

// We flatten the attribute related index here, but it is unflattened when serializing.
define_event! {
    [6, 0]      Send (module::ledger::SendArgs) {
        1     | from:                   Address                                [ id ],
        2     | to:                     Address                                [ id ],
        3     | symbol:                 Symbol                                 [ symbol ],
        4     | amount:                 TokenAmount,
        5     | memo:                   Option<Memo>,
    },
    [7, 0]      KvStorePut (module::kvstore::PutArgs) {
        1     | key:                    ByteVec,
        2     | value:                  ByteVec,
        3     | owner:                  Option<Address>                        [ id_non_null ],
    },
    [7, 1]      KvStoreDisable (module::kvstore::DisableArgs) {
        1     | key:                    ByteVec,
        2     | owner:                  Option<Address>                        [ id_non_null ],
        3     | reason:                 Option<Reason<u64>> ,
    },
    [9, 0]      AccountCreate (module::account::CreateArgs) {
        1     | account:                Address                                [ id ],
        2     | description:            Option<String>,
        3     | roles:                  BTreeMap<Address, BTreeSet<module::account::Role>>,
        4     | features:               module::account::features::FeatureSet,
    },
    [9, 1]      AccountSetDescription (module::account::SetDescriptionArgs) {
        1     | account:                Address                                [ id ],
        2     | description:            String,
    },
    [9, 2]      AccountAddRoles (module::account::AddRolesArgs) {
        1     | account:                Address                                [ id ],
        2     | roles:                  BTreeMap<Address, BTreeSet<module::account::Role>>,
    },
    [9, 3]      AccountRemoveRoles (module::account::RemoveRolesArgs) {
        1     | account:                Address                                [ id ],
        2     | roles:                  BTreeMap<Address, BTreeSet<module::account::Role>>,
    },
    [9, 4]      AccountDisable (module::account::DisableArgs) {
        1     | account:                Address                                [ id ],
    },
    [9, 5]      AccountAddFeatures (module::account::AddFeaturesArgs) {
        1     | account:                Address                                [ id ],
        2     | roles:                  BTreeMap<Address, BTreeSet<module::account::Role>>,
        3     | features:               module::account::features::FeatureSet,
    },
    [9, 1, 0]   AccountMultisigSubmit (module::account::features::multisig::SubmitTransactionArgs) {
        1     | submitter:              Address                                [ id ],
        2     | account:                Address                                [ id ],
        3     | memo:                   Option<Memo>,
        4     | transaction:            Box<AccountMultisigTransaction>        [ inner ],
        5     | token:                  Option<ByteVec>,
        6     | threshold:              u64,
        7     | timeout:                Timestamp,
        8     | execute_automatically:  bool,
    },
    [9, 1, 1]   AccountMultisigApprove (module::account::features::multisig::ApproveArgs) {
        1     | account:                Address                                [ id ],
        2     | token:                  ByteVec,
        3     | approver:               Address                                [ id ],
    },
    [9, 1, 2]   AccountMultisigRevoke (module::account::features::multisig::RevokeArgs) {
        1     | account:                Address                                [ id ],
        2     | token:                  ByteVec,
        3     | revoker:                Address                                [ id ],
    },
    [9, 1, 3]   AccountMultisigExecute (module::account::features::multisig::ExecuteArgs) {
        1     | account:                Address                                [ id ],
        2     | token:                  ByteVec,
        3     | executer:               Option<Address>                        [ id_non_null ],
        4     | response:               ResponseMessage,
    },
    [9, 1, 4]   AccountMultisigWithdraw (module::account::features::multisig::WithdrawArgs) {
        1     | account:                Address                                [ id ],
        2     | token:                  ByteVec,
        3     | withdrawer:             Address                                [ id ],
    },
    [9, 1, 5]   AccountMultisigSetDefaults (module::account::features::multisig::SetDefaultsArgs) {
        1     | submitter:              Address                                [ id ],
        2     | account:                Address                                [ id ],
        3     | threshold:              Option<u64>,
        4     | timeout_in_secs:        Option<u64>,
        5     | execute_automatically:  Option<bool>,
    },
    [9, 1, 6]   AccountMultisigExpired {
        1     | account:                Address                                [ id ],
        2     | token:                  ByteVec,
        3     | time:                   Timestamp,
    },
    [11, 0]     TokenCreate (module::ledger::TokenCreateArgs) {
        1     | summary:                ledger::TokenInfoSummary,
        2     | symbol:                 Address                                [ id ],
        3     | owner:                  Option<Address>                        [ id_non_null ],
        4     | initial_distribtion:    Option<ledger::LedgerTokensAddressMap>,
        5     | maximum_supply:         Option<ledger::TokenAmount>,
        6     | extended_info:          Option<module::ledger::extended_info::TokenExtendedInfo>,
    },
    [11, 1]     TokenUpdate (module::ledger::TokenUpdateArgs) {
        1     | symbol:                 Address                                [ id ],
        2     | name:                   Option<String>,
        3     | ticker:                 Option<String>,
        4     | decimals:               Option<u64>,
        5     | owner:                  Option<Option<Address>>                [ double_id_non_null ],
        6     | memo:                   Option<Memo>,
    },
    [11, 2]     TokenAddExtendedInfo (module::ledger::TokenAddExtendedInfoArgs) {
        1     | symbol:                 Address                                [ id ],
        2     | extended_info:          Vec<AttributeRelatedIndex>,
    },
    [11, 3]     TokenRemoveExtendedInfo (module::ledger::TokenRemoveExtendedInfoArgs) {
        1     | symbol:                 Address                                [ id ],
        2     | extended_info:          Vec<AttributeRelatedIndex>,
    },
    [12, 0]     TokenMint (module::ledger::TokenMintArgs) {
        1     | symbol:                 Address                                [ id ],
        2     | initial_distribtion:    Option<ledger::LedgerTokensAddressMap>,
        3     | memo:                   Option<Memo>,
    },
    [12, 1]     TokenBurn (module::ledger::TokenBurnArgs) {
        1     | symbol:                 Address                                [ id ],
        2     | distribtion:            Option<ledger::LedgerTokensAddressMap>,
        3     | memo:                   Option<Memo>,
    },
}

/// An Event that happened on the server and that is part of the log.
#[derive(Debug, Encode, Decode)]
#[cbor(map)]
pub struct EventLog {
    #[n(0)]
    pub id: EventId,

    #[n(1)]
    pub time: Timestamp,

    #[n(2)]
    pub content: EventInfo,
}

impl EventLog {
    pub fn kind(&self) -> EventKind {
        EventKind::from(&self.content)
    }

    pub fn symbol(&self) -> Option<&Address> {
        self.content.symbol()
    }

    pub fn is_about(&self, id: &Address) -> bool {
        self.content.is_about(id)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use many_identity::testing::identity;

    #[test]
    fn eventid_from_bytevec() {
        let b = ByteVec::from(vec![1, 2, 3, 4, 5]);
        let t = EventId::from(b.clone());

        assert_eq!(b.as_slice(), Into::<Vec<u8>>::into(t));
    }

    #[test]
    fn eventid_from_biguint() {
        let v = u64::MAX;
        let t = EventId::from(BigUint::from(v));

        assert_eq!(v.to_be_bytes(), Into::<Vec<u8>>::into(t).as_slice());
    }

    #[test]
    fn eventid_from_u64() {
        let v = u64::MAX;
        let t = EventId::from(v);

        assert_eq!(v.to_be_bytes(), Into::<Vec<u8>>::into(t).as_slice());
    }

    #[test]
    fn eventid_add() {
        let v = u64::MAX;
        let mut t = EventId::from(v) + 1;

        assert_eq!(
            Into::<Vec<u8>>::into(t.clone()),
            (BigUint::from(u64::MAX) + 1u32).to_bytes_be()
        );
        t += 1;
        assert_eq!(
            Into::<Vec<u8>>::into(t),
            (BigUint::from(u64::MAX) + 2u32).to_bytes_be()
        );

        let b = ByteVec::from(v.to_be_bytes().to_vec());
        let t2 = EventId::from(v) + b;

        assert_eq!(
            Into::<Vec<u8>>::into(t2),
            (BigUint::from(v) * 2u64).to_bytes_be()
        );
    }

    #[test]
    fn eventid_sub() {
        let v = u64::MAX;
        let t = EventId::from(v) - 1;

        assert_eq!(Into::<Vec<u8>>::into(t), (v - 1).to_be_bytes());

        let b = ByteVec::from(1u64.to_be_bytes().to_vec());
        let t2 = EventId::from(v) - b;

        assert_eq!(Into::<Vec<u8>>::into(t2), (v - 1).to_be_bytes());
    }

    #[test]
    fn event_info_addresses() {
        let i0 = identity(0);
        let i01 = i0.with_subresource_id(1).unwrap();

        let s0 = EventInfo::Send {
            from: i0,
            to: i01,
            symbol: Default::default(),
            amount: Default::default(),
            memo: None,
        };
        assert_eq!(s0.addresses(), BTreeSet::from_iter(&[i0, i01]));
    }

    #[test]
    fn event_info_addresses_inner() {
        // TODO: reenable this when inner for multisig transactions work.
        // let i0 = identity(0);
        // let i1 = identity(1);
        // let i01 = i0.with_subresource_id(1).unwrap();
        // let i11 = i1.with_subresource_id(1).unwrap();
        //
        // let s0 = EventInfo::AccountMultisigSubmit {
        //     submitter: i0,
        //     account: i1,
        //     memo: None,
        //     transaction: Box::new(AccountMultisigTransaction::Send(SendArgs {
        //         from: Some(i01),
        //         to: i11,
        //         amount: Default::default(),
        //         symbol: Default::default(),
        //     })),
        //     token: None,
        //     threshold: 0,
        //     timeout: Timestamp::now(),
        //     execute_automatically: false,
        //     data: None,
        // };
        // assert_eq!(s0.addresses(), BTreeSet::from_iter(&[i0, i01, i1, i11]));
    }

    #[test]
    fn event_info_is_about() {
        let i0 = identity(0);
        let i1 = identity(1);
        let i01 = i0.with_subresource_id(1).unwrap();
        let i11 = i1.with_subresource_id(1).unwrap();

        let s0 = EventInfo::Send {
            from: i0,
            to: i01,
            symbol: Default::default(),
            amount: Default::default(),
            memo: None,
        };
        assert!(s0.is_about(&i0));
        assert!(s0.is_about(&i01));
        assert!(!s0.is_about(&i1));
        assert!(!s0.is_about(&i11));
    }

    #[test]
    fn event_info_is_about_null() {
        let i0 = identity(0);
        let i01 = i0.with_subresource_id(1).unwrap();
        let token = Vec::new().into();

        let s0 = EventInfo::AccountMultisigExecute {
            account: i01,
            token,
            executer: None,
            response: Default::default(),
        };
        assert!(s0.is_about(&i01));
        assert!(!s0.is_about(&Address::anonymous()));
    }

    #[test]
    fn event_info_symbol() {
        let i0 = identity(0);
        let i1 = identity(1);
        let i01 = i0.with_subresource_id(1).unwrap();

        let event = EventInfo::Send {
            from: i0,
            to: i01,
            symbol: i1,
            amount: Default::default(),
            memo: None,
        };
        assert_eq!(event.symbol(), Some(&i1));

        let event = EventInfo::AccountDisable { account: i0 };
        assert_eq!(event.symbol(), None);
    }

    mod event_info {
        use super::super::*;
        use many_identity::testing::identity;
        use many_types::Memo;
        use proptest::prelude::*;
        use proptest::string::string_regex;

        fn _create_event_info(memo: Memo, transaction: AccountMultisigTransaction) -> EventInfo {
            EventInfo::AccountMultisigSubmit {
                submitter: identity(0),
                account: identity(1),
                memo: Some(memo),
                transaction: Box::new(transaction),
                token: None,
                threshold: 1,
                timeout: Timestamp::now(),
                execute_automatically: false,
            }
        }

        fn _assert_serde(info: EventInfo) {
            let bytes = minicbor::to_vec(info.clone()).expect("Could not serialize");
            let decoded: EventInfo = minicbor::decode(&bytes).expect("Could not decode");

            assert_eq!(format!("{decoded:?}"), format!("{info:?}"));
        }

        proptest! {
            #[test]
            fn huge_memo(memo in string_regex("[A-Za-z0-9\\., ]{4001,5000}").unwrap()) {
                let memo: Option<Memo> = memo.try_into().ok();
                assert!(memo.is_none());
            }

            #[test]
            fn submit_send(memo in string_regex("[A-Za-z0-9\\., ]{0,4000}").unwrap(), amount: u64) {
                let memo = memo.try_into().unwrap();
                _assert_serde(
                    _create_event_info(memo, AccountMultisigTransaction::Send(module::ledger::SendArgs {
                        from: Some(identity(2)),
                        to: identity(3),
                        symbol: identity(4),
                        amount: amount.into(),
                        memo: None,
                    })),
                );
            }

            #[test]
            fn submit_submit_send(memo in string_regex("[A-Za-z0-9\\., ]{0,4000}").unwrap(), memo2 in string_regex("[A-Za-z0-9\\., ]{0,4000}").unwrap(), amount: u64) {
                let memo = memo.try_into().unwrap();
                let memo2 = memo2.try_into().unwrap();
                _assert_serde(
                    _create_event_info(memo,
                        AccountMultisigTransaction::AccountMultisigSubmit(
                            module::account::features::multisig::SubmitTransactionArgs {
                                account: identity(2),
                                memo: Some(memo2),
                                transaction: Box::new(AccountMultisigTransaction::Send(module::ledger::SendArgs {
                                    from: Some(identity(2)),
                                    to: identity(3),
                                    symbol: identity(4),
                                    amount: amount.into(),
                                    memo: None,
                                })),
                                threshold: None,
                                timeout_in_secs: None,
                                execute_automatically: None,
                            }
                        )
                    )
                );
            }

            #[test]
            fn submit_set_defaults(memo in string_regex("[A-Za-z0-9\\., ]{0,4000}").unwrap()) {
                let memo = memo.try_into().unwrap();
                _assert_serde(
                    _create_event_info(memo, AccountMultisigTransaction::AccountMultisigSetDefaults(module::account::features::multisig::SetDefaultsArgs {
                        account: identity(2),
                        threshold: Some(2),
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                    }))
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::testutils::{call_module, call_module_cbor};
    use mockall::predicate;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn info() {
        let mut mock = MockEventsModuleBackend::new();
        mock.expect_info()
            .with(predicate::eq(InfoArgs {}))
            .times(1)
            .returning(|_args| {
                Ok(InfoReturn {
                    total: 12,
                    event_types: vec![EventKind::Send],
                })
            });
        let module = super::EventsModule::new(Arc::new(Mutex::new(mock)));

        let info_returns: InfoReturn =
            minicbor::decode(&call_module(1, &module, "events.info", "null").unwrap()).unwrap();

        assert_eq!(info_returns.total, 12);
        assert_eq!(info_returns.event_types, &[EventKind::Send]);
    }

    #[test]
    fn list() {
        let data = ListArgs {
            count: Some(1),
            order: None,
            filter: None,
        };
        let mut mock = MockEventsModuleBackend::new();
        mock.expect_list()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(|_args| {
                Ok(ListReturns {
                    nb_events: 1,
                    events: vec![EventLog {
                        id: EventId::from(vec![1, 1, 1, 1]),
                        time: Timestamp::now(),
                        content: EventInfo::Send {
                            from: Address::anonymous(),
                            to: Address::anonymous(),
                            symbol: Default::default(),
                            amount: TokenAmount::from(1000u64),
                            memo: None,
                        },
                    }],
                })
            });
        let module = super::EventsModule::new(Arc::new(Mutex::new(mock)));

        let list_returns: ListReturns = minicbor::decode(
            &call_module_cbor(1, &module, "events.list", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(list_returns.nb_events, 1);
        assert_eq!(list_returns.events.len(), 1);
    }

    #[test]
    fn encode_decode() {
        let event = hex::decode(
            "a30045030000000201c11a62e7dcea02a500820\
982010301d92710582080e8acda9634f6f745be872b0e5e9b65b1d3624a3ba91c6432143\
f60e90000020245020000000103f604d92712a301d92710582080e8acda9634f6f745be8\
72b0e5e9b65b1d3624a3ba91c6432143f60e900000204a3003a00015f9001781d4163636\
f756e742077697468204944207b69647d20756e6b6e6f776e2e02a162696478326d61666\
66261686b736477617165656e6179793267786b65333268676237617134616f347774373\
4356c7366733677696a7005c11a62e7dcf5",
        )
        .unwrap();
        let event_log: EventLog = minicbor::decode(&event).unwrap();
        if let EventInfo::AccountMultisigExecute { response, .. } = event_log.content {
            assert!(response.data.unwrap_err().is_attribute_specific());
        }
    }

    #[test]
    fn encode_decode_event_filter() {
        let state_key = EventFilterAttributeSpecificIndex::MultisigTransactionState;
        let pending_state =
            EventFilterAttributeSpecific::MultisigTransactionState(VecOrSingle(vec![
                MultisigTransactionState::Pending,
            ]));
        let event_filter = EventFilter {
            account: None,
            kind: None,
            symbol: None,
            id_range: None,
            date_range: None,
            events_filter_attribute_specific: BTreeMap::from([(state_key, pending_state)]),
        };
        let encoded = minicbor::to_vec(&event_filter).unwrap();
        let decoded: EventFilter = minicbor::decode(&encoded).unwrap();

        assert_eq!(decoded, event_filter);
    }
}
