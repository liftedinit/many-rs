use crate::types::{Percent, Timestamp};
use crate::Identity;
use minicbor::bytes::ByteVec;
use minicbor::data::{Tag, Type};
use minicbor::{encode, Decode, Decoder, Encode, Encoder};
use num_bigint::{BigInt, BigUint};
use num_traits::Num;
use serde::Deserialize;
use std::fmt::{Display, Formatter};
use std::ops::Shr;

/// A Symbol is represented by a non-anonymous identity.
pub type Symbol = Identity;

/// Transaction fees.
#[derive(Default, Clone, Encode, Decode)]
pub struct TransactionFee {
    #[n(0)]
    pub fixed: Option<TokenAmount>,
    #[n(1)]
    pub percent: Option<Percent>,
}

impl TransactionFee {
    /// Calculates the actual fees of a transaction. The returned amount is the
    /// fees calculated, and not (amount + fees).
    /// ```
    /// use many::types::ledger::{TokenAmount, TransactionFee};
    /// use many::types::Percent;
    /// let fees = TransactionFee {
    ///   fixed: Some(1000u64.into()),
    ///   percent: Some(Percent::new(0, 0x800000)),
    /// };
    /// let amount = TokenAmount::from(5000000u64);
    ///
    /// assert_eq!(fees.calculate_fees(&amount) + amount, 5_010_765u64);
    /// ```
    pub fn calculate_fees(&self, amount: &TokenAmount) -> TokenAmount {
        let mut fees = self.fixed.clone().unwrap_or_default();
        fees += if let Some(ref p) = self.percent {
            amount.clone() * *p
        } else {
            TokenAmount::zero()
        };
        fees
    }
}

type TokenAmountStorage = BigUint;

#[repr(transparent)]
#[derive(Debug, Default, Hash, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct TokenAmount(TokenAmountStorage);

impl TokenAmount {
    pub fn zero() -> Self {
        Self(0u8.into())
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0u8.into()
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_bytes_be()
    }
}

impl std::ops::Mul<Percent> for TokenAmount {
    type Output = TokenAmount;

    fn mul(self, rhs: Percent) -> Self::Output {
        let mut n: BigUint = self.0;
        n = (n * rhs.0.to_bits()).shr(32);
        n.into()
    }
}

macro_rules! op_impl {
    ( $( $t: ty )* ) => {
        $(
            impl std::ops::Add<$t> for TokenAmount {
                type Output = TokenAmount;
                fn add(self, rhs: $t) -> Self::Output { Self(self.0 + Into::<BigUint>::into(rhs)) }
            }

            impl std::ops::Sub<$t> for TokenAmount {
                type Output = TokenAmount;
                fn sub(self, rhs: $t) -> Self::Output { Self(self.0 - Into::<BigUint>::into(rhs)) }
            }

            impl std::ops::Mul<$t> for TokenAmount {
                type Output = TokenAmount;
                fn mul(self, rhs: $t) -> Self::Output { Self(self.0 * Into::<BigUint>::into(rhs)) }
            }

            impl std::ops::AddAssign<$t> for TokenAmount {
                fn add_assign(&mut self, rhs: $t) { self.0 += Into::<BigUint>::into(rhs); }
            }

            impl std::ops::SubAssign<$t> for TokenAmount {
                fn sub_assign(&mut self, rhs: $t) { self.0 -= Into::<BigUint>::into(rhs); }
            }

            impl std::ops::MulAssign<$t> for TokenAmount {
                fn mul_assign(&mut self, rhs: $t) { self.0 *= Into::<BigUint>::into(rhs); }
            }
        )*
    }
}

macro_rules! from_impl {
    ( $( $t: ty )* ) => {
        $(
        impl From<$t> for TokenAmount {
            fn from(v: $t) -> Self {
                Self(v.into())
            }
        }
        )*
    };
}

macro_rules! eq_impl {
    ( $( $t: ty )* ) => {
        $(
            impl PartialEq<$t> for TokenAmount {
                fn eq(&self, other: &$t) -> bool { self.0 == (*other).into() }
            }
        )*
    };
}

op_impl!(u8 u16 u32 u64 u128 TokenAmount BigUint);
from_impl!(u8 u16 u32 u64 u128 BigUint);
eq_impl!(u8 u16 u32 u64 u128);

impl From<TokenAmount> for BigUint {
    fn from(t: TokenAmount) -> BigUint {
        t.0
    }
}

impl PartialEq<BigUint> for TokenAmount {
    fn eq(&self, other: &BigUint) -> bool {
        self.0 == *other
    }
}

impl From<Vec<u8>> for TokenAmount {
    fn from(v: Vec<u8>) -> Self {
        TokenAmount(BigUint::from_bytes_be(v.as_slice()))
    }
}

impl From<TokenAmount> for Vec<u8> {
    fn from(t: TokenAmount) -> Vec<u8> {
        t.0.to_bytes_be()
    }
}

impl TryFrom<num_bigint::BigInt> for TokenAmount {
    type Error = ();

    fn try_from(value: BigInt) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into().map_err(|_| ())?))
    }
}

impl Display for TokenAmount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Encode for TokenAmount {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        use num_traits::cast::ToPrimitive;

        // Encode efficiently.
        if let Some(amount) = self.0.to_u64() {
            e.u64(amount)?;
        } else {
            e.tag(Tag::PosBignum)?.bytes(&self.0.to_bytes_be())?;
        }
        Ok(())
    }
}

impl<'b> Decode<'b> for TokenAmount {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        // Decode either.
        match d.datatype()? {
            Type::Tag => {
                if d.tag()? != Tag::PosBignum {
                    return Err(minicbor::decode::Error::Message("Invalid tag."));
                }

                let bytes = d.bytes()?.to_vec();
                Ok(TokenAmount::from(bytes))
            }
            _ => Ok(d.u64()?.into()),
        }
    }
}

impl<'de> Deserialize<'de> for TokenAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = TokenAmount;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("amount in number or string")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TokenAmount(v.into()))
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let storage = TokenAmountStorage::from_str_radix(v, 10).map_err(E::custom)?;
                Ok(TokenAmount(storage))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
#[repr(transparent)]
pub struct TransactionId(pub ByteVec);

impl From<ByteVec> for TransactionId {
    fn from(t: ByteVec) -> TransactionId {
        TransactionId(t)
    }
}

impl From<Vec<u8>> for TransactionId {
    fn from(t: Vec<u8>) -> TransactionId {
        TransactionId(ByteVec::from(t))
    }
}

impl From<u64> for TransactionId {
    fn from(v: u64) -> TransactionId {
        TransactionId(ByteVec::from(v.to_be_bytes().to_vec()))
    }
}

impl From<BigUint> for TransactionId {
    fn from(b: BigUint) -> TransactionId {
        TransactionId(ByteVec::from(b.to_bytes_be()))
    }
}

impl std::ops::Add<ByteVec> for TransactionId {
    type Output = TransactionId;

    fn add(self, rhs: ByteVec) -> Self::Output {
        (BigUint::from_bytes_be(&self.0) + BigUint::from_bytes_be(&rhs)).into()
    }
}

impl std::ops::Add<u32> for TransactionId {
    type Output = TransactionId;

    fn add(self, rhs: u32) -> Self::Output {
        (BigUint::from_bytes_be(&self.0) + rhs).into()
    }
}

impl std::ops::AddAssign<u32> for TransactionId {
    fn add_assign(&mut self, other: u32) {
        *self = self.clone() + other;
    }
}

impl std::ops::Sub<ByteVec> for TransactionId {
    type Output = TransactionId;

    fn sub(self, rhs: ByteVec) -> Self::Output {
        (BigUint::from_bytes_be(&self.0) - BigUint::from_bytes_be(&rhs)).into()
    }
}

impl std::ops::Sub<u32> for TransactionId {
    type Output = TransactionId;

    fn sub(self, rhs: u32) -> Self::Output {
        (BigUint::from_bytes_be(&self.0) - rhs).into()
    }
}

impl Encode for TransactionId {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        e.bytes(&self.0)?;
        Ok(())
    }
}

impl<'b> Decode<'b> for TransactionId {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(TransactionId(ByteVec::from(d.bytes()?.to_vec())))
    }
}

impl From<TransactionId> for Vec<u8> {
    fn from(t: TransactionId) -> Vec<u8> {
        t.0.to_vec()
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum TransactionKind {
    Send,
    MultisigSubmit,
    MultisigApprove,
    MultisigRevoke,
    MultisigExecute,
    MultisigWithdraw,
    MultisigSetDefaults,
}

impl Encode for TransactionKind {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        match self {
            TransactionKind::Send => e.u8(0),
            TransactionKind::MultisigSubmit => e.array(3)?.u8(9)?.u8(1)?.u8(0),
            TransactionKind::MultisigApprove => e.array(3)?.u8(9)?.u8(1)?.u8(1),
            TransactionKind::MultisigRevoke => e.array(3)?.u8(9)?.u8(1)?.u8(2),
            TransactionKind::MultisigExecute => e.array(3)?.u8(9)?.u8(1)?.u8(3),
            TransactionKind::MultisigWithdraw => e.array(3)?.u8(9)?.u8(1)?.u8(4),
            TransactionKind::MultisigSetDefaults => e.array(3)?.u8(9)?.u8(1)?.u8(5),
        }?;
        Ok(())
    }
}

impl<'b> Decode<'b> for TransactionKind {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        match d.datatype()? {
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => match d.u32()? {
                0 => Ok(Self::Send),
                x => Err(minicbor::decode::Error::UnknownVariant(x)),
            },
            Type::Array | Type::ArrayIndef => match d.decode::<Vec<u32>>()?.as_slice() {
                [9, 1, 0] => Ok(Self::MultisigSubmit),
                [9, 1, 1] => Ok(Self::MultisigApprove),
                [9, 1, 2] => Ok(Self::MultisigRevoke),
                [9, 1, 3] => Ok(Self::MultisigExecute),
                [9, 1, 4] => Ok(Self::MultisigWithdraw),
                [9, 1, 5] => Ok(Self::MultisigSetDefaults),
                _ => Err(minicbor::decode::Error::Message("Invalid variant")),
            },
            x => Err(minicbor::decode::Error::TypeMismatch(
                x,
                "An array or integer.",
            )),
        }
    }
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct Transaction {
    #[n(0)]
    pub id: TransactionId,

    #[n(1)]
    pub time: Timestamp,

    #[n(2)]
    pub content: TransactionInfo,
}

impl Transaction {
    pub fn kind(&self) -> TransactionKind {
        match self.content {
            TransactionInfo::Send { .. } => TransactionKind::Send,
            TransactionInfo::MultisigSubmit { .. } => TransactionKind::MultisigSubmit,
            TransactionInfo::MultisigApprove { .. } => TransactionKind::MultisigApprove,
            TransactionInfo::MultisigRevoke { .. } => TransactionKind::MultisigRevoke,
            TransactionInfo::MultisigExecute { .. } => TransactionKind::MultisigExecute,
            TransactionInfo::MultisigWithdraw { .. } => TransactionKind::MultisigWithdraw,
            TransactionInfo::MultisigSetDefaults { .. } => TransactionKind::MultisigSetDefaults,
        }
    }

    pub fn symbol(&self) -> Option<&Identity> {
        match &self.content {
            TransactionInfo::Send { symbol, .. } => Some(symbol),
            TransactionInfo::MultisigSubmit { .. } => None,
            TransactionInfo::MultisigApprove { .. } => None,
            TransactionInfo::MultisigRevoke { .. } => None,
            TransactionInfo::MultisigExecute { .. } => None,
            TransactionInfo::MultisigWithdraw { .. } => None,
            TransactionInfo::MultisigSetDefaults { .. } => None,
        }
    }

    pub fn is_about(&self, id: &Identity) -> bool {
        match &self.content {
            TransactionInfo::Send { from, to, .. } => id == from || id == to,
            TransactionInfo::MultisigSubmit {
                account, submitter, ..
            } => id == submitter || id == account,
            TransactionInfo::MultisigApprove {
                account, approver, ..
            } => id == approver || id == account,
            TransactionInfo::MultisigRevoke {
                account, revoker, ..
            } => id == revoker || id == account,
            TransactionInfo::MultisigExecute {
                account, executer, ..
            } => Some(id) == executer.as_ref() || id == account,
            TransactionInfo::MultisigWithdraw {
                account,
                withdrawer,
                ..
            } => id == withdrawer || id == account,
            TransactionInfo::MultisigSetDefaults {
                account, submitter, ..
            } => submitter.map(|ref e| id == e).unwrap_or(false) || id == account,
        }
    }
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
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum TransactionInfo {
    Send {
        from: Identity,
        to: Identity,
        symbol: Symbol,
        amount: TokenAmount,
    },

    MultisigSubmit {
        submitter: Identity,
        account: Identity,
        memo: Option<String>,
        transaction: Box<TransactionInfo>,
        token: Option<ByteVec>,
        threshold: u64,
        timeout: Timestamp,
        execute_automatically: bool,
        data: Option<ByteVec>,
    },

    MultisigApprove {
        account: Identity,
        token: ByteVec,
        approver: Identity,
    },

    MultisigRevoke {
        account: Identity,
        token: ByteVec,
        revoker: Identity,
    },

    MultisigExecute {
        account: Identity,
        token: ByteVec,
        executer: Option<Identity>,
    },

    MultisigWithdraw {
        account: Identity,
        token: ByteVec,
        withdrawer: Identity,
    },

    MultisigSetDefaults {
        submitter: Option<Identity>,
        account: Identity,
        threshold: Option<u64>,
        timeout_in_secs: Option<u64>,
        execute_automatically: Option<bool>,
    },
}

encode_tx_info! {
    Send {
        1 => from: Identity,
        2 => to: Identity,
        3 => symbol: Symbol,
        4 => amount: TokenAmount,
    },
    MultisigSubmit {
        1 => submitter: Identity,
        2 => account: Identity,
        3 => memo: Option<String>,
        4 => transaction: Box<TransactionInfo>,
        5 => token: Option<ByteVec>,
        6 => threshold: u64,
        7 => timeout: Timestamp,
        8 => execute_automatically: bool,
        9 => data: Option<ByteVec>,
    },
    MultisigApprove {
        1 => account: Identity,
        2 => token: ByteVec,
        3 => approver: Identity,
    },
    MultisigRevoke {
        1 => account: Identity,
        2 => token: ByteVec,
        3 => revoker: Identity,
    },
    MultisigExecute {
        1 => account: Identity,
        2 => token: ByteVec,
        3 => executer: Option<Identity>,
    },
    MultisigWithdraw {
        1 => account: Identity,
        2 => token: ByteVec,
        3 => withdrawer: Identity,
    },
    MultisigSetDefaults {
        1 => submitter: Option<Identity>,
        2 => account: Identity,
        3 => threshold: Option<u64>,
        4 => timeout_in_secs: Option<u64>,
        5 => execute_automatically: Option<bool>,
    },
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn txid_from_bytevec() {
        let b = ByteVec::from(vec![1, 2, 3, 4, 5]);
        let t = TransactionId::from(b.clone());

        assert_eq!(b.as_slice(), Into::<Vec<u8>>::into(t));
    }

    #[test]
    fn txid_from_biguint() {
        let v = u64::MAX;
        let t = TransactionId::from(BigUint::from(v));

        assert_eq!(v.to_be_bytes(), Into::<Vec<u8>>::into(t).as_slice());
    }

    #[test]
    fn txid_from_u64() {
        let v = u64::MAX;
        let t = TransactionId::from(v);

        assert_eq!(v.to_be_bytes(), Into::<Vec<u8>>::into(t).as_slice());
    }

    #[test]
    fn txid_add() {
        let v = u64::MAX;
        let mut t = TransactionId::from(v) + 1;

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
        let t2 = TransactionId::from(v) + b;

        assert_eq!(
            Into::<Vec<u8>>::into(t2),
            (BigUint::from(v) * 2u64).to_bytes_be()
        );
    }

    #[test]
    fn txid_sub() {
        let v = u64::MAX;
        let t = TransactionId::from(v) - 1;

        assert_eq!(Into::<Vec<u8>>::into(t), (v - 1).to_be_bytes());

        let b = ByteVec::from(1u64.to_be_bytes().to_vec());
        let t2 = TransactionId::from(v) - b;

        assert_eq!(Into::<Vec<u8>>::into(t2), (v - 1).to_be_bytes());
    }

    mod tx_info {
        use super::super::*;
        use proptest::prelude::*;

        fn _create_tx_info(
            memo: String,
            data: Vec<u8>,
            transaction: TransactionInfo,
        ) -> TransactionInfo {
            TransactionInfo::MultisigSubmit {
                submitter: Identity::public_key_raw_([0; 28]),
                account: Identity::public_key_raw_([1; 28]),
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
            eprintln!("info: {:?}", info);
            let bytes = minicbor::to_vec(info.clone()).expect("Could not serialize");
            eprintln!("bytes: {}", hex::encode(&bytes));
            let decoded: TransactionInfo = minicbor::decode(&bytes).expect("Could not decode");

            assert_eq!(format!("{:?}", decoded), format!("{:?}", info));
        }

        proptest! {
            #[test]
            fn submit_send(memo in "\\PC*", amount: u64) {
                _assert_serde(
                    _create_tx_info(memo, vec![], TransactionInfo::Send {
                        from: Identity::public_key_raw_([2; 28]),
                        to: Identity::public_key_raw_([3; 28]),
                        symbol: Identity::public_key_raw_([4; 28]),
                        amount: amount.into(),
                    })
                );
            }

            #[test]
            fn submit_submit_send(memo in "\\PC*", memo2 in "\\PC*", amount: u64) {
                _assert_serde(
                    _create_tx_info(memo, vec![],
                        _create_tx_info(memo2, vec![], TransactionInfo::Send {
                            from: Identity::public_key_raw_([2; 28]),
                            to: Identity::public_key_raw_([3; 28]),
                            symbol: Identity::public_key_raw_([4; 28]),
                            amount: amount.into(),
                        }),
                    )
                );
            }

            #[test]
            fn submit_set_defaults(memo in "\\PC*") {
                _assert_serde(
                    _create_tx_info(memo, vec![], TransactionInfo::MultisigSetDefaults {
                        account: Identity::public_key_raw_([2; 28]),
                        submitter: Some(Identity::public_key_raw_([3; 28])),
                        threshold: Some(2),
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                    })
                );
            }
        }
    }
}
