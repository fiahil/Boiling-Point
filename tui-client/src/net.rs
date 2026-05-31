//! Live WebSocket transport. A read task decodes inbound [`ServerMessage`]s and
//! a write task encodes outbound [`ClientMessage`]s, bridged to the app loop by
//! channels. Used only by [`crate::run`]; the client is fully testable without
//! it via replay/mock (research R5).

use boiling_point_protocol::{codec, ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as TMsg};

/// Connect to `url` and return a receiver of decoded server messages plus a
/// sender for client intents. The receiver yields `None` (closes) when the
/// socket drops, which the app surfaces as a disconnect.
pub(crate) async fn connect(
    url: &str,
) -> Result<(mpsc::Receiver<ServerMessage>, mpsc::Sender<ClientMessage>), String> {
    let (ws, _resp) = connect_async(url).await.map_err(|e| e.to_string())?;
    let (mut sink, mut stream) = ws.split();

    let (in_tx, in_rx) = mpsc::channel::<ServerMessage>(64);
    let (out_tx, mut out_rx) = mpsc::channel::<ClientMessage>(64);

    // Reader: socket -> decode -> app.
    tokio::spawn(async move {
        while let Some(Ok(frame)) = stream.next().await {
            if let TMsg::Binary(bytes) = frame {
                match codec::decode::<ServerMessage>(bytes.as_ref()) {
                    Ok(msg) => {
                        if in_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => continue, // ignore undecodable frames
                }
            }
        }
        // Dropping `in_tx` here closes the receiver -> the app sees a disconnect.
    });

    // Writer: app intents -> encode -> socket.
    tokio::spawn(async move {
        while let Some(intent) = out_rx.recv().await {
            match codec::encode(&intent) {
                Ok(bytes) => {
                    if sink.send(TMsg::Binary(bytes)).await.is_err() {
                        break;
                    }
                }
                Err(_) => continue,
            }
        }
    });

    Ok((in_rx, out_tx))
}
