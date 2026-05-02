use std::collections::HashSet;
use std::sync::Arc;
use xancode::Codec;
use xanq::address::{Address, DeliveryMode};
use xanq::client::Client;
use xanq::consumer::Consumer;
use xanq::server::Server;

#[derive(Codec, Clone, Debug, PartialEq, Eq)]
struct TestAddr {
    name: String,
    mode: DeliveryMode,
}

impl TestAddr {
    fn new(name: &str, mode: DeliveryMode) -> Self {
        Self {
            name: name.into(),
            mode,
        }
    }
}

impl Address for TestAddr {
    fn delivery_mode(&self) -> DeliveryMode {
        self.mode
    }
}

#[derive(Codec, Debug, Clone, PartialEq, Eq)]
struct TestMsg {
    text: String,
}

async fn spawn_server() -> (Arc<Server<TestAddr>>, u16) {
    let (server, addr) = Server::<TestAddr>::spawn("127.0.0.1:0").await.unwrap();
    println!("[setup] server listening on {addr}");
    (server, addr.port())
}

#[tokio::test]
async fn anycast_one_server_two_clients_three_producers() {
    println!("\n=== anycast test ===");
    let (server, port) = spawn_server().await;
    let addr = TestAddr::new("topic-anycast", DeliveryMode::Anycast);
    println!("[setup] address = {:?}", addr);

    let c1 = Client::<TestAddr>::connect(("127.0.0.1", port))
        .await
        .unwrap();
    println!("[setup] client1 connected");
    let c2 = Client::<TestAddr>::connect(("127.0.0.1", port))
        .await
        .unwrap();
    println!("[setup] client2 connected");

    let cons1 = c1.consumer::<TestMsg>(&addr).await.unwrap();
    println!("[sub] cons1 sub_id={}", cons1.sub_id());
    let cons2 = c2.consumer::<TestMsg>(&addr).await.unwrap();
    println!("[sub] cons2 sub_id={}", cons2.sub_id());

    println!("[produce] server -> 'from-server'");
    server
        .produce(&addr, TestMsg { text: "from-server".into() })
        .await
        .unwrap();
    println!("[produce] client1 -> 'from-c1'");
    c1.produce(&addr, TestMsg { text: "from-c1".into() })
        .await
        .unwrap();
    println!("[produce] client2 -> 'from-c2'");
    c2.produce(&addr, TestMsg { text: "from-c2".into() })
        .await
        .unwrap();

    // alternate so neither consumer drains everything before the other gets a chance
    let mut all: Vec<TestMsg> = Vec::new();
    loop {
        let m1 = cons1.consume().await.unwrap();
        let m2 = cons2.consume().await.unwrap();
        match (&m1, &m2) {
            (None, None) => {
                println!("[consume] both empty -> done");
                break;
            }
            _ => {}
        }
        if let Some(m) = m1 {
            println!("[consume] cons1 got '{}'", m.text);
            all.push(m);
        } else {
            println!("[consume] cons1 empty");
        }
        if let Some(m) = m2 {
            println!("[consume] cons2 got '{}'", m.text);
            all.push(m);
        } else {
            println!("[consume] cons2 empty");
        }
    }

    let texts: Vec<String> = all.iter().map(|m| m.text.clone()).collect();
    println!("[result] received {} message(s): {:?}", all.len(), texts);

    assert_eq!(
        all.len(),
        3,
        "anycast: each message must be delivered exactly once across all consumers"
    );
    let unique: HashSet<_> = all.into_iter().map(|m| m.text).collect();
    assert_eq!(
        unique,
        HashSet::from([
            "from-server".to_string(),
            "from-c1".to_string(),
            "from-c2".to_string(),
        ])
    );
    println!("[result] OK: every produced message delivered exactly once");
}

#[tokio::test]
async fn broadcast_one_server_two_clients_three_producers() {
    println!("\n=== broadcast test ===");
    let (server, port) = spawn_server().await;
    let addr = TestAddr::new("topic-broadcast", DeliveryMode::Broadcast);
    println!("[setup] address = {:?}", addr);

    let c1 = Client::<TestAddr>::connect(("127.0.0.1", port))
        .await
        .unwrap();
    println!("[setup] client1 connected");
    let c2 = Client::<TestAddr>::connect(("127.0.0.1", port))
        .await
        .unwrap();
    println!("[setup] client2 connected");

    let cons1 = c1.consumer::<TestMsg>(&addr).await.unwrap();
    println!("[sub] cons1 sub_id={}", cons1.sub_id());
    let cons2 = c2.consumer::<TestMsg>(&addr).await.unwrap();
    println!("[sub] cons2 sub_id={}", cons2.sub_id());

    println!("[produce] server -> 'from-server'");
    server
        .produce(&addr, TestMsg { text: "from-server".into() })
        .await
        .unwrap();
    println!("[produce] client1 -> 'from-c1'");
    c1.produce(&addr, TestMsg { text: "from-c1".into() })
        .await
        .unwrap();
    println!("[produce] client2 -> 'from-c2'");
    c2.produce(&addr, TestMsg { text: "from-c2".into() })
        .await
        .unwrap();

    let mut got1 = Vec::new();
    while let Some(m) = cons1.consume().await.unwrap() {
        println!("[consume] cons1 got '{}'", m.text);
        got1.push(m);
    }
    let mut got2 = Vec::new();
    while let Some(m) = cons2.consume().await.unwrap() {
        println!("[consume] cons2 got '{}'", m.text);
        got2.push(m);
    }

    let t1: Vec<String> = got1.iter().map(|m| m.text.clone()).collect();
    let t2: Vec<String> = got2.iter().map(|m| m.text.clone()).collect();
    println!("[result] cons1 received {} message(s): {:?}", got1.len(), t1);
    println!("[result] cons2 received {} message(s): {:?}", got2.len(), t2);

    let expected: HashSet<String> = HashSet::from([
        "from-server".into(),
        "from-c1".into(),
        "from-c2".into(),
    ]);
    assert_eq!(got1.len(), 3, "broadcast: cons1 should receive every message");
    assert_eq!(got2.len(), 3, "broadcast: cons2 should receive every message");
    let s1: HashSet<_> = got1.into_iter().map(|m| m.text).collect();
    let s2: HashSet<_> = got2.into_iter().map(|m| m.text).collect();
    assert_eq!(s1, expected);
    assert_eq!(s2, expected);
    println!("[result] OK: every consumer received every message");
}
