use many_error::ManyError;
use many_types::{AttributeRelatedIndex, Memo};
use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use visual_logo::VisualTokenLogo;

pub mod visual_logo;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum ExtendedInfoKey {
    Memo = 0,
    VisualLogo = 1,
}

impl From<ExtendedInfoKey> for AttributeRelatedIndex {
    fn from(val: ExtendedInfoKey) -> Self {
        AttributeRelatedIndex::new(val as u32)
    }
}

impl TryFrom<AttributeRelatedIndex> for ExtendedInfoKey {
    type Error = ();

    fn try_from(value: AttributeRelatedIndex) -> Result<Self, Self::Error> {
        ExtendedInfoKey::try_from(&value)
    }
}

impl TryFrom<&AttributeRelatedIndex> for ExtendedInfoKey {
    type Error = ();

    fn try_from(value: &AttributeRelatedIndex) -> Result<Self, Self::Error> {
        match value.attribute {
            0 => Ok(Self::Memo),
            1 => Ok(Self::VisualLogo),
            _ => Err(()),
        }
    }
}

impl<C> Encode<C> for ExtendedInfoKey {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, ctx: &mut C) -> Result<(), Error<W::Error>> {
        e.encode_with(Into::<AttributeRelatedIndex>::into(*self), ctx)?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for ExtendedInfoKey {
    fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, decode::Error> {
        let attr: AttributeRelatedIndex = d.decode_with(ctx)?;
        attr.try_into()
            .map_err(|_| decode::Error::message("Invalid attribute."))
    }
}

#[derive(Debug, Clone)]
enum ExtendedInfo {
    Memo(Arc<Memo>),
    VisualLogo(Arc<VisualTokenLogo>),
}

impl ExtendedInfo {
    pub fn as_key(&self) -> ExtendedInfoKey {
        match self {
            ExtendedInfo::Memo(_) => ExtendedInfoKey::Memo,
            ExtendedInfo::VisualLogo(_) => ExtendedInfoKey::VisualLogo,
        }
    }

    pub(super) fn index(&self) -> AttributeRelatedIndex {
        match self {
            ExtendedInfo::Memo(_) => AttributeRelatedIndex::new(ExtendedInfoKey::Memo as u32),
            ExtendedInfo::VisualLogo(_) => {
                AttributeRelatedIndex::new(ExtendedInfoKey::VisualLogo as u32)
            }
        }
    }
}

impl PartialEq<ExtendedInfoKey> for ExtendedInfo {
    fn eq(&self, other: &ExtendedInfoKey) -> bool {
        self.as_key() == *other
    }
}

impl PartialEq for ExtendedInfo {
    fn eq(&self, other: &Self) -> bool {
        self.index().eq(&other.index())
    }
}

impl PartialOrd for ExtendedInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.index().partial_cmp(&other.index())
    }
}

impl Eq for ExtendedInfo {}

impl Ord for ExtendedInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index().cmp(&other.index())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenExtendedInfo {
    inner: BTreeMap<ExtendedInfoKey, ExtendedInfo>,
}

impl TokenExtendedInfo {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    fn insert(&mut self, value: ExtendedInfo) {
        self.inner.insert(value.as_key(), value);
    }

    pub fn contains_index(&self, index: &AttributeRelatedIndex) -> Result<bool, ManyError> {
        let key = ExtendedInfoKey::try_from(index).map_err(|_| {
            ManyError::unknown("Unable to convert AttributeRelatedIndex to ExtendedInfoKey")
        })?;
        Ok(self.inner.contains_key(&key))
    }

    pub fn remove(&mut self, index: &AttributeRelatedIndex) -> Result<(), ManyError> {
        let key = ExtendedInfoKey::try_from(index).map_err(|_| {
            ManyError::unknown("Unable to convert AttributeRelatedIndex to ExtendedInfoKey")
        })?;
        self.inner.remove(&key);
        Ok(())
    }

    pub fn retain(&mut self, indices: Vec<AttributeRelatedIndex>) -> Result<(), ManyError> {
        let keys = indices
            .into_iter()
            .map(|i| {
                ExtendedInfoKey::try_from(i)
                    .map_err(|_| ManyError::unknown("Unable to convert {i} to ExtendedInfoKey"))
            })
            .collect::<Result<BTreeSet<ExtendedInfoKey>, _>>()?;
        self.inner.retain(|&k, _| keys.contains(&k));
        Ok(())
    }

    pub fn try_with_memo(
        mut self,
        memo: impl TryInto<Memo, Error = ManyError>,
    ) -> Result<Self, ManyError> {
        self.insert(ExtendedInfo::Memo(Arc::new(memo.try_into()?)));
        Ok(self)
    }

    pub fn with_memo(mut self, memo: Memo) -> Result<Self, ManyError> {
        self.insert(ExtendedInfo::Memo(Arc::new(memo)));
        Ok(self)
    }

    pub fn with_visual_logo(mut self, logo: VisualTokenLogo) -> Result<Self, ManyError> {
        self.insert(ExtendedInfo::VisualLogo(Arc::new(logo)));
        Ok(self)
    }

    pub fn memo(&self) -> Option<&Memo> {
        self.inner
            .get(&ExtendedInfoKey::Memo)
            .and_then(|e| match e {
                ExtendedInfo::Memo(m) => Some(m.as_ref()),
                _ => None,
            })
    }
    pub fn memo_mut(&mut self) -> Option<&mut Memo> {
        self.inner
            .get_mut(&ExtendedInfoKey::Memo)
            .and_then(|e| match e {
                ExtendedInfo::Memo(m) => Some(Arc::make_mut(m)),
                _ => None,
            })
    }

    pub fn visual_logo(&self) -> Option<&VisualTokenLogo> {
        self.inner
            .get(&ExtendedInfoKey::VisualLogo)
            .and_then(|e| match e {
                ExtendedInfo::VisualLogo(m) => Some(m.as_ref()),
                _ => None,
            })
    }
    pub fn visual_logo_mut(&mut self) -> Option<&mut VisualTokenLogo> {
        self.inner
            .get_mut(&ExtendedInfoKey::VisualLogo)
            .and_then(|e| match e {
                ExtendedInfo::VisualLogo(m) => Some(Arc::make_mut(m)),
                _ => None,
            })
    }
}

impl Default for TokenExtendedInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> Encode<C> for TokenExtendedInfo {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, ctx: &mut C) -> Result<(), Error<W::Error>> {
        e.map(self.inner.len() as u64)?;
        for (k, v) in self.inner.iter() {
            e.encode_with(k, ctx)?;

            match v {
                ExtendedInfo::Memo(m) => {
                    e.encode_with(m.as_ref(), ctx)?;
                }
                ExtendedInfo::VisualLogo(v) => {
                    e.encode_with(v.as_ref(), ctx)?;
                }
            }
        }
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for TokenExtendedInfo {
    fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, decode::Error> {
        let l = d.map().and_then(|l| {
            l.ok_or_else(|| decode::Error::message("Indefinite length map unsupported."))
        })?;

        let mut inner = BTreeMap::new();
        for _ in 0..l {
            let key: ExtendedInfoKey = d.decode_with(ctx)?;
            match key {
                ExtendedInfoKey::Memo => {
                    let memo: Memo = d.decode_with(ctx)?;
                    inner.insert(key, ExtendedInfo::Memo(Arc::new(memo)));
                }
                ExtendedInfoKey::VisualLogo => {
                    let visual_logo: VisualTokenLogo = d.decode_with(ctx)?;
                    inner.insert(key, ExtendedInfo::VisualLogo(Arc::new(visual_logo)));
                }
            }
        }

        Ok(Self { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_extended_info() {
        let mut logos = VisualTokenLogo::default();
        logos.unicode_front('∑');
        logos.image_back("foo", vec![2u8; 10]);

        let ext_info = TokenExtendedInfo::default()
            .try_with_memo("Foobar".to_string())
            .unwrap()
            .with_visual_logo(logos)
            .unwrap();

        let enc = minicbor::to_vec(&ext_info).unwrap();
        let res: TokenExtendedInfo = minicbor::decode(&enc).unwrap();

        assert_eq!(res, ext_info);
    }

    #[test]
    fn get() {
        let mut logos = VisualTokenLogo::default();
        logos.unicode_front('∑');
        logos.image_back("foo", vec![2u8; 10]);

        let ext_info = TokenExtendedInfo::default()
            .try_with_memo("Foobar".to_string())
            .unwrap()
            .with_visual_logo(logos.clone())
            .unwrap();
        assert!(ext_info.memo().is_some());
        assert_eq!(
            ext_info.memo().unwrap(),
            &Memo::try_from("Foobar".to_string()).unwrap()
        );
        assert!(ext_info.visual_logo().is_some());
        assert_eq!(ext_info.visual_logo().unwrap(), &logos);

        let ext_info = TokenExtendedInfo::default()
            .try_with_memo("Foobar".to_string())
            .unwrap();
        assert!(ext_info.memo().is_some());
        assert_eq!(
            ext_info.memo().unwrap(),
            &Memo::try_from("Foobar".to_string()).unwrap()
        );
        assert!(ext_info.visual_logo().is_none());

        let ext_info = TokenExtendedInfo::default()
            .with_visual_logo(logos.clone())
            .unwrap();
        assert!(ext_info.memo().is_none());
        assert!(ext_info.visual_logo().is_some());
        assert_eq!(ext_info.visual_logo().unwrap(), &logos);
    }

    #[test]
    fn get_mut() {
        let mut logos = VisualTokenLogo::default();
        logos.unicode_front('∑');
        logos.image_back("foo", vec![2u8; 10]);

        let mut ext_info = TokenExtendedInfo::default()
            .try_with_memo("Foobar".to_string())
            .unwrap()
            .with_visual_logo(logos.clone())
            .unwrap();
        assert!(ext_info.memo_mut().is_some());
        assert_eq!(
            ext_info.memo_mut().unwrap(),
            &Memo::try_from("Foobar".to_string()).unwrap()
        );
        assert!(ext_info.visual_logo_mut().is_some());
        assert_eq!(ext_info.visual_logo_mut().unwrap(), &logos);

        let mut ext_info = TokenExtendedInfo::default()
            .try_with_memo("Foobar".to_string())
            .unwrap();
        assert!(ext_info.memo_mut().is_some());
        assert_eq!(
            ext_info.memo_mut().unwrap(),
            &Memo::try_from("Foobar".to_string()).unwrap()
        );
        assert!(ext_info.visual_logo_mut().is_none());

        let mut ext_info = TokenExtendedInfo::default()
            .with_visual_logo(logos.clone())
            .unwrap();
        assert!(ext_info.memo_mut().is_none());
        assert!(ext_info.visual_logo_mut().is_some());
        assert_eq!(ext_info.visual_logo_mut().unwrap(), &logos);
    }
}
