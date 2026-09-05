use anyhow::{Context, Result};
use futures_util::SinkExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

pub async fn send_text<S>(socket: &mut WebSocketStream<S>, body: String) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    timeout(Duration::from_secs(10), socket.send(Message::Text(body)))
        .await
        .context("websocket write timeout")?
        .context("send websocket message")
}
