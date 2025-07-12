macro_rules! kk_message {
    ($($x:ident),*) => {
        #[derive(Debug, Clone)]
        pub enum Message {
            $($x(protos::$x)),*
        }

        impl ::prost::Message for Message {
            fn encode_raw<B>(&self, buf: &mut B)
            where
                B: ::bytes::buf::BufMut,
                Self: Sized {
                match self {
                    $(Self::$x(x) => x.encode_raw(buf)),*
                }
            }

            fn merge_field<B>(
                &mut self,
                tag: u32,
                wire_type: ::prost::encoding::WireType,
                buf: &mut B,
                ctx: ::prost::encoding::DecodeContext,
            ) -> Result<(), ::prost::DecodeError>
            where
                B: ::bytes::buf::Buf,
                Self: Sized
            {
                match self {
                    $(Self::$x(x) => x.merge_field(tag, wire_type, buf, ctx)),*
                }
            }

            fn encoded_len(&self) -> usize {
                match self {
                    $(Self::$x(x) => x.encoded_len()),*
                }
            }

            fn clear(&mut self) {
                match self {
                    $(Self::$x(x) => x.clear()),*
                }
            }
        }

        impl Message {
            pub const fn message_type(&self) -> protos::MessageType {
                match self {
                    $(Message::$x(_) => protos::MessageType::$x),*
                }
            }

            fn decode_as_type<B: bytes::Buf>(buf: &mut B, message_type: protos::MessageType) -> Result<Self, ::prost::DecodeError> {
                Ok(match message_type {
                    $(protos::MessageType::$x => Message::$x(<protos::$x as ::prost::Message>::decode(buf)?)),*
                })
            }
        }

        $(impl From<protos::$x> for Message {
            fn from(x: protos::$x) -> Self {
                Self::$x(x)
            }
        })*

        $(impl TryFrom<Message> for protos::$x {
            type Error = ();
            fn try_from(x: Message) -> Result<Self, Self::Error> {
                match x {
                    Message::$x(y) => Ok(y),
                    _ => Err(())
                }
            }
        })*
    };
}

pub(crate) use kk_message;
