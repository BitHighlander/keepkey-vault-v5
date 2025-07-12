use super::{Message, MessageType};
use core::fmt::{self, Display, Formatter};
use prost::DecodeError;
use thiserror::Error;

#[derive(Error, Debug)]
pub struct EncodeError {
    required: usize,
    remaining: usize,
}

impl Display for EncodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to encode message; insufficient buffer capacity (required: {}, remaining: {})",
            self.required, self.remaining
        )
    }
}

impl EncodeError {
    pub const fn new(required: usize, remaining: usize) -> Self {
        Self {
            required,
            remaining,
        }
    }
}

impl Message {
    pub fn encoded_len(&self) -> usize {
        prost::Message::encoded_len(self) + 8
    }

    pub fn encode<B: bytes::BufMut>(&self, buf: &mut B) -> Result<(), EncodeError> {
        let encoded_len = prost::Message::encoded_len(self);
        let required = 8 + encoded_len;
        let remaining = buf.remaining_mut();
        if remaining < required {
            return Err(EncodeError::new(required, remaining));
        }
        buf.put_u8(b'#');
        buf.put_u8(b'#');
        buf.put_u16(Into::<i32>::into(self.message_type()).try_into().unwrap());
        buf.put_u32(encoded_len.try_into().unwrap());
        prost::Message::encode(self, buf)
            .map_err(|x| EncodeError::new(x.required_capacity(), x.remaining()))?;
        Ok(())
    }

    pub fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError> {
        if buf.remaining() < 8 {
            return Err(DecodeError::new("buffer too short"));
        }
        if !(buf.get_u8() == b'#' && buf.get_u8() == b'#') {
            return Err(DecodeError::new("bad magic bytes"));
        }
        let msg_type: i32 = buf.get_u16().into();
        let msg_len: usize = buf.get_u32().try_into().unwrap();
        if buf.remaining() < msg_len {
            return Err(DecodeError::new("buffer too short"));
        }

        Self::decode_as_type(
            buf,
            MessageType::from_i32(msg_type).ok_or_else(|| DecodeError::new("bad message type"))?,
        )
    }
}
