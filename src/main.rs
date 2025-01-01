use futures::future::join_all;
use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::mpsc::{channel, Receiver};
use tokio::task;
use toml;

const CONFIGSTR: &str = "./Config.toml";
const DVM_REQ: Kind = Kind::JobRequest(5300);
const DVM_RESP: Kind = Kind::JobResult(6300);
const DVM_ADVERT: Kind = Kind::Custom(31990);

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    package: Package,
    comms: Comms,
}

#[derive(Debug, Deserialize, Serialize)]
struct Package {
    name: String,
    about: String,
    lnurl: String,
    nsec: Option<String>,
    announced: bool,
    random_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Comms {
    relays: Vec<String>,
    admins: Vec<String>,
    npubs: Vec<String>,
}

fn set_followlist(config: &Config) -> EventBuilder {
    let mut tags = Vec::new();
    config.comms.npubs.iter().for_each(|n| {
        let pk = PublicKey::parse(n).unwrap();
        let tag: Tag = Tag::from_standardized(TagStandard::public_key(pk));
        tags.push(tag);
    });
    let event = EventBuilder::new(Kind::ContactList, "").tags(tags);
    event
}

#[tokio::main]
async fn main() -> Result<()> {
    // get config
    let mut config = get_config();
    let keyopt = config.package.nsec.clone();
    let keys: Keys = Keys::parse(keyopt.expect("nsec generation error")).unwrap();
    let npubs = get_npubs(&config.comms.npubs);
    let admins = get_admins(&config.comms.npubs);
    let client = Client::new(keys.clone());
    add_relays(&client, &config.comms.relays).await.unwrap();
    // client.add_relay("ws://localhost:7000").await?;
    client.connect().await;

    let event = set_followlist(&config);
    client.send_event_builder(event).await.unwrap();
    println!("Find me at: {}", keys.public_key().to_bech32()?);

    // verify we have announced
    if !config.package.announced {
        announce_me(&config, &client).await;
        config.package.announced = true;
        save_config(&config);
        println!("Announced!");
    }

    // Generate filters for subscriptions
    let sub_notes = Filter::new()
        .authors(npubs)
        .kind(Kind::TextNote)
        .since(Timestamp::min());
    let sub_dvmreq = Filter::new()
        .kind(DVM_REQ)
        .pubkey(keys.public_key())
        .since(Timestamp::now());

    // todo: Verify DM decrypting
    // let sub_admin = Filter::new()
    //     .authors(admins.clone())
    //     .kinds(vec![
    //         Kind::PrivateDirectMessage,
    //         Kind::EncryptedDirectMessage,
    //     ])
    //     .since(Timestamp::min());
    let Output { .. } = client.subscribe(vec![sub_notes, sub_dvmreq], None).await?;

    // Setup background thread for processing received events
    let (sender, receiver) = channel::<Event>();
    let c = client.clone();
    task::spawn(handle_events(receiver, admins, c));

    // Wait for events to come in
    client
        .handle_notifications(|notification| async {
            if let RelayPoolNotification::Event { event, .. } = notification {
                if event.kind != Kind::TextNote {
                    println!("{event:?}");
                }
                sender.send(*event).unwrap();
            }
            Ok(false) // Set to true to exit from the loop
        })
        .await
        .unwrap();

    Ok(())
}

/// Shuffle received events to proper function.
/// Update event list with TextNotes, Send DVM_RESP when we get a REQ,
/// and handle admin DMs
async fn handle_events(receiver: Receiver<Event>, _admins: Vec<PublicKey>, client: Client) {
    println!("Waiting for events");
    let mut events: Vec<Event> = Vec::new();
    loop {
        let ev = receiver.recv().unwrap();
        match ev.kind {
            Kind::TextNote => update_event_list(&mut events, ev),
            Kind::JobRequest(5300) => send_resp(ev, &client, &events).await,
            Kind::PrivateDirectMessage => handle_cmd(ev, &client),
            Kind::EncryptedDirectMessage => handle_cmd(ev, &client),
            _ => (),
        }
    }
}

/// Send a 6300 after getting a 5300, referencing the request job
async fn send_resp(event: Event, client: &Client, events: &Vec<Event>) {
    // relays: relays used
    let eventlist = events
        .iter()
        .map(|e| format!("[\"e\", \"{}\"]", e.id.to_string()))
        .collect::<Vec<String>>();
    let eventlist = eventlist.join(",");
    let eventlist = format!("[{eventlist}]");
    let signer = client.signer().await.unwrap();
    let etag = Tag::parse(["e", &event.id.to_string()]).unwrap();
    let ptag = Tag::parse(["p", &event.pubkey.to_hex()]).unwrap();
    let stag = Tag::parse(["status", "success"]).unwrap();
    let alttag = Tag::parse(["alt", "MN DVM Result"]).unwrap();
    let ev = EventBuilder::new(DVM_RESP, &eventlist)
        .tags([etag, ptag, stag, alttag])
        .sign(&signer)
        .await
        .unwrap();
    client.send_event(ev).await.unwrap();
    println!("Sent response");
}

/// Adds an event to the list if it is not present yet.
/// Uses a simple time based ordering
fn update_event_list(list: &mut Vec<Event>, event: Event) {
    if !list.contains(&event) {
        list.push(event);
        list.sort_by(|a, b| {
            if a.created_at == b.created_at {
                a.id.cmp(&b.id)
            } else {
                // sort timestamps in descending order (ie, earlier notes show at front)
                b.created_at.cmp(&a.created_at)
            }
        });
        while list.len() > 200 {
            list.pop();
        }
    }
}

fn handle_cmd(_event: Event, _client: &Client) {
    // unimplemented
}

async fn announce_me(config: &Config, client: &Client) {
    // set metadata
    let md = Metadata::new()
        .name(&config.package.name)
        .lud06(&config.package.lnurl)
        .about(&config.package.about);
    client.set_metadata(&md).await.unwrap();

    // announce DVM
    use serde_json::json;
    let content = json!({"name": config.package.name, "about" : config.package.about, "encryptionSupported": false})
        .to_string();
    let tag_k = Tag::parse(["k", "5300"]).unwrap();
    let dval = config.package.random_id.clone();
    let tag_d = Tag::parse(["d", &dval.unwrap()]).unwrap();
    let signer = client.signer().await.unwrap();
    let ev = EventBuilder::new(DVM_ADVERT, content)
        .tags([tag_k, tag_d])
        .sign(&signer);
    let ev = ev.await.unwrap();
    client.send_event(ev).await.unwrap();
}
fn get_config() -> Config {
    let mut f = std::fs::File::open(CONFIGSTR).unwrap();
    let mut filestr = String::new();
    f.read_to_string(&mut filestr).unwrap();
    let mut config: Config = toml::from_str(&filestr).unwrap();
    if config.package.nsec.is_none() {
        let keys = Keys::generate();
        config.package.nsec = Some(keys.secret_key().to_secret_hex());
        save_config(&config);
    }
    if config.package.random_id.is_none() {
        let mut randid = Keys::generate()
            .public_key
            .to_bech32()
            .expect("keygen err")
            .replace("npub", "");
        randid.truncate(20);
        config.package.random_id = Some(randid);
        save_config(&config);
    }
    config
}
fn save_config(config: &Config) {
    let out = toml::to_string_pretty(config).unwrap();
    std::fs::write(CONFIGSTR, &out).unwrap();
}
async fn add_relays(client: &Client, relays: &Vec<String>) -> Result<()> {
    let forjoin = relays.iter().map(|r| client.add_relay(r));
    join_all(forjoin).await;

    Ok(())
}
fn get_npubs(npubs: &Vec<String>) -> Vec<PublicKey> {
    npubs
        .iter()
        .map(|pubkey| PublicKey::parse(pubkey).unwrap())
        .collect::<Vec<PublicKey>>()
}
fn get_admins(admins: &Vec<String>) -> Vec<PublicKey> {
    admins
        .iter()
        .map(|pubkey| PublicKey::parse(pubkey).unwrap())
        .collect::<Vec<PublicKey>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tconfig() {
        let config = get_config();
        let keyopt = config.package.nsec.clone();
        let keys: Keys = Keys::parse(keyopt.expect("nsec generation error")).unwrap();
        let client = Client::new(keys.clone());
        announce_me(&config, &client).await;
    }
}
