#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate unwrap_to;
extern crate backtrace;
extern crate lib3h;
extern crate lib3h_protocol;

use lib3h::{
    dht::{dht_trait::Dht, mirror_dht::MirrorDht},
    engine::{RealEngine, RealEngineConfig},
    transport::{memory_mock::transport_memory::TransportMemory, transport_trait::Transport},
    transport_wss::TransportWss,
};
use lib3h_crypto_api::{FakeCryptoSystem, InsecureBuffer};
use lib3h_protocol::{
    data_types::*, network_engine::NetworkEngine, protocol_client::Lib3hClientProtocol,
    protocol_server::Lib3hServerProtocol,
};
use url::Url;

use lib3h_protocol::Address;
lazy_static! {
    pub static ref NETWORK_A_ID: String = "net_A".to_string();
    pub static ref ALEX_AGENT_ID: Address = "alex".to_string().into_bytes();
    pub static ref BILLY_AGENT_ID: Address = "billy".to_string().into_bytes();
    pub static ref SPACE_ADDRESS_A: Address = "SPACE_A".to_string().into_bytes();
    pub static ref SPACE_ADDRESS_B: Address = "SPACE_B".to_string().into_bytes();
}
//--------------------------------------------------------------------------------------------------
// Test suites
//--------------------------------------------------------------------------------------------------

type TwoEnginesTestFn = fn(alex: &mut Box<dyn NetworkEngine>, billy: &mut Box<dyn NetworkEngine>);

lazy_static! {
    pub static ref TWO_ENGINES_BASIC_TEST_FNS: Vec<(TwoEnginesTestFn, bool)> = vec![
        (setup_only, true),
        (basic_two_send_message, true),
        (basic_two_join_first, false),
    ];
}

//--------------------------------------------------------------------------------------------------
// Logging
//--------------------------------------------------------------------------------------------------

// for this to actually show log entries you also have to run the tests like this:
// RUST_LOG=lib3h=debug cargo test -- --nocapture
fn enable_logging_for_test(enable: bool) {
    std::env::set_var("RUST_LOG", "trace");
    let _ = env_logger::builder()
        .default_format_timestamp(false)
        .default_format_module_path(false)
        .is_test(enable)
        .try_init();
}

//--------------------------------------------------------------------------------------------------
// Engine Setup
//--------------------------------------------------------------------------------------------------

fn basic_setup_mock(
    name: &str,
) -> RealEngine<TransportMemory, MirrorDht, InsecureBuffer, FakeCryptoSystem> {
    let config = RealEngineConfig {
        socket_type: "mem".into(),
        bootstrap_nodes: vec![],
        work_dir: String::new(),
        log_level: 'd',
        bind_url: Url::parse(format!("mem://{}", name).as_str()).unwrap(),
        dht_custom_config: vec![],
    };
    let engine = RealEngine::new_mock(config, name.into(), MirrorDht::new_with_config).unwrap();
    let p2p_binding = engine.advertise();
    println!(
        "basic_setup_mock(): test engine for {}, advertise: {}",
        name, p2p_binding
    );
    engine
}

fn basic_setup_wss(
) -> RealEngine<TransportWss<std::net::TcpStream>, MirrorDht, InsecureBuffer, FakeCryptoSystem> {
    let config = RealEngineConfig {
        socket_type: "ws".into(),
        bootstrap_nodes: vec![],
        work_dir: String::new(),
        log_level: 'd',
        bind_url: Url::parse("wss://127.0.0.1:64519").unwrap(),
        dht_custom_config: vec![],
    };
    let engine =
        RealEngine::new(config, "test_engine_wss".into(), MirrorDht::new_with_config).unwrap();
    let p2p_binding = engine.advertise();
    println!("test_engine advertise: {}", p2p_binding);
    engine
}

//--------------------------------------------------------------------------------------------------
// Utils
//--------------------------------------------------------------------------------------------------

fn print_two_engines_test_name(print_str: &str, test_fn: TwoEnginesTestFn) {
    print_test_name(print_str, test_fn as *mut std::os::raw::c_void);
}

/// Print name of test function
fn print_test_name(print_str: &str, test_fn: *mut std::os::raw::c_void) {
    backtrace::resolve(test_fn, |symbol| {
        let mut full_name = symbol.name().unwrap().as_str().unwrap().to_string();
        let mut test_name = full_name.split_off("engine_test::".to_string().len());
        test_name.push_str("()");
        println!("{}{}", print_str, test_name);
    });
}

//--------------------------------------------------------------------------------------------------
// Custom tests
//--------------------------------------------------------------------------------------------------

#[test]
fn basic_connect_test_mock() {
    enable_logging_for_test(true);
    // Setup
    let mut engine_a = basic_setup_mock("basic_send_test_mock_node_a");
    let engine_b = basic_setup_mock("basic_send_test_mock_node_b");
    engine_a.run().unwrap();
    engine_b.run().unwrap();
    // Get URL
    let url_b = engine_b.advertise();
    println!("url_b: {}", url_b);
    // Send Connect Command
    let connect_msg = ConnectData {
        request_id: "connect_a_1".into(),
        peer_uri: url_b.clone(),
        network_id: NETWORK_A_ID.clone(),
    };
    engine_a
        .post(Lib3hClientProtocol::Connect(connect_msg.clone()))
        .unwrap();
    println!("\nengine_a.process()...");
    let (did_work, srv_msg_list) = engine_a.process().unwrap();
    println!("engine_a: {:?}", srv_msg_list);
    assert!(did_work);
    engine_a.terminate().unwrap();
    engine_b.terminate().unwrap();
}

#[test]
fn basic_track_test_wss() {
    enable_logging_for_test(true);
    // Setup
    let mut engine = basic_setup_wss();
    basic_track_test(&mut engine);
}

#[test]
fn basic_track_test_mock() {
    enable_logging_for_test(true);
    // Setup
    let mut engine = basic_setup_mock("basic_track_test_mock");
    basic_track_test(&mut engine);
}

fn basic_track_test<T: Transport, D: Dht>(
    engine: &mut RealEngine<T, D, InsecureBuffer, FakeCryptoSystem>,
) {
    // Start
    engine.run().unwrap();

    // Test
    let mut track_space = SpaceData {
        request_id: "track_a_1".into(),
        space_address: SPACE_ADDRESS_A.clone(),
        agent_id: ALEX_AGENT_ID.clone(),
    };
    // First track should succeed
    engine
        .post(Lib3hClientProtocol::JoinSpace(track_space.clone()))
        .unwrap();
    let (did_work, srv_msg_list) = engine.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 1);
    let res_msg = unwrap_to!(srv_msg_list[0] => Lib3hServerProtocol::SuccessResult);
    assert_eq!(res_msg.request_id, "track_a_1".to_string());
    assert_eq!(res_msg.space_address, SPACE_ADDRESS_A.as_slice());
    assert_eq!(res_msg.to_agent_id, ALEX_AGENT_ID.as_slice());
    println!(
        "SuccessResult info: {}",
        std::str::from_utf8(res_msg.result_info.as_slice()).unwrap()
    );
    // Track same again, should fail
    track_space.request_id = "track_a_2".into();
    engine
        .post(Lib3hClientProtocol::JoinSpace(track_space))
        .unwrap();
    let (did_work, srv_msg_list) = engine.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 1);
    let res_msg = unwrap_to!(srv_msg_list[0] => Lib3hServerProtocol::FailureResult);
    assert_eq!(res_msg.request_id, "track_a_2".to_string());
    assert_eq!(res_msg.space_address, SPACE_ADDRESS_A.as_slice());
    assert_eq!(res_msg.to_agent_id, ALEX_AGENT_ID.as_slice());
    println!(
        "FailureResult info: {}",
        std::str::from_utf8(res_msg.result_info.as_slice()).unwrap()
    );
    // Done
    engine.terminate().unwrap();
}

#[test]
fn basic_two_nodes_mock() {
    enable_logging_for_test(true);
    // Launch tests on each setup
    for (test_fn, can_setup) in TWO_ENGINES_BASIC_TEST_FNS.iter() {
        launch_two_nodes_test_with_memory_network(*test_fn, *can_setup).unwrap();
    }
}

// Do general test with config
fn launch_two_nodes_test_with_memory_network(
    test_fn: TwoEnginesTestFn,
    can_setup: bool,
) -> Result<(), ()> {
    println!("");
    print_two_engines_test_name("IN-MEMORY TWO ENGINES TEST: ", test_fn);
    println!("=======================");

    // Setup
    let mut alex: Box<dyn NetworkEngine> = Box::new(basic_setup_mock("alex"));
    let mut billy: Box<dyn NetworkEngine> = Box::new(basic_setup_mock("billy"));
    if can_setup {
        basic_two_setup(&mut alex, &mut billy);
    }

    // Execute test
    test_fn(&mut alex, &mut billy);

    // Wrap-up test
    println!("==================");
    print_two_engines_test_name("IN-MEMORY TWO ENGINES TEST END: ", test_fn);
    // Terminate nodes
    alex.terminate().unwrap();
    billy.terminate().unwrap();

    Ok(())
}

/// Empty function that triggers the test suite
fn setup_only(_alex: &mut Box<dyn NetworkEngine>, _billy: &mut Box<dyn NetworkEngine>) {
    // n/a
}

///
fn basic_two_setup(alex: &mut Box<dyn NetworkEngine>, billy: &mut Box<dyn NetworkEngine>) {
    // Start
    alex.run().unwrap();
    billy.run().unwrap();

    // Connect Alex to Billy
    let req_connect = ConnectData {
        request_id: "connect".to_string(),
        peer_uri: billy.advertise(),
        network_id: NETWORK_A_ID.clone(),
    };
    alex.post(Lib3hClientProtocol::Connect(req_connect.clone()))
        .unwrap();
    let (did_work, srv_msg_list) = alex.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 1);
    let connected_msg = unwrap_to!(srv_msg_list[0] => Lib3hServerProtocol::Connected);
    println!("connected_msg = {:?}", connected_msg);
    assert_eq!(connected_msg.uri, req_connect.peer_uri);
    // More process: Have Billy process P2p::PeerAddress of alex
    let (_did_work, _srv_msg_list) = billy.process().unwrap();
    let (_did_work, _srv_msg_list) = alex.process().unwrap();

    // Alex joins space A
    let mut track_space = SpaceData {
        request_id: "track_a_1".into(),
        space_address: SPACE_ADDRESS_A.clone(),
        agent_id: ALEX_AGENT_ID.clone(),
    };
    alex.post(Lib3hClientProtocol::JoinSpace(track_space.clone()))
        .unwrap();
    let (_did_work, _srv_msg_list) = alex.process().unwrap();
    // More process
    let (_did_work, _srv_msg_list) = billy.process().unwrap();

    // Billy joins space A
    track_space.agent_id = BILLY_AGENT_ID.clone();
    billy
        .post(Lib3hClientProtocol::JoinSpace(track_space.clone()))
        .unwrap();
    let (_did_work, _srv_msg_list) = billy.process().unwrap();
    // More process
    let (_did_work, _srv_msg_list) = alex.process().unwrap();

    println!("DONE basic_two_setup DONE \n\n\n");
}

//
fn basic_two_send_message(alex: &mut Box<dyn NetworkEngine>, billy: &mut Box<dyn NetworkEngine>) {
    // Create message
    let req_dm = DirectMessageData {
        space_address: SPACE_ADDRESS_A.clone(),
        request_id: "dm_1".to_string(),
        to_agent_id: BILLY_AGENT_ID.clone(),
        from_agent_id: ALEX_AGENT_ID.clone(),
        content: "wah".as_bytes().to_vec(),
    };
    // Send
    alex.post(Lib3hClientProtocol::SendDirectMessage(req_dm.clone()))
        .unwrap();
    let (did_work, srv_msg_list) = alex.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 0);
    // Receive
    let (did_work, srv_msg_list) = billy.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 1);
    let msg = unwrap_to!(srv_msg_list[0] => Lib3hServerProtocol::HandleSendDirectMessage);
    assert_eq!(msg, &req_dm);
    let content = std::str::from_utf8(msg.content.as_slice()).unwrap();
    println!("HandleSendDirectMessage: {}", content);

    // Post response
    let mut res_dm = req_dm.clone();
    res_dm.to_agent_id = req_dm.from_agent_id.clone();
    res_dm.from_agent_id = req_dm.to_agent_id.clone();
    res_dm.content = format!("echo: {}", content).as_bytes().to_vec();
    billy
        .post(Lib3hClientProtocol::HandleSendDirectMessageResult(
            res_dm.clone(),
        ))
        .unwrap();
    let (did_work, srv_msg_list) = billy.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 0);
    // Receive response
    let (did_work, srv_msg_list) = alex.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 1);
    let msg = unwrap_to!(srv_msg_list[0] => Lib3hServerProtocol::SendDirectMessageResult);
    assert_eq!(msg, &res_dm);
    let content = std::str::from_utf8(msg.content.as_slice()).unwrap();
    println!("SendDirectMessageResult: {}", content);
}

//
fn basic_two_join_first(alex: &mut Box<dyn NetworkEngine>, billy: &mut Box<dyn NetworkEngine>) {
    // Start
    alex.run().unwrap();
    billy.run().unwrap();

    // Setup: Track before connecting

    // A joins space
    let mut track_space = SpaceData {
        request_id: "track_a_1".into(),
        space_address: SPACE_ADDRESS_A.clone(),
        agent_id: ALEX_AGENT_ID.clone(),
    };
    alex.post(Lib3hClientProtocol::JoinSpace(track_space.clone()))
        .unwrap();
    let (_did_work, _srv_msg_list) = alex.process().unwrap();

    // Billy joins space
    track_space.agent_id = BILLY_AGENT_ID.clone();
    billy
        .post(Lib3hClientProtocol::JoinSpace(track_space.clone()))
        .unwrap();
    let (_did_work, _srv_msg_list) = billy.process().unwrap();

    // Connect Alex to Billy
    let req_connect = ConnectData {
        request_id: "connect".to_string(),
        peer_uri: billy.advertise(),
        network_id: NETWORK_A_ID.clone(),
    };
    alex.post(Lib3hClientProtocol::Connect(req_connect.clone()))
        .unwrap();
    let (did_work, srv_msg_list) = alex.process().unwrap();
    assert!(did_work);
    assert_eq!(srv_msg_list.len(), 1);
    let connected_msg = unwrap_to!(srv_msg_list[0] => Lib3hServerProtocol::Connected);
    println!("connected_msg = {:?}", connected_msg);
    assert_eq!(connected_msg.uri, req_connect.peer_uri);
    // More process: Have Billy process P2p::PeerAddress of alex
    let (_did_work, _srv_msg_list) = billy.process().unwrap();
    let (_did_work, _srv_msg_list) = alex.process().unwrap();

    println!("DONE Setup for basic_two_multi_join() DONE \n\n\n");

    // Do Send DM test
    basic_two_send_message(alex, billy);
}
