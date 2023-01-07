use warp::Filter;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use futures_util::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

#[cfg(target_os = "linux")]
use tokio_gpiod::{Chip, Options, EdgeDetect};

use clap::Parser;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to use
    #[arg(short, long, default_value_t = 6528)]
    port: u16,

    #[arg(short, long, default_value_t = false)]
    mock: bool,
}

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>>;

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

#[tokio::main]
async fn main() {

    let args = Args::parse();

    // Keep track of all connected users, key is usize, value
    // is a websocket sender.
    let users_list = Users::default();

    // a clone for the thing that actually updates states
    let users_list_writer = users_list.clone();

    // Turn our "state" into a new Filter...
    let users = warp::any().map(move || users_list.clone());

    // GET /buttons -> websocket upgrade
    let buttons = warp::path("buttons")
    // The `ws()` filter will prepare Websocket handshake...
        .and(warp::ws())
        .and(users)
        .map(|ws: warp::ws::Ws, users| {
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| user_connected(socket, users))
        });

    // GET / -> index html
    let index = warp::path::end().map(|| warp::reply::html(include_str!("../resources/index.html")));

    let routes = index.or(buttons);

    let (gpio_tx, client_rx) = mpsc::unbounded_channel::<String>();

    if args.mock {
        tokio::task::spawn(async move {
            // Input the Contra code
            let cycle = vec![
                "ud", "uu", "ud", "uu", "dd", "du", "dd", "du", // up up down down
                "ld", "lu", "rd", "ru", "ld", "lu", "rd", "ru", // left right left right
                "1d", "1u", "2d", "2u", // b a
                "1d", "1u", "2d", "2u", // b a
                "3d", "3u", "4d", "4u", // "select" "start"
            ];
            loop {
                for state in cycle.clone().into_iter() {
                    gpio_tx.send(String::from(state)).unwrap_or_else(|e| {
                        eprintln!("websocket send error: {}", e);
                    });
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
            }
        });
    }
    else {
        #[cfg(target_os = "linux")]
        // Broadcast updates from GPIO to the thing that sends to clients
        tokio::task::spawn(async move {
            // open chip. why gpiochip0? not sure! but it works!
            // at least it works on an RPi4b
            let chip = Chip::new("gpiochip0").await.unwrap();

            let opts = Options::input([16, 6, 20, 12, 19, 26, 21, 13]) // configure lines offsets
                .edge(EdgeDetect::Both); // We want events for both button up and button down

            let mut inputs = chip.request_lines(opts).await.unwrap();
            while let Ok(event) = inputs.read_event().await {
                // the 'line' matches the index of the Options::input
                // array above. So, pin 16, which is up, is index 0.
                let input = match event.line {
                    0 => 'u', // up
                    1 => 'd', // down
                    2 => 'l', // left
                    3 => 'r', // right
                    4 => '1', // button 1
                    5 => '2', // button 2
                    6 => '3', // button 3
                    7 => '4', // button 4
                    _ => panic!("Unknown line"),
                };

                let state = match event.edge {
                    tokio_gpiod::Edge::Falling => 'd', // pressed
                    tokio_gpiod::Edge::Rising => 'u', // not pressed
                };

                // We send a 2 character message over the websocket, with the input
                // (i.e. 'l' for left on the joystick) and its state (i.e. 'd' for
                // pressed).
                //
                // So, push the joystick to the left = "ld", let go = "lu".
                gpio_tx.send(format!("{}{}", input, state)).unwrap_or_else(|e| {
                    eprintln!("websocket send error: {}", e);
                });
            }
        });
        #[cfg(not(target_os = "linux"))]
        panic!("Real input from GPIO only supported on Linux! Use --mock for test inputs.")
    }
    // Broadcast updates to clients
    tokio::task::spawn(async move {
        let mut rx = UnboundedReceiverStream::new(client_rx);
        // Read a message off the channel where GPIO writes go.
        while let Some(message) = rx.next().await {
            for (_uid, tx) in users_list_writer.read().await.iter() {
                if let Err(_disconnected) = tx.send(Message::text(message.clone())) {
                    // The tx is disconnected, our `user_disconnected` code
                    // should be happening in another task, nothing more to
                    // do here.
                }
            }
        }
    });

    eprintln!("Starting server on port {}", args.port);
    eprintln!("http://localhost:{}", args.port);
    warp::serve(routes).run(([0, 0, 0, 0], args.port)).await;
}

async fn user_connected(ws: WebSocket, users: Users) {
    let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    eprintln!("new user: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    // Use an unbounded channel to handle buffering and flushing of messages
    // to the websocket...
    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx
                .send(message)
                .unwrap_or_else(|e| {
                    eprintln!("websocket send error: {}", e);
                })
                .await;
        }
    });

    // Save the sender in our list of connected users.
    users.write().await.insert(my_id, tx);

    // This is to process messages from clients, which... there aren't any!
    // Except for the "close" message when you shutdown a client.
    while let Some(result) = user_ws_rx.next().await {
        match result {
            Ok(msg) => eprintln!("Got a message from client {}: {:?}", my_id, msg),
            Err(e) => {
                eprintln!("websocket error(uid={}): {}", my_id, e);
                break;
            }
        };
    }

    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    user_disconnected(my_id, &users).await;
}

async fn user_disconnected(my_id: usize, users: &Users) {
    eprintln!("good bye user: {}", my_id);

    // Stream closed up, so remove from the user list
    users.write().await.remove(&my_id);
}
