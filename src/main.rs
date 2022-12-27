use clap::Parser;
use nostr_sdk::nostr::{Keys, Kind, RelayMessage, SubscriptionFilter};
use nostr_sdk::Client;
use nostr_sdk::RelayPoolNotifications::{ReceivedEvent, ReceivedMessage};
use std::time::{Duration, SystemTime};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CommandArgs {
    /// Source nodes to copy events from
    #[arg(short, long)]
    sources: Vec<String>,

    /// Destination relay to copy events to
    #[arg(short, long)]
    destination: String,

    /// Kinds of events to copy
    #[arg(long)]
    kinds: Vec<u64>,

    #[arg(long, default_value_t = 2)]
    hours: u8,
}

#[tokio::main]
async fn main() {
    let args = CommandArgs::parse();

    println!("{:?}", args);

    let keys = Keys::generate_from_os_random();

    let source_client = Client::new(&keys);
    for r in args.sources.iter() {
        source_client.add_relay(r, None).await.unwrap();
    }
    source_client.connect().await.unwrap();

    let dest_client = Client::new(&keys);
    dest_client.add_relay(args.destination, None).await.unwrap();
    dest_client.connect().await.unwrap();

    // search range
    let batch_range = Duration::from_secs(60 * 60 * args.hours as u64);
    let mut before = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut after = before - batch_range.as_secs();
    let mut sub = SubscriptionFilter::new()
        .kinds(args.kinds.iter().map(|a| Kind::Custom(*a)).collect())
        .until(before)
        .since(after);

    println!("Sending sub: {:?}", sub);
    source_client.subscribe(vec![sub.clone()]).await.unwrap();

    loop {
        let mut notifications = source_client.notifications();
        while let Ok(notification) = notifications.recv().await {
            match notification {
                ReceivedEvent(_) => {
                    // not used
                }
                ReceivedMessage(msg) => match msg {
                    RelayMessage::Event { event, .. } => {
                        println!(">> {}", event.id);
                        dest_client.send_event(*event).await.unwrap();
                    }
                    RelayMessage::Notice { message } => {
                        println!("NOTICE: {}", message);
                    }
                    RelayMessage::EndOfStoredEvents { .. } => {
                        before -= batch_range.as_secs();
                        after -= batch_range.as_secs();
                        println!("Before: {}", before);
                        sub = sub.until(before).since(after);
                        source_client.subscribe(vec![sub.clone()]).await.unwrap();
                    }
                    RelayMessage::Ok { event_id, .. } => {
                        println!(">> {}", event_id);
                    }
                    RelayMessage::Empty => {}
                },
            }
        }
    }
}
