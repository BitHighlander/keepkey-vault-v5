pub mod protocol_adapter;
pub mod usb;
pub mod webusb;
pub mod hid;

pub use protocol_adapter::*;
pub use usb::*;
pub use webusb::*;
pub use hid::*;

use crate::messages::{self, Message};
use anyhow::{anyhow, bail, Result};
use core::time::Duration;
use std::io::{stdin, stdout, Write};
use log::info;

pub trait Transport {
    type Error: std::error::Error;
    fn write(&mut self, msg: &[u8], timeout: Duration) -> Result<usize, Self::Error>;
    fn read(&mut self, buf: &mut Vec<u8>, timeout: Duration) -> Result<(), Self::Error>;
    fn reset(&mut self) -> Result<(), Self::Error>;
}

pub fn standard_message_handler(msg: &Message) -> Result<Option<Message>> {
    info!("StandardHandler: Processing message type: {:?}", msg.message_type());
    
    Ok(match msg {
        Message::ButtonRequest(req) => {
            info!("StandardHandler: ButtonRequest received, code: {:?}", req.code);
            eprintln!("Confirm action on device...");
            let ack = messages::ButtonAck::default();
            info!("StandardHandler: Sending ButtonAck");
            Some(ack.into())
        }
        Message::PinMatrixRequest(x) => {
            match x.r#type {
                Some(t) => match messages::PinMatrixRequestType::from_i32(t)
                    .ok_or_else(|| anyhow!("unrecognized PinMatrixRequestType ({})", t))?
                {
                    messages::PinMatrixRequestType::Current => {
                        eprint!("Enter current PIN: ")
                    }
                    messages::PinMatrixRequestType::NewFirst => eprint!("Enter new PIN: "),
                    messages::PinMatrixRequestType::NewSecond => {
                        eprint!("Re-enter new PIN: ")
                    }
                },
                None => bail!("expected PinMatrixRequestType"),
            }
            stdout().flush().unwrap();
            let mut pin = String::new();
            stdin().read_line(&mut pin)?;
            let pin = pin.trim();
            Some(
                messages::PinMatrixAck {
                    pin: pin.to_owned(),
                }
                .into(),
            )
        }
        Message::PassphraseRequest(_) => {
            eprint!("Enter BIP-39 passphrase: ");
            stdout().flush().unwrap();
            let mut passphrase = String::new();
            stdin().read_line(&mut passphrase)?;
            let passphrase = passphrase.trim().to_owned();
            Some(messages::PassphraseAck { passphrase }.into())
        }
        Message::Failure(x) => bail!("Failure: {}", x.message()),
        _ => None,
    })
}

/// Handler for PIN creation flows that handles ButtonRequest but passes through PinMatrixRequest
/// This allows the frontend to handle PIN matrix input while still automatically confirming buttons
pub fn pin_flow_message_handler(msg: &Message) -> Result<Option<Message>> {
    info!("PinFlowHandler: Processing message type: {:?}", msg.message_type());
    
    Ok(match msg {
        Message::ButtonRequest(req) => {
            info!("PinFlowHandler: ButtonRequest received, code: {:?}", req.code);
            eprintln!("Confirm action on device...");
            let ack = messages::ButtonAck::default();
            info!("PinFlowHandler: Sending ButtonAck");
            Some(ack.into())
        }
        Message::PinMatrixRequest(_) => {
            info!("PinFlowHandler: PinMatrixRequest received, passing through to frontend");
            // Don't handle PIN matrix - let frontend handle it
            None
        }
        Message::EntropyRequest(_) => {
            info!("PinFlowHandler: EntropyRequest received, providing entropy automatically (display_random=false)");
            
            // Generate 32 bytes of random entropy for device (standard requirement)
            let mut entropy = [0u8; 32];
            use rand::RngCore;
            rand::thread_rng().fill_bytes(&mut entropy);
            
            info!("PinFlowHandler: Sending 32 bytes of entropy to complete device initialization");
            
            let entropy_ack = messages::EntropyAck { 
                entropy: Some(entropy.into())
            };
            Some(entropy_ack.into())
        }
        Message::PassphraseRequest(_) => {
            info!("PinFlowHandler: PassphraseRequest received, passing through to frontend");
            // Don't handle passphrase in PIN flow - let frontend handle it
            None
        }
        Message::Failure(x) => bail!("Failure: {}", x.message()),
        _ => None,
    })
}

/// Handler for recovery flows that handles ButtonRequest but passes through CharacterRequest
/// This allows the frontend to handle character input for recovery phrase entry
pub fn recovery_flow_message_handler(msg: &Message) -> Result<Option<Message>> {
    info!("RecoveryFlowHandler: Processing message type: {:?}", msg.message_type());
    
    Ok(match msg {
        Message::ButtonRequest(req) => {
            info!("RecoveryFlowHandler: ButtonRequest received, code: {:?}", req.code);
            eprintln!("Confirm action on device...");
            let ack = messages::ButtonAck::default();
            info!("RecoveryFlowHandler: Sending ButtonAck");
            Some(ack.into())
        }
        Message::CharacterRequest(_) => {
            info!("RecoveryFlowHandler: CharacterRequest received, passing through to frontend");
            // Don't handle character input - let frontend handle it
            None
        }
        Message::PinMatrixRequest(_) => {
            info!("RecoveryFlowHandler: PinMatrixRequest received, passing through to frontend");
            // Don't handle PIN matrix - let frontend handle it for recovery PIN setup
            None
        }
        Message::EntropyRequest(_) => {
            info!("RecoveryFlowHandler: EntropyRequest received, providing entropy automatically");
            
            // Generate 32 bytes of random entropy for device (standard requirement)
            let mut entropy = [0u8; 32];
            use rand::RngCore;
            rand::thread_rng().fill_bytes(&mut entropy);
            
            info!("RecoveryFlowHandler: Sending 32 bytes of entropy for recovery completion");
            
            let entropy_ack = messages::EntropyAck { 
                entropy: Some(entropy.into())
            };
            Some(entropy_ack.into())
        }
        Message::PassphraseRequest(_) => {
            info!("RecoveryFlowHandler: PassphraseRequest received, passing through to frontend");
            // Don't handle passphrase in recovery flow - let frontend handle it
            None
        }
        Message::Failure(x) => bail!("Failure: {}", x.message()),
        _ => None,
    })
}

pub trait ProtocolAdapter {
    fn reset(&mut self) -> Result<()>;
    fn send(&mut self, msg: Message) -> Result<()>;
    fn handle(&mut self, msg: Message) -> Result<Message>;
    fn as_mut_dyn(&mut self) -> &mut dyn ProtocolAdapter;
    fn with_handler<'a: 'b, 'b>(
        &'a mut self,
        handler: &'b MessageHandler<'b>,
    ) -> Box<dyn ProtocolAdapter + 'b> {
        Box::from(MessageHandlerStack {
            parent_adapter: self.as_mut_dyn(),
            handler,
        })
    }
    fn with_mut_handler<'a: 'b, 'b>(
        &'a mut self,
        handler: &'b mut MessageHandlerMut<'b>,
    ) -> Box<dyn ProtocolAdapter + 'b> {
        Box::from(MessageHandlerMutStack {
            parent_adapter: self.as_mut_dyn(),
            handler,
        })
    }
    fn with_standard_handler<'a>(&'a mut self) -> Box<dyn ProtocolAdapter + 'a> {
        self.with_handler(&standard_message_handler)
    }
    fn with_pin_flow_handler<'a>(&'a mut self) -> Box<dyn ProtocolAdapter + 'a> {
        self.with_handler(&pin_flow_message_handler)
    }
    fn with_recovery_flow_handler<'a>(&'a mut self) -> Box<dyn ProtocolAdapter + 'a> {
        self.with_handler(&recovery_flow_message_handler)
    }
}

pub type MessageHandler<'a> = dyn Fn(&Message) -> Result<Option<Message>> + 'a;
pub type MessageHandlerMut<'a> = dyn FnMut(&Message) -> Result<Option<Message>> + 'a;

pub struct MessageHandlerStack<'a, 'b> {
    parent_adapter: &'a mut dyn ProtocolAdapter,
    handler: &'b MessageHandler<'b>,
}

pub struct MessageHandlerMutStack<'a, 'b> {
    parent_adapter: &'a mut dyn ProtocolAdapter,
    handler: &'b mut MessageHandlerMut<'b>,
}

impl ProtocolAdapter for MessageHandlerStack<'_, '_> {
    fn reset(&mut self) -> Result<()> {
        self.parent_adapter.reset()
    }
    fn send(&mut self, msg: Message) -> Result<()> {
        self.parent_adapter.send(msg)
    }
    fn handle(&mut self, msg: Message) -> Result<Message> {
        info!("MessageHandlerStack::handle: Starting with message type: {:?}", msg.message_type());
        let mut msg = msg;
        loop {
            info!("MessageHandlerStack::handle: Calling parent adapter...");
            let msg_out = self.parent_adapter.handle(msg)?;
            info!("MessageHandlerStack::handle: Parent returned message type: {:?}", msg_out.message_type());
            
            match (self.handler)(&msg_out)? {
                Some(x) => {
                    info!("MessageHandlerStack::handle: Handler returned message type: {:?}, looping...", x.message_type());
                    msg = x;
                }
                None => {
                    info!("MessageHandlerStack::handle: Handler returned None, done");
                    return Ok(msg_out);
                }
            }
        }
    }
    fn as_mut_dyn(&mut self) -> &mut dyn ProtocolAdapter {
        self
    }
}

impl ProtocolAdapter for MessageHandlerMutStack<'_, '_> {
    fn reset(&mut self) -> Result<()> {
        self.parent_adapter.reset()
    }
    fn send(&mut self, msg: Message) -> Result<()> {
        self.parent_adapter.send(msg)
    }
    fn handle(&mut self, msg: Message) -> Result<Message> {
        let mut msg = msg;
        loop {
            let msg_out = self.parent_adapter.handle(msg)?;
            match (self.handler)(&msg_out)? {
                Some(x) => msg = x,
                None => return Ok(msg_out),
            }
        }
    }
    fn as_mut_dyn(&mut self) -> &mut dyn ProtocolAdapter {
        self
    }
}
