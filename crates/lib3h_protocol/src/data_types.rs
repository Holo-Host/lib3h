use crate::Address;
use std::cmp::Ordering;
use url::Url;

/// Tuple holding all the info required for identifying an Aspect.
/// (entry_address, aspect_address)
pub type AspectKey = (Address, Address);

//--------------------------------------------------------------------------------------------------
// Entry (Semi-opaque Holochain entry type)
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct EntryAspectData {
    pub aspect_address: Address,
    pub type_hint: String,
    #[serde(with = "base64")]
    pub aspect: Vec<u8>,
    pub publish_ts: u64,
}
impl Ord for EntryAspectData {
    fn cmp(&self, other: &Self) -> Ordering {
        self.aspect_address.cmp(&other.aspect_address)
    }
}
impl PartialOrd for EntryAspectData {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct EntryData {
    pub entry_address: Address,
    pub aspect_list: Vec<EntryAspectData>,
}

impl EntryData {
    /// get an EntryAspectData from an EntryData
    pub fn get(&self, aspect_address: &Address) -> Option<EntryAspectData> {
        for aspect in self.aspect_list.iter() {
            if aspect.aspect_address == *aspect_address {
                return Some(aspect.clone());
            }
        }
        None
    }

    /// Return true if we added new content from other
    pub fn merge(&mut self, other: &EntryData) -> bool {
        // Must be same entry address
        if self.entry_address != other.entry_address {
            return false;
        }
        // Get all new aspects
        let mut to_append = Vec::new();
        for aspect in other.aspect_list.iter() {
            if self
                .aspect_list
                .iter()
                .any(|a| a.aspect_address == aspect.aspect_address)
            {
                continue;
            }
            to_append.push(aspect.clone());
        }
        // append new aspects
        if to_append.len() == 0 {
            return false;
        }
        self.aspect_list.append(&mut to_append);
        true
    }
}

//--------------------------------------------------------------------------------------------------
// Generic responses
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GenericResultData {
    pub request_id: String,
    pub space_address: Address,
    pub to_agent_id: Address,
    #[serde(with = "base64")]
    pub result_info: Vec<u8>,
}

//--------------------------------------------------------------------------------------------------
// Connection
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ConnectData {
    /// Identifier of this request
    pub request_id: String,
    /// A transport address to connect to.
    /// We should find peers at that address.
    /// Ex:
    ///  - `wss://192.168.0.102:58081/`
    ///  - `holorelay://x.x.x.x`
    #[serde(with = "url_serde")]
    pub peer_uri: Url,
    /// Specify to which network to connect to.
    /// Empty string for 'any'
    pub network_id: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ConnectedData {
    /// Identifier of the `Connect` request we are responding to
    pub request_id: String,
    /// The first uri we are connected to
    #[serde(with = "url_serde")]
    pub uri: Url,
    // TODO #172 - Add network_id? Or let local client figure it out with the request_id?
    // TODO #178 - Add some info on network state
    // pub peer_count: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DisconnectedData {
    /// Specify to which network to connect to.
    /// Empty string for 'all'
    pub network_id: String,
}

//--------------------------------------------------------------------------------------------------
// Space tracking
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SpaceData {
    /// Identifier of this request
    pub request_id: String,
    pub space_address: Address,
    pub agent_id: Address,
}

//--------------------------------------------------------------------------------------------------
// Direct Messaging
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DirectMessageData {
    pub space_address: Address,
    pub request_id: String,
    pub to_agent_id: Address,
    pub from_agent_id: Address,
    #[serde(with = "base64")]
    pub content: Vec<u8>,
}

//--------------------------------------------------------------------------------------------------
// Query
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QueryEntryData {
    pub space_address: Address,
    pub entry_address: Address,
    pub request_id: String,
    pub requester_agent_id: Address,
    #[serde(with = "base64")]
    pub query: Vec<u8>, // opaque query struct
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QueryEntryResultData {
    pub space_address: Address,
    pub entry_address: Address,
    pub request_id: String,
    pub requester_agent_id: Address,
    pub responder_agent_id: Address,
    #[serde(with = "base64")]
    pub query_result: Vec<u8>, // opaque query-result struct
}

//--------------------------------------------------------------------------------------------------
// Publish, Store & Drop
//--------------------------------------------------------------------------------------------------

/// Wrapped Entry message
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ProvidedEntryData {
    pub space_address: Address,
    pub provider_agent_id: Address,
    pub entry: EntryData,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct StoreEntryAspectData {
    pub request_id: String,
    pub space_address: Address,
    pub provider_agent_id: Address,
    pub entry_address: Address,
    pub entry_aspect: EntryAspectData,
}

/// Identifier of what entry (and its meta?) to drop
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DropEntryData {
    pub space_address: Address,
    pub request_id: String,
    pub entry_address: Address,
}

//--------------------------------------------------------------------------------------------------
// Gossip
//--------------------------------------------------------------------------------------------------

/// Request for Entry
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FetchEntryData {
    pub space_address: Address,
    pub entry_address: Address,
    pub request_id: String,
    pub provider_agent_id: Address,
    pub aspect_address_list: Option<Vec<Address>>, // None -> Get all, otherwise get specified aspects
}

/// DHT data response from a request
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FetchEntryResultData {
    pub space_address: Address,
    pub provider_agent_id: Address,
    pub request_id: String,
    pub entry: EntryData,
}

//--------------------------------------------------------------------------------------------------
// Lists (publish & hold)
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GetListData {
    pub space_address: Address,
    /// Request List from a specific Agent
    pub provider_agent_id: Address,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct EntryListData {
    pub space_address: Address,
    pub provider_agent_id: Address,
    pub request_id: String,
    pub address_map: std::collections::HashMap<Address, Vec<Address>>, // Aspect addresses per entry
}

// ---------- serialization helper for binary data as base 64 ---------- //

mod base64 {
    extern crate base64;
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&base64::display::Base64Display::with_config(
            bytes,
            base64::STANDARD,
        ))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <String>::deserialize(deserializer)?;
        base64::decode(&s).map_err(de::Error::custom)
    }
}
