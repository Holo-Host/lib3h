use crate::Address;

/// Tuple holding all the info required for identifying a metadata.
/// (entry_address, attribute, content)
/// TODO: Figure out if we keep this
pub type MetaTuple = (Address, String, Vec<u8>);
/// (entry_address, attribute)
/// TODO: Figure out if we keep this
pub type MetaKey = (Address, String);

//--------------------------------------------------------------------------------------------------
// Generic responses
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ResultData {
    pub request_id: String,
    pub dna_address: Address,
    pub to_agent_id: Address,
    pub result_info: Vec<u8>,
}

//--------------------------------------------------------------------------------------------------
// Connection
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectData {
    /// Identifier of this request
    pub request_id: String,
    /// A transport address to connect to.
    /// We should find peers at that address.
    /// Ex:
    ///  - `wss://192.168.0.102:58081/`
    ///  - `holorelay://x.x.x.x`
    pub peer_transport: String,
    /// TODO: Add a machine Id?
    /// Specify to which network to connect to.
    /// Empty string for 'any'
    pub network_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectedData {
    /// Identifier of the `Connect` request we are responding to
    pub request_id: String,
    /// MachineId of the first peer we are connected to
    pub machine_id: Address,
    // TODO: Add network_id? Or let local client figure it out with the request_id?
    // TODO: Maybe add some info on network state?
    // pub peer_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisconnectedData {
    /// Specify to which network to connect to.
    /// Empty string for 'all'
    pub network_id: String,
}

//--------------------------------------------------------------------------------------------------
// DNA tracking
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct TrackDnaData {
    pub dna_address: Address,
    pub agent_id: Address,
}

//--------------------------------------------------------------------------------------------------
// Direct Messaging
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct DirectMessageData {
    pub dna_address: Address,
    pub request_id: String,
    pub to_agent_id: Address,
    pub from_agent_id: Address,
    pub content: Vec<u8>,
}

//--------------------------------------------------------------------------------------------------
// DHT Entry
//--------------------------------------------------------------------------------------------------

/// Entry data message
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClaimedEntryData {
    pub dna_address: Address,
    pub provider_agent_id: Address,
    pub entry_address: Address,
    pub entry_content: Vec<u8>,
}

/// Entry hodled message
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntryStoredData {
    pub dna_address: Address,
    pub provider_agent_id: Address,
    pub entry_address: Address,
    pub holder_agent_id: Address,
}

/// Request for Entry
#[derive(Debug, Clone, PartialEq)]
pub struct FetchEntryData {
    pub dna_address: Address,
    pub entry_address: Address,
    pub request_id: String,
    pub requester_agent_id: Address,
}

/// DHT data response from a request
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FetchEntryResultData {
    pub request_id: String,
    pub requester_agent_id: Address,
    pub entry: ClaimedEntryData,
}

/// Identifier of what entry (and its meta?) to drop
#[derive(Debug, Clone, PartialEq)]
pub struct DropEntryData {
    pub dna_address: Address,
    pub request_id: String,
    pub entry_address: Address,
}

//--------------------------------------------------------------------------------------------------
// DHT Meta
//--------------------------------------------------------------------------------------------------

/// DHT Meta message
#[derive(Debug, Clone, PartialEq)]
pub struct DhtMetaData {
    pub dna_address: Address,
    pub provider_agent_id: Address,
    pub entry_address: Address,
    pub attribute: String,
    pub meta_content: Vec<u8>,
}

/// Meta hodled message
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MetaStoredData {
    pub dna_address: Address,
    pub provider_agent_id: Address,
    pub entry_address: Address,
    pub holder_agent_id: Address,
    pub attribute: String,
    pub meta_address: Address,
}

/// Metadata Request from another agent
#[derive(Debug, Clone, PartialEq)]
pub struct FetchMetaData {
    pub dna_address: Address,
    pub request_id: String,
    pub requester_agent_id: Address,
    pub entry_address: Address,
    pub attribute: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchMetaResultData {
    pub request_id: String,
    pub requester_agent_id: Address,
    pub dna_address: Address,
    pub provider_agent_id: Address,
    pub entry_address: Address,
    pub attribute: String,
    pub meta_content_list: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropMetaData {
    pub dna_address: Address,
    pub request_id: String,
    pub entry_address: Address,
    pub attribute: String,
}

//--------------------------------------------------------------------------------------------------
// Lists (publish & hold)
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct GetListData {
    pub dna_address: Address,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntryListData {
    pub dna_address: Address,
    pub request_id: String,
    pub entry_address_list: Vec<Address>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetaListData {
    pub dna_address: Address,
    pub request_id: String,
    // List of meta identifiers, a pair: (entry_address, attribute, hashed_content)
    pub meta_list: Vec<MetaTuple>,
}

//--------------------------------------------------------------------------------------------------
// Refactor
//--------------------------------------------------------------------------------------------------

pub enum EntryAspectKind {
    Content, // the actual entry content
    Header,  // the header for the entry
    Meta,    // could be EntryWithHeader for links
    ValidationResult,
}

pub struct EntryAspect {
    pub kind: EntryAspectKind,
    pub publish_ts: u64,
    pub data: String, // opaque, but in core would be EntryWithHeader for both Entry and Meta
}

pub struct EntryData {
    pub aspect_list: Vec<EntryAspect>,
    pub entry_address: Address,
}
