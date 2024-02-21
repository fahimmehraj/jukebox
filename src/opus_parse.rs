use std::{
    pin::Pin,
    task::{Context, Poll},
    vec,
};

use bytes::BytesMut;
use log::{debug, error, trace};
use packed_struct::prelude::*;
use tokio::io::{AsyncRead, ReadBuf};
use tokio_stream::Stream;

#[derive(PackedStruct, Debug)]
#[packed_struct(endian = "lsb", bit_numbering = "msb0", size_bytes = "27")]
pub struct OggPageHeader {
    #[packed_field(bytes = "0..=3")]
    capture_pattern: [u8; 4],

    #[packed_field(bytes = "4")]
    pad_byte: u8,

    #[packed_field(bytes = "5")]
    flag: u8,

    #[packed_field(bytes = "6..=13")]
    gran_pos: u64,

    #[packed_field(bytes = "14..=17")]
    serial: u32,

    #[packed_field(bytes = "18..=21")]
    pagenum: u32,

    #[packed_field(bytes = "22..=25")]
    crc: u32,

    #[packed_field(bytes = "26")]
    segnum: u8,
}

pub struct OggStream<T: AsyncRead + Unpin> {
    stream: T,
    segment_table: Option<vec::IntoIter<u8>>,
    current_packet: Option<Vec<u8>>,
}

impl<T: AsyncRead + Unpin> Stream for OggStream<T> {
    type Item = Vec<u8>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Vec<u8>>> {
        match self.segment_table {
            Some(ref mut segtable) => match segtable.next() {
                Some(seg) => {
                    let mut packet = vec![0u8; seg as usize];
                    let mut read_buf = ReadBuf::new(&mut packet);
                    match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                        Poll::Ready(Ok(())) => {
                            self.current_packet = Some(
                                [self.current_packet.take().unwrap_or_default(), packet].concat(),
                            );
                            if seg == 255 {
                                return self.poll_next(cx);
                            }
                            Poll::Ready(self.current_packet.take())
                        }
                        Poll::Ready(Err(e)) => {
                            error!("error reading packet: {}", e);
                            Poll::Ready(None)
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                None => {
                    self.segment_table = None;
                    self.poll_next(cx)
                }
            },
            None => {
                let mut buf = [0u8; 27];
                let mut read_buf = ReadBuf::new(&mut buf);
                // read until buf is full
                match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(())) => {
                        let header = OggPageHeader::unpack(&buf).unwrap();
                        debug!("{:?}", &header.capture_pattern);
                        let mut segtable = vec![0u8; header.segnum as usize];
                        let mut read_buf = ReadBuf::new(&mut segtable);
                        match Pin::new(&mut self.stream).poll_read(cx, &mut read_buf) {
                            Poll::Ready(Ok(())) => {
                                self.segment_table = Some(segtable.into_iter());
                                self.poll_next(cx)
                            }
                            Poll::Ready(Err(e)) => {
                                error!("error reading segtable: {}", e);
                                Poll::Ready(None)
                            }
                            Poll::Pending => Poll::Pending,
                        }
                    }
                    Poll::Ready(Err(e)) => {
                        error!("error reading header: {}", e);
                        Poll::Ready(None)
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }
}

impl<T: AsyncRead + Unpin> OggStream<T> {
    pub fn new(stream: T) -> Self {
        OggStream {
            stream,
            segment_table: None,
            current_packet: None,
        }
    }
}

