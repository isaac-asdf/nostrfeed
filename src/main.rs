use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;

use nostr_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::default();

    client.add_relay("wss://nostr.oxtr.dev").await?;
    client.add_relay("wss://relay.damus.io").await?;
    client.add_relay("wss://nostr.openchain.fr").await?;

    client.connect().await;

    let keys = get_keys();

    let subscription = Filter::new()
        .authors(keys)
        .kind(Kind::TextNote)
        .since(Timestamp::min());

    // Subscribe (auto generate subscription ID)
    let Output { val: sub_id_1, .. } = client.subscribe(vec![subscription], None).await?;

    // Handle subscription notifications with `handle_notifications` method
    client
        .handle_notifications(|notification| async {
            if let RelayPoolNotification::Event {
                subscription_id,
                event,
                ..
            } = notification
            {
                // Check subscription ID
                if subscription_id == sub_id_1 {
                    // Handle (ex. update specific UI)
                }

                // Check kind
                if event.kind == Kind::TextNote {
                    println!("TextNote: {:?}", event);
                } else {
                    println!("{:?}", event);
                }
            }
            Ok(false) // Set to true to exit from the loop
        })
        .await?;

    Ok(())
}

fn get_keys() -> Vec<PublicKey> {
    let mut f = std::fs::File::open("./npubs.txt").unwrap();
    let mut keys = String::new();
    f.read_to_string(&mut keys).unwrap();
    println!("{keys}");
    keys.split("\n")
        .map(|pubkey| PublicKey::parse(pubkey).unwrap())
        .collect::<Vec<PublicKey>>()
}
