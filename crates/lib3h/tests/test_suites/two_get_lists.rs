use crate::{
    node_mock::NodeMock,
    test_suites::two_basic::{request_entry_1, TwoNodesTestFn},
    utils::constants::*,
};
use lib3h_protocol::{data_types::*, protocol_server::Lib3hServerProtocol};
use rmp_serde::Deserializer;
use serde::Deserialize;

lazy_static! {
    pub static ref TWO_NODES_GET_LISTS_TEST_FNS: Vec<(TwoNodesTestFn, bool)> =
        vec![(publish_entry_list_test, true),];
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

/// Return some data in publish_list request
pub fn publish_entry_list_test(alex: &mut NodeMock, billy: &mut NodeMock) {
    // author an entry without publishing it
    alex.author_entry(&ENTRY_ADDRESS_1, vec![ASPECT_CONTENT_1.clone()], false)
        .unwrap();
    // Reply to the publish_list request received from network module
    alex.reply_to_first_HandleGetAuthoringEntryList();
    let (did_work, srv_msg_list) = alex.process().unwrap();
    assert!(did_work);

    // Should receive a HandleFetchEntry request from network module after receiving list
    assert_eq!(srv_msg_list.len(), 1);
    // extract msg data
    let fetch_data = unwrap_to!(srv_msg_list[0] => Lib3hServerProtocol::HandleFetchEntry);
    // Respond
    alex.reply_to_HandleFetchEntry(&fetch_data)
        .expect("Reply to HandleFetchEntry should work");
    let (did_work, srv_msg_list) = alex.process().unwrap();
    assert!(did_work);

    // Billy should receive storeAspect
    let (did_work, srv_msg_list) = billy.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 1, "{:?}", srv_msg_list);

    // Billy asks for that entry
    request_entry_1(billy);
}