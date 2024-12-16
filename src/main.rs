use futures::future::join_all;
use nostr_sdk::prelude::*;
use std::io::Read;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

#[tokio::main]
async fn main() -> Result<()> {
    // get keys (generate first time with Keys::generate())
    let mut f = std::fs::File::open("./keys.txt").unwrap();
    let mut keys = String::new();
    f.read_to_string(&mut keys).unwrap();
    let keys = Keys::parse(&keys).unwrap();
    let client = Client::new(keys.clone());
    // let client = Client::default();

    add_relays(&client).await.unwrap();

    let keys = get_keys();
    let admins = get_admins();

    let (sender, receiver) = channel::<Event>();

    let sub_notes = Filter::new()
        .authors(keys)
        .kind(Kind::TextNote)
        .since(Timestamp::min());

    let sub_admin = Filter::new()
        .authors(admins.clone())
        .kinds(vec![
            Kind::PrivateDirectMessage,
            Kind::EncryptedDirectMessage,
        ])
        .since(Timestamp::now());

    // Subscribe (auto generate subscription ID)
    let Output { .. } = client.subscribe(vec![sub_notes], None).await.unwrap();

    // Spawn off an expensive computation
    println!("spawning");
    thread::spawn(move || {
        handle_events(receiver, admins);
    });

    // Handle subscription notifications with `handle_notifications` method
    println!("starting sub");
    client
        .handle_notifications(|notification| async {
            if let RelayPoolNotification::Event { event, .. } = notification {
                if event.kind == Kind::TextNote {
                    sender.send(*event).unwrap();
                } else if event.kind == Kind::PrivateDirectMessage {
                    sender.send(*event).unwrap();
                } else if event.kind == Kind::EncryptedDirectMessage {
                    sender.send(*event).unwrap();
                }
            }
            Ok(false) // Set to true to exit from the loop
        })
        .await
        .unwrap();

    Ok(())
}

fn handle_events(receiver: Receiver<Event>, admins: Vec<PublicKey>) {
    // todo
    println!("Waiting for events");
    loop {}
}

async fn add_relays(client: &Client) -> Result<()> {
    let mut f = std::fs::File::open("./relays.txt").unwrap();
    let mut relays = String::new();
    f.read_to_string(&mut relays).unwrap();
    let forjoin = relays.split("\n").map(|r| client.add_relay(r));
    join_all(forjoin).await;

    Ok(())
}

fn get_keys() -> Vec<PublicKey> {
    let mut f = std::fs::File::open("./npubs.txt").unwrap();
    let mut keys = String::new();
    f.read_to_string(&mut keys).unwrap();
    keys.split("\n")
        .map(|pubkey| PublicKey::parse(pubkey).unwrap())
        .collect::<Vec<PublicKey>>()
}
fn get_admins() -> Vec<PublicKey> {
    let mut f = std::fs::File::open("./admins.txt").unwrap();
    let mut keys = String::new();
    f.read_to_string(&mut keys).unwrap();
    keys.split("\n")
        .map(|pubkey| PublicKey::parse(pubkey).unwrap())
        .collect::<Vec<PublicKey>>()
}
