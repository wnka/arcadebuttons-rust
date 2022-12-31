use tokio::time::sleep;
use warp::Filter;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use futures_util::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

use rppal::{gpio::Gpio, gpio::InputPin, gpio::Trigger, gpio::Level};

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>>;

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

#[tokio::main]
async fn main() {
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

<<<<<<< HEAD
    // Placeholder to toggle button #2
    tokio::task::spawn(async move {
        loop {
            for (_uid, tx) in users_list_writer.read().await.iter() {
                if let Err(_disconnected) = tx.send(Message::text("2d")) {
                    // The tx is disconnected, our `user_disconnected` code
                    // should be happening in another task, nothing more to
                    // do here.
                }
            }
            sleep(Duration::from_millis(500)).await;
            for (_uid, tx) in users_list_writer.read().await.iter() {
                if let Err(_disconnected) = tx.send(Message::text("2u")) {
                    // The tx is disconnected, our `user_disconnected` code
                    // should be happening in another task, nothing more to
                    // do here.
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
    });

=======
    let (gpio_tx, client_rx) = mpsc::unbounded_channel();

    tokio::task::spawn(async move {
        loop {
            sleep(Duration::from_millis(50)).await;
            gpio_tx.send("2u").unwrap_or_else(|e| {
                eprintln!("websocket send error: {}", e);
            });
            sleep(Duration::from_millis(50)).await;
            gpio_tx.send("2d").unwrap_or_else(|e| {
                eprintln!("websocket send error: {}", e);
            });
        }
    });

    // Placeholder to toggle button #2
    tokio::task::spawn(async move {
        let mut rx = UnboundedReceiverStream::new(client_rx);
        while let Some(message) = rx.next().await {
                for (_uid, tx) in users_list_writer.read().await.iter() {
                    if let Err(_disconnected) = tx.send(Message::text(message)) {
                        // The tx is disconnected, our `user_disconnected` code
                        // should be happening in another task, nothing more to
                        // do here.
                    }
                }
            }
    });

>>>>>>> 01bce75 (use mpsc to bridge inputs to updates)
    warp::serve(routes).run(([0, 0, 0, 0], 6528)).await;
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

    // Every time the user sends a message, broadcast it to
    // all other users...
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
