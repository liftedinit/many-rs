use crate::message::ResponseMessage;
use crate::server::module;
use crate::types::ledger::{Symbol, TokenAmount};
use crate::types::{AttributeRelatedIndex, Timestamp};
use crate::Identity;
use minicbor::bytes::ByteVec;
use minicbor::{encode, Decode, Decoder, Encode, Encoder};
use num_bigint::BigUint;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
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

impl Encode for EventId {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        e.bytes(&self.0)?;
        Ok(())
    }
}

impl<'b> Decode<'b> for EventId {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(EventId(ByteVec::from(d.bytes()?.to_vec())))
    }
}

impl From<EventId> for Vec<u8> {
    fn from(t: EventId) -> Vec<u8> {
        t.0.to_vec()
    }
}

macro_rules! define_tx_kind {
    ( $( [ $index: literal $(, $sub: literal )* ] $name: ident { $( $idx: literal | $fname: ident : $type: ty, )* }, )* ) => {
        #[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
        #[repr(u8)]
        #[non_exhaustive]
        pub enum TransactionKind {
            $( $name ),*
        }

        impl From<TransactionKind> for AttributeRelatedIndex {
            fn from(other: TransactionKind) -> Self {
                match other {
                    $( TransactionKind :: $name => AttributeRelatedIndex::new($index) $(.with_index($sub))* ),*
                }
            }
        }

        impl From<&TransactionInfo> for TransactionKind {
            fn from(other: &TransactionInfo) -> Self {
                match other {
                    $( TransactionInfo :: $name { .. } => TransactionKind :: $name ),*
                }
            }
        }

        impl TryFrom<AttributeRelatedIndex> for TransactionKind {
            type Error = Vec<u32>;

            fn try_from(other: AttributeRelatedIndex) -> Result<Self, Vec<u32>> {
                match &other.flattened()[..] {
                    $( [ $index $(, $sub)* ] => Ok( TransactionKind :: $name ), )*
                    x => Err(x.to_vec()),
                }
            }
        }

        impl Encode for TransactionKind {
            fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
                Into::<AttributeRelatedIndex>::into(*self).encode(e)
            }
        }

        impl<'b> Decode<'b> for TransactionKind {
            fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
                TryFrom::try_from(d.decode::<AttributeRelatedIndex>()?)
                    .map_err(|_| minicbor::decode::Error::Message("Invalid attribute index"))
            }
        }
    }
}

macro_rules! define_tx_info_symbol {
    (@pick_symbol) => {};
    (@pick_symbol $name: ident symbol $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        return Some(& $name)
    };
    (@pick_symbol $name_: ident $( $tag_: ident )*, $( $name: ident $( $tag: ident )*, )* ) => {
        define_tx_info_symbol!(@pick_symbol $( $name $( $tag )*, )* )
    };

    (@inner) => {};
    (@inner $name: ident inner $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        if let Some(s) = $name .symbol() {
            return Some(s);
        }
    };
    (@inner $name_: ident $( $tag_: ident )*, $( $name: ident $( $tag: ident )*, )* ) => {
        define_tx_info_symbol!(@inner $( $name $( $tag )*, )* )
    };

    ( $( $name: ident { $( $fname: ident $( $tag: ident )* , )* } )* ) => {
        pub fn symbol(&self) -> Option<&Symbol> {
            match self {
                $( TransactionInfo :: $name {
                    $( $fname, )*
                } => {
                    // Remove warnings.
                    $( let _ = $fname; )*
                    define_tx_info_symbol!(@pick_symbol $( $fname $( $tag )*, )* );

                    // If we're here, we need to go deeper. Check if there's an inner.
                    define_tx_info_symbol!(@inner $( $fname $( $tag )*, )*);
                } )*
            }

            None
        }
    };
}

macro_rules! define_tx_info_is_about {
    (@check_id $id: ident) => {};
    (@check_id $id: ident $name: ident id $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        if $name == $id {
            return true;
        }
        define_tx_info_is_about!(@check_id $id $( $name_ $( $tag_ )*, )* )
    };
    (@check_id $id: ident $name_: ident $( $tag_: ident )*, $( $name: ident $( $tag: ident )*, )* ) => {
        define_tx_info_is_about!(@check_id $id $( $name $( $tag )*, )* )
    };

    (@inner $id: ident) => {};
    (@inner $id: ident $name: ident inner $(,)? $( $name_: ident $( $tag_: ident )*, )* ) => {
        if $name .is_about($id) {
            return true;
        }
        define_tx_info_is_about!(@inner $id $( $name_ $( $tag_ )*, )* )
    };
    (@inner $id: ident $name_: ident $( $tag_: ident )*, $( $name: ident $( $tag: ident )*, )* ) => {
        define_tx_info_is_about!(@inner $id $( $name $( $tag )*, )* )
    };

    ( $( $name: ident { $( $fname: ident $( $tag: ident )* , )* } )* ) => {
        pub fn is_about(&self, id: &Identity) -> bool {
            match self {
                $( TransactionInfo :: $name {
                    $( $fname, )*
                } => {
                    // Remove warnings.
                    $( let _ = $fname; )*
                    define_tx_info_is_about!(@check_id id $( $fname $( $tag )*, )* );

                    // Inner fields might match the identity.
                    define_tx_info_is_about!(@inner id $( $fname $( $tag )*, )* );
                } )*
            }
            false
        }
    };
}

macro_rules! define_tx_info {
    ( $( $name: ident { $( $idx: literal | $fname: ident : $type: ty $([ $( $tag: ident )* ])?, )* }, )* ) => {
        #[derive(Clone, Debug)]
        #[non_exhaustive]
        pub enum TransactionInfo {
            $( $name {
                $( $fname: $type ),*
            } ),*
        }

        impl TransactionInfo {
            define_tx_info_symbol!( $( $name { $( $fname $( $( $tag )* )?, )* } )* );
            define_tx_info_is_about!( $( $name { $( $fname $( $( $tag )* )?, )* } )* );
        }

        encode_tx_info!( $( $name { $( $idx => $fname : $type, )* }, )* );
    };
}

// This is necessary because variables must be used in repeating patterns.
macro_rules! replace_expr {
    ($_t:tt $sub:expr) => {
        $sub
    };
}

macro_rules! encode_tx_info {
    ( $( $sname: ident { $( $idx: literal => $name: ident : $type: ty, )* }, )* ) => {
        impl Encode for TransactionInfo {
            fn encode<W: encode::Write>(
                &self,
                e: &mut Encoder<W>,
            ) -> Result<(), encode::Error<W::Error>> {
                match self {
                    $(  TransactionInfo :: $sname { $( $name, )* } => {
                            e.map( 1u64 $(+ replace_expr!($idx 1u64))* )?
                                .u8(0)?.encode(TransactionKind :: $sname)?
                                $( .u8($idx)?.encode($name)? )*
                            ;
                            Ok(())
                        }, )*
                }
            }
        }

        impl<'b> Decode<'b> for TransactionInfo {
            fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
                let mut len = d.map()?.ok_or(minicbor::decode::Error::Message(
                    "Invalid transaction type.",
                ))?;

                if d.u8()? != 0 {
                    return Err(minicbor::decode::Error::Message(
                        "TransactionKind should be the first item.",
                    ));
                }
                #[allow(unreachable_patterns)]
                match d.decode::<TransactionKind>()? {
                    $(  TransactionKind :: $sname => {
                        $( let mut $name : Option< $type > = None; )*
                        // len also includes the index 0 which is treated outside this macro.
                        while len > 1 {
                            match d.u32()? {
                                $( $idx => $name = Some(d.decode()?), )*

                                x => return Err(minicbor::decode::Error::UnknownVariant(x)),
                            }
                            len -= 1;
                        }

                        $( let $name: $type = $name.ok_or(minicbor::decode::Error::MissingValue($idx, stringify!($name)))?; )*

                        Ok(TransactionInfo :: $sname {
                            $( $name, )*
                        })
                    }, )*
                    _ => Err(minicbor::decode::Error::Message("Unsupported transaction kind"))
                }
            }
        }
    }
}

macro_rules! define_multisig_tx {
    ( $( $name: ident $(: $arg: ty )?, )* ) => {
        #[derive(Clone, Debug, PartialEq)]
        #[non_exhaustive]
        pub enum AccountMultisigTransaction {
            $( $( $name($arg), )? )*
        }

        impl AccountMultisigTransaction {
            pub fn symbol(&self) -> Option<&Identity> {
                // TODO: implement this for recursively checking if inner infos
                // has a symbol defined.
                None
            }

            pub fn is_about(&self, _id: &Identity) -> bool {
                false
            }
        }

        impl Encode for AccountMultisigTransaction {
            fn encode<W: encode::Write>(
                &self,
                e: &mut Encoder<W>,
            ) -> Result<(), encode::Error<W::Error>> {
                match self {
                    $(
                    $( AccountMultisigTransaction :: $name(arg) => {
                        let _: $arg;  // We do this to remove a macro error for not using $arg.
                        e.map(2)?
                         .u8(0)?.encode(TransactionKind:: $name)?
                         .u8(1)?.encode(arg)?;
                    }, )?
                    )*
                }
                Ok(())
            }
        }

        impl<'b> Decode<'b> for AccountMultisigTransaction {
            fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
                let len = d.map()?.ok_or(minicbor::decode::Error::Message(
                    "Invalid transaction type.",
                ))?;

                if len != 2 {
                    return Err(minicbor::decode::Error::Message("Transactions must have 2 values"));
                }
                if d.u8()? != 0 {
                    return Err(minicbor::decode::Error::Message(
                        "TransactionKind should be the first item.",
                    ));
                }
                #[allow(unreachable_patterns)]
                match d.decode::<TransactionKind>()? {
                    $(
                    $(  TransactionKind :: $name => {
                        let _: $arg;  // We do this to remove a macro error for not using $arg.
                        if d.u8()? != 1 {
                            Err(minicbor::decode::Error::Message("Invalid field index"))
                        } else {
                            Ok(Self :: $name(d.decode()?))
                        }
                    }, )?
                    )*
                    _ => return Err(minicbor::decode::Error::Message("Unsupported transaction kind"))
                }
            }
        }
    }
}

macro_rules! define_tx {
    ( $( [ $index: literal $(, $sub: literal )* ] $name: ident $(($method_arg: ty))? { $( $idx: literal | $fname: ident : $type: ty $([ $($tag: ident)* ])?, )* }, )* ) => {
        define_tx_kind!( $( [ $index $(, $sub )* ] $name { $( $idx | $fname : $type, )* }, )* );
        define_tx_info!( $( $name { $( $idx | $fname : $type $([ $( $tag )* ])?, )* }, )* );

        define_multisig_tx!( $( $name $(: $method_arg)?, )*);
    }
}

// We flatten the attribute related index here, but it is unflattened when serializing.
define_tx! {
    [4, 0]      Send (module::ledger::SendArgs) {
        1     | from:                   Identity                                [ id ],
        2     | to:                     Identity                                [ id ],
        3     | symbol:                 Symbol                                  [ symbol ],
        4     | amount:                 TokenAmount,
    },
    [9, 0]      AccountCreate (module::account::CreateArgs) {
        1     | account:                Identity                                [ id ],
        2     | description:            Option<String>,
        3     | roles:                  BTreeMap<Identity, BTreeSet<module::account::Role>>,
        4     | features:               module::account::features::FeatureSet,
    },
    [9, 1]      AccountSetDescription (module::account::SetDescriptionArgs) {
        1     | account:                Identity                                [ id ],
        2     | description:            String,
    },
    [9, 2]      AccountAddRoles (module::account::AddRolesArgs) {
        1     | account:                Identity                                [ id ],
        2     | roles:                  BTreeMap<Identity, BTreeSet<module::account::Role>>,
    },
    [9, 3]      AccountRemoveRoles (module::account::RemoveRolesArgs) {
        1     | account:                Identity                                [ id ],
        2     | roles:                  BTreeMap<Identity, BTreeSet<module::account::Role>>,
    },
    [9, 4]      AccountDelete (module::account::DeleteArgs) {
        1     | account:                Identity                                [ id ],
    },
    [9, 5]      AccountAddFeatures (module::account::AddFeaturesArgs) {
        1     | account:                Identity                                [ id ],
        2     | roles:                  BTreeMap<Identity, BTreeSet<module::account::Role>>,
        3     | features:               module::account::features::FeatureSet,
    },
    [9, 1, 0]   AccountMultisigSubmit (module::account::features::multisig::SubmitTransactionArgs) {
        1     | submitter:              Identity                                [ id ],
        2     | account:                Identity                                [ id ],
        3     | memo:                   Option<String>,
        4     | transaction:            Box<AccountMultisigTransaction>         [ inner ],
        5     | token:                  Option<ByteVec>,
        6     | threshold:              u64,
        7     | timeout:                Timestamp,
        8     | execute_automatically:  bool,
        9     | data:                   Option<ByteVec>,
    },
    [9, 1, 1]   AccountMultisigApprove (module::account::features::multisig::ApproveArgs) {
        1     | account:                Identity                                [ id ],
        2     | token:                  ByteVec,
        3     | approver:               Identity                                [ id ],
    },
    [9, 1, 2]   AccountMultisigRevoke (module::account::features::multisig::RevokeArgs) {
        1     | account:                Identity                                [ id ],
        2     | token:                  ByteVec,
        3     | revoker:                Identity                                [ id ],
    },
    [9, 1, 3]   AccountMultisigExecute (module::account::features::multisig::ExecuteArgs) {
        1     | account:                Identity                                [ id ],
        2     | token:                  ByteVec,
        3     | executer:               Option<Identity>                        [ id ],
        4     | response:               ResponseMessage,
    },
    [9, 1, 4]   AccountMultisigWithdraw (module::account::features::multisig::WithdrawArgs) {
        1     | account:                Identity                                [ id ],
        2     | token:                  ByteVec,
        3     | withdrawer:             Identity                                [ id ],
    },
    [9, 1, 5]   AccountMultisigSetDefaults (module::account::features::multisig::SetDefaultsArgs) {
        1     | submitter:              Identity                                [ id ],
        2     | account:                Identity                                [ id ],
        3     | threshold:              Option<u64>,
        4     | timeout_in_secs:        Option<u64>,
        5     | execute_automatically:  Option<bool>,
    },
    [9, 1, 6]   AccountMultisigExpired {
        1     | account:                Identity                                [ id ],
        2     | token:                  ByteVec,
        3     | time:                   Timestamp,
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
    pub content: TransactionInfo,
}

impl EventLog {
    pub fn kind(&self) -> TransactionKind {
        TransactionKind::from(&self.content)
    }

    pub fn symbol(&self) -> Option<&Identity> {
        self.content.symbol()
    }

    pub fn is_about(&self, id: &Identity) -> bool {
        self.content.is_about(id)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn txid_from_bytevec() {
        let b = ByteVec::from(vec![1, 2, 3, 4, 5]);
        let t = EventId::from(b.clone());

        assert_eq!(b.as_slice(), Into::<Vec<u8>>::into(t));
    }

    #[test]
    fn txid_from_biguint() {
        let v = u64::MAX;
        let t = EventId::from(BigUint::from(v));

        assert_eq!(v.to_be_bytes(), Into::<Vec<u8>>::into(t).as_slice());
    }

    #[test]
    fn txid_from_u64() {
        let v = u64::MAX;
        let t = EventId::from(v);

        assert_eq!(v.to_be_bytes(), Into::<Vec<u8>>::into(t).as_slice());
    }

    #[test]
    fn txid_add() {
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
    fn txid_sub() {
        let v = u64::MAX;
        let t = EventId::from(v) - 1;

        assert_eq!(Into::<Vec<u8>>::into(t), (v - 1).to_be_bytes());

        let b = ByteVec::from(1u64.to_be_bytes().to_vec());
        let t2 = EventId::from(v) - b;

        assert_eq!(Into::<Vec<u8>>::into(t2), (v - 1).to_be_bytes());
    }

    #[test]
    fn tx_info_is_about() {
        let i0 = Identity::public_key_raw([0; 28]);
        let i1 = Identity::public_key_raw([1; 28]);
        let i01 = i0.with_subresource_id_unchecked(1);
        let i11 = i1.with_subresource_id_unchecked(1);

        let s0 = TransactionInfo::Send {
            from: i0,
            to: i01,
            symbol: Default::default(),
            amount: Default::default(),
        };
        assert!(s0.is_about(&i0));
        assert!(s0.is_about(&i01));
        assert!(!s0.is_about(&i1));
        assert!(!s0.is_about(&i11));
    }

    #[test]
    fn tx_info_symbol() {
        let i0 = Identity::public_key_raw([0; 28]);
        let i1 = Identity::public_key_raw([1; 28]);
        let i01 = i0.with_subresource_id_unchecked(1);

        let tx = TransactionInfo::Send {
            from: i0,
            to: i01,
            symbol: i1,
            amount: Default::default(),
        };
        assert_eq!(tx.symbol(), Some(&i1));

        let tx = TransactionInfo::AccountDelete { account: i0 };
        assert_eq!(tx.symbol(), None);
    }

    mod tx_info {
        use super::super::*;
        use proptest::prelude::*;

        fn _create_tx_info(
            memo: String,
            data: Vec<u8>,
            transaction: AccountMultisigTransaction,
        ) -> TransactionInfo {
            TransactionInfo::AccountMultisigSubmit {
                submitter: Identity::public_key_raw([0; 28]),
                account: Identity::public_key_raw([1; 28]),
                memo: Some(memo),
                transaction: Box::new(transaction),
                token: None,
                threshold: 1,
                timeout: Timestamp::now(),
                execute_automatically: false,
                data: Some(data.into()),
            }
        }

        fn _assert_serde(info: TransactionInfo) {
            let bytes = minicbor::to_vec(info.clone()).expect("Could not serialize");
            let decoded: TransactionInfo = minicbor::decode(&bytes).expect("Could not decode");

            assert_eq!(format!("{:?}", decoded), format!("{:?}", info));
        }

        proptest! {
            #[test]
            fn submit_send(memo in "\\PC*", amount: u64) {
                _assert_serde(
                    _create_tx_info(memo, vec![], AccountMultisigTransaction::Send(module::ledger::SendArgs {
                        from: Some(Identity::public_key_raw([2; 28])),
                        to: Identity::public_key_raw([3; 28]),
                        symbol: Identity::public_key_raw([4; 28]),
                        amount: amount.into(),
                    })),
                );
            }

            #[test]
            fn submit_submit_send(memo in "\\PC*", memo2 in "\\PC*", amount: u64) {
                _assert_serde(
                    _create_tx_info(memo, vec![],
                        AccountMultisigTransaction::AccountMultisigSubmit(
                            module::account::features::multisig::SubmitTransactionArgs {
                                account: Identity::public_key_raw([2; 28]),
                                memo: Some(memo2),
                                transaction: Box::new(AccountMultisigTransaction::Send(module::ledger::SendArgs {
                                    from: Some(Identity::public_key_raw([2; 28])),
                                    to: Identity::public_key_raw([3; 28]),
                                    symbol: Identity::public_key_raw([4; 28]),
                                    amount: amount.into(),
                                })),
                                threshold: None,
                                timeout_in_secs: None,
                                execute_automatically: None,
                                data: None,
                            }
                        )
                    )
                );
            }

            #[test]
            fn submit_set_defaults(memo in "\\PC*") {
                _assert_serde(
                    _create_tx_info(memo, vec![], AccountMultisigTransaction::AccountMultisigSetDefaults(module::account::features::multisig::SetDefaultsArgs {
                        account: Identity::public_key_raw([2; 28]),
                        threshold: Some(2),
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                    }))
                );
            }
        }
    }
}
