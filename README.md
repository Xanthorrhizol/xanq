# xanq

A simple embeddable message queue.

- Embed the queue directly in your process, or run it as a TCP server multiple processes connect to.
- Per-address delivery mode: **Anycast** (competing consumers) or **Broadcast** (fan-out).
- Typed addresses and messages — bring your own structs, derive `Codec`, you're done.
- Async, built on Tokio.

## Quick start

Add the crate and define your address and message types.

```sh
cargo add xanq
cargo add xancode
cargo add tokio --features macros,rt-multi-thread
```

```rust
use xancode::Codec;
use xanq::address::{Address, DeliveryMode};

#[derive(Codec, Clone)]
struct Topic { name: String }

impl Address for Topic {
    fn delivery_mode(&self) -> DeliveryMode {
        DeliveryMode::Anycast
    }
}

#[derive(Codec)]
struct Greeting { text: String }
```

### In-process (single binary)

```rust
use xanq::consumer::Consumer;
use xanq::server::Server;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let server = Server::<Topic>::new();
    let topic = Topic { name: "greetings".into() };

    let cons = server.consumer::<Greeting>(&topic).await?;
    server.produce(&topic, Greeting { text: "hello".into() }).await?;

    let msg = cons.consume().await?.unwrap();
    println!("{}", msg.text);
    Ok(())
}
```

### Cross-process (server + clients)

Server binary:

```rust
use xanq::server::Server;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (_server, bound) = Server::<Topic>::spawn("0.0.0.0:9000").await?;
    println!("xanq listening on {bound}");
    tokio::signal::ctrl_c().await.ok();
    Ok(())
}
```

Client binary:

```rust
use xanq::client::Client;
use xanq::consumer::Consumer;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let client = Client::<Topic>::connect("server.host:9000").await?;
    let topic = Topic { name: "greetings".into() };

    let cons = client.consumer::<Greeting>(&topic).await?;
    client.produce(&topic, Greeting { text: "hi".into() }).await?;

    let msg = cons.consume().await?.unwrap();
    println!("{}", msg.text);
    Ok(())
}
```

The same `Server` instance can serve in-process consumers/producers and remote
clients simultaneously — embed it in one process and let other processes
connect over TCP.

## Concepts

**Address.** A user-defined type that identifies a queue. Implements
`Address`, which requires `Codec` (xancode) — derive it. The address's
`delivery_mode()` decides how the queue distributes messages.

**DeliveryMode.** Set per address.
- `Anycast` — one shared queue per address. Each message is delivered to
  exactly one consumer that pops it.
- `Broadcast` — each subscriber gets its own queue. Every message is
  delivered to every current subscriber.

The first producer or consumer to touch an address fixes its mode for the
server; later operations with a mismatched mode return an error.

**Message.** Any type implementing `Codec`. No marker trait needed — derive
`Codec` and pass it to `produce` / receive it from `consume`.

**Producer / Consumer.**
- `server.produce(&addr, msg).await` / `client.produce(&addr, msg).await` —
  one-shot publish.
- `server.producer(addr)` / `client.producer(addr)` — reusable handle when
  you publish to the same address repeatedly.
- `server.consumer::<Msg>(&addr).await?` /
  `client.consumer::<Msg>(&addr).await?` — subscribe and get a handle.
  `consumer.consume().await` returns `Option<Msg>` (None when the queue is
  empty). The subscription is released when the handle is dropped.

## License

MIT — see [LICENSE](LICENSE).
