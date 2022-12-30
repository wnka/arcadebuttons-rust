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

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>>;

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

#[tokio::main]
async fn main() {
    //let route = warp::path("static").and(warp::fs::dir("resources"));

    // Keep track of all connected users, key is usize, value
    // is a websocket sender.
    let users = Users::default();
    // Turn our "state" into a new Filter...
    let users = warp::any().map(move || users.clone());

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

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn user_connected(ws: WebSocket, users: Users) {
    let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

    eprintln!("new user: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (mut user_ws_tx, mut _user_ws_rx) = ws.split();

    // Use an unbounded channel to handle buffering and flushing of messages
    // to the websocket...
    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    user_ws_tx.send(Message::text("2d")).unwrap_or_else(|e| eprintln!("uh oh: {}", e)).await;
    user_ws_tx.send(Message::text("1d")).unwrap_or_else(|e| eprintln!("uh oh: {}", e)).await;
    user_ws_tx.send(Message::text("3d")).unwrap_or_else(|e| eprintln!("uh oh: {}", e)).await;
    user_ws_tx.send(Message::text("4d")).unwrap_or_else(|e| eprintln!("uh oh: {}", e)).await;
    user_ws_tx.send(Message::text("ud")).unwrap_or_else(|e| eprintln!("uh oh: {}", e)).await;
    user_ws_tx.send(Message::text("rd")).unwrap_or_else(|e| eprintln!("uh oh: {}", e)).await;
    user_ws_tx.send(Message::text("2u")).unwrap_or_else(|e| eprintln!("uh oh: {}", e)).await;

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

    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    //user_disconnected(my_id, &users).await;
}

async fn user_disconnected(my_id: usize, users: &Users) {
    eprintln!("good bye user: {}", my_id);

    // Stream closed up, so remove from the user list
    users.write().await.remove(&my_id);
}
