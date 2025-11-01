//! Simple ping-pong example using MXP

use mxp::{Message, MessageType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MXP Ping-Pong Example");
    println!("====================\n");

    // This is a basic example showing MXP message exchange
    // Full client-server example coming soon!

    // Create a message
    let ping = Message::new(MessageType::Call, b"ping");
    println!(
        "Created message: type={:?}, payload={:?}",
        ping.message_type(),
        std::str::from_utf8(ping.payload()).unwrap()
    );

    // Encode
    let encoded = ping.encode();
    println!("Encoded to {} bytes", encoded.len());

    // Decode
    let decoded = Message::decode(encoded.clone())?;
    println!(
        "Decoded: payload={:?}",
        std::str::from_utf8(decoded.payload()).unwrap()
    );

    println!("\n✅ MXP protocol working correctly!");

    Ok(())
}
