use minicbor::encode::Write;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use strum::Display;

#[derive(Clone, Decode, Display, Debug, Encode, Eq, PartialEq)]
#[cbor(index_only)]
pub enum ComputeStatus {
    #[n(0)]
    Deployed = 0,
    #[n(1)]
    Closed,
}

#[derive(Clone, Debug, Decode, Display, Encode, Eq, PartialEq)]
#[strum(serialize_all = "PascalCase")]
#[cbor(index_only)]
pub enum ByteUnits {
    #[n(0)]
    K = 0,
    #[n(1)]
    KI,
    #[n(2)]
    M,
    #[n(3)]
    MI,
    #[n(4)]
    G,
    #[n(5)]
    GI,
    #[n(6)]
    T,
    #[n(7)]
    TI,
    #[n(8)]
    P,
    #[n(9)]
    PI,
    #[n(10)]
    E,
    #[n(11)]
    EI,
}

#[derive(Clone, Debug, Decode, Display, Encode, Eq, PartialEq)]
#[strum(serialize_all = "kebab-case")]
#[cbor(index_only)]
pub enum Region {
    #[n(0)]
    UsEast = 0,
    #[n(1)]
    UsWest,
}

#[derive(Clone, Debug, Display, Eq, PartialEq)]
pub enum ComputeListFilter {
    All,
    Status(ComputeStatus),
}

impl<C> Encode<C> for ComputeListFilter {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        match self {
            ComputeListFilter::All => {
                e.u8(0)?;
            }
            ComputeListFilter::Status(status) => {
                e.u8(1)?;
                status.encode(e, &mut ())?;
            }
        }
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for ComputeListFilter {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        match d.u8()? {
            0 => Ok(ComputeListFilter::All),
            1 => Ok(ComputeListFilter::Status(ComputeStatus::decode(
                d,
                &mut (),
            )?)),
            x => Err(decode::Error::unknown_variant(u32::from(x))),
        }
    }
}

#[derive(Clone, Debug, Decode, Encode, PartialEq)]
#[cbor(map)]
pub struct DeploymentInfo {
    #[n(0)]
    pub provider: String,

    #[n(1)]
    pub provider_info: ProviderInfo,

    #[n(2)]
    pub price: f64,
}

#[derive(Clone, Debug, Decode, Encode, PartialEq)]
#[cbor(map)]
pub struct DeploymentMeta {
    #[n(0)]
    pub status: ComputeStatus,

    #[n(1)]
    pub dseq: u64,

    #[n(2)]
    pub meta: Option<DeploymentInfo>,

    #[n(3)]
    pub image: String,
}

// Converted from Akash Go code
#[derive(Clone, Copy, Debug, Display, Eq, PartialEq, Serialize, Deserialize)]
#[strum(serialize_all = "UPPERCASE")]
pub enum ServiceProtocol {
    TCP = 0,
    UDP = 1,
}

impl<C> Encode<C> for ServiceProtocol {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.u8(match self {
            ServiceProtocol::TCP => 0,
            ServiceProtocol::UDP => 1,
        })?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for ServiceProtocol {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        Ok(match d.u8()? {
            0 => Self::TCP,
            1 => Self::UDP,
            x => return Err(decode::Error::unknown_variant(u32::from(x))),
        })
    }
}

// Converted from Akash Go code
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
#[cbor(map)]
pub struct ProviderInfo {
    #[n(0)]
    pub host: Option<String>,
    #[n(1)]
    pub port: u16,
    #[n(2)]
    pub external_port: u16,
    #[n(3)]
    pub protocol: ServiceProtocol,
}

// Converted from Akash Go code
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq, Serialize, Deserialize)]
#[cbor(map)]
pub struct ServiceStatus {
    #[n(0)]
    pub name: String,
    #[n(1)]
    pub available: i32,
    #[n(2)]
    pub total: i32,
    #[n(3)]
    pub uris: Option<Vec<String>>,

    #[n(4)]
    pub observed_generation: i64,
    #[n(5)]
    pub replicas: i32,
    #[n(6)]
    pub updated_replicas: i32,
    #[n(7)]
    pub ready_replicas: i32,
    #[n(8)]
    pub available_replicas: i32,
}

// Converted from Akash Go code
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq, Serialize, Deserialize)]
#[cbor(map)]
pub struct ForwardedPortStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[n(0)]
    pub host: Option<String>,

    #[n(1)]
    pub port: u16,

    #[serde(rename = "externalPort")]
    #[n(2)]
    pub external_port: u16,

    #[n(3)]
    pub proto: ServiceProtocol,
    #[n(4)]
    pub name: String,
}

// Converted from Akash Go code
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq, Serialize, Deserialize)]
#[cbor(map)]
pub struct LeasedIPStatus {
    #[n(0)]
    pub port: u32,
    #[n(1)]
    pub external_port: u32,
    #[n(2)]
    pub protocol: String,
    #[n(3)]
    pub ip: String,
}

// Converted from Akash Go code
#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq, Serialize, Deserialize)]
#[cbor(map)]
pub struct LeaseStatus {
    #[n(0)]
    pub services: HashMap<String, Option<Box<ServiceStatus>>>,
    #[n(1)]
    pub forwarded_ports: HashMap<String, Vec<ForwardedPortStatus>>,
    #[n(2)]
    pub ips: Option<HashMap<String, Vec<LeasedIPStatus>>>,
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventAttribute {
    pub key: String,
    pub value: String,
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub r#type: String,
    pub attributes: Vec<EventAttribute>,
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub events: Vec<LogEvent>,
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TxLog {
    pub logs: Vec<LogEntry>,
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BidState {
    Invalid = 0,
    Open,
    Active,
    Lost,
    Closed,
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BidPrice {
    pub denom: String,
    #[serde(deserialize_with = "f64_from_string")]
    pub amount: f64,
}

// Implement a function to deserialize a string into a f64
fn f64_from_string<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>().map_err(de::Error::custom)
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BidId {
    #[serde(deserialize_with = "u64_from_string")]
    pub dseq: u64,
    #[serde(deserialize_with = "u64_from_string")]
    pub gseq: u64,
    #[serde(deserialize_with = "u64_from_string")]
    pub oseq: u64,
    pub owner: String,
    pub provider: String,
}

// Implement a function to deserialize a string into a u64
fn u64_from_string<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<u64>().map_err(de::Error::custom)
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BidDetails {
    pub bid_id: BidId,
    pub price: BidPrice,
    pub state: BidState,
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bid {
    pub bid: BidDetails,
}

// Ignore the rest of the fields
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bids {
    pub bids: Vec<Bid>,
}

// Ignore the rest of the fields
#[derive(Debug, Deserialize)]
pub struct LeasesResponse {
    pub leases: Vec<LeaseInfo>,
}

// Ignore the rest of the fields
#[derive(Debug, Deserialize)]
pub struct LeaseInfo {
    pub lease: Lease,
}

// Ignore the rest of the fields
#[derive(Debug, Deserialize)]
pub struct Lease {
    pub state: String,
}
